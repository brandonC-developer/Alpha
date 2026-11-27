use std::collections::HashMap;

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};
use crate::model::Tool;
use crate::plans::{PlanKind, WindowName, active_5h_block_start, list_plans, window_status};
use crate::query::{Filter, compute_tool_anomalies, sessions};
use crate::time::fmt_utc;

/// Alerta evaluada lista para notificarse al usuario.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Alert {
    pub rule: String,
    pub key: String,
    pub title: String,
    pub body: String,
}

struct AlertSettings {
    window5h_levels: Vec<f64>,
    weekly_level: Option<f64>,
    budget_level: Option<f64>,
    anomaly: bool,
    paused_until: Option<DateTime<Utc>>,
}

/// Evalúa todas las reglas de alerta en `now` usando la zona horaria local y registra los disparos nuevos.
pub fn evaluate_alerts(conn: &Connection, now: DateTime<Utc>) -> Result<Vec<Alert>> {
    evaluate_alerts_in_tz(conn, now, &Local)
}

/// Evalúa todas las reglas de alerta en `now` usando `tz` para formatear horas, meses y semanas locales.
pub fn evaluate_alerts_in_tz<Tz: TimeZone>(
    conn: &Connection,
    now: DateTime<Utc>,
    tz: &Tz,
) -> Result<Vec<Alert>> {
    let settings = load_alert_settings(conn)?;
    let mut candidates = Vec::new();

    evaluate_plan_alerts(conn, now, tz, &settings, &mut candidates)?;
    if settings.anomaly {
        evaluate_anomaly_alerts(conn, now, &mut candidates)?;
    }

    let paused = settings.paused_until.is_some_and(|until| now < until);
    let now_str = fmt_utc(now);
    let mut emitted = Vec::new();

    let mut insert_stmt = conn.prepare(
        "INSERT OR IGNORE INTO alert_fired (rule, key, at)
         VALUES (?1, ?2, ?3)",
    )?;
    for alert in candidates {
        let inserted = insert_stmt.execute(params![&alert.rule, &alert.key, &now_str])?;
        if inserted > 0 && !paused {
            emitted.push(alert);
        }
    }

    Ok(emitted)
}

fn evaluate_plan_alerts<Tz: TimeZone>(
    conn: &Connection,
    now: DateTime<Utc>,
    tz: &Tz,
    settings: &AlertSettings,
    out: &mut Vec<Alert>,
) -> Result<()> {
    let plans = list_plans(conn)?;
    let local_now = now.with_timezone(tz);

    for plan in plans {
        match plan.kind {
            PlanKind::Subscription => {
                let statuses = window_status(conn, &plan.id, now)?;
                for ws in statuses {
                    let (Some(_limit), Some(percent)) = (ws.limit, ws.percent) else {
                        continue;
                    };

                    match ws.name {
                        WindowName::FiveHours => {
                            let Some(block_start) = active_5h_block_start(conn, &plan.id, now)?
                            else {
                                continue;
                            };
                            let block_key = format!("{}:{}", plan.id, fmt_utc(block_start));
                            for &level in &settings.window5h_levels {
                                if percent < level {
                                    continue;
                                }
                                let level_str = fmt_level(level);
                                let body = format_5h_body(ws.resets_at, ws.exhausts_at, tz);
                                out.push(Alert {
                                    rule: format!("window5h.{level_str}"),
                                    key: block_key.clone(),
                                    title: format!(
                                        "{}: {level_str} % de la ventana de 5 h",
                                        plan.name
                                    ),
                                    body,
                                });
                            }
                        }
                        WindowName::Weekly => {
                            let Some(level) = settings.weekly_level else {
                                continue;
                            };
                            if percent < level {
                                continue;
                            }
                            let level_str = fmt_level(level);
                            let days = match ws.resets_at {
                                Some(resets_at) => days_until(now, resets_at),
                                None => fallback_weekly_days(conn, &plan.id, now)?,
                            };
                            out.push(Alert {
                                rule: format!("weekly.{level_str}"),
                                key: format!(
                                    "{}:{}",
                                    plan.id,
                                    local_now.date_naive().format("%G-W%V")
                                ),
                                title: format!("{}: {level_str} % del límite semanal", plan.name),
                                body: format!(
                                    "Quedan {days} días para que se renueve por completo."
                                ),
                            });
                        }
                    }
                }
            }
            PlanKind::Payg => {
                let (Some(level), Some(budget_usd)) = (settings.budget_level, plan.budget_usd)
                else {
                    continue;
                };
                if budget_usd <= 0.0 {
                    continue;
                }

                let month_start = local_month_start_utc(now, tz)?;
                let spent_usd: f64 = if month_start <= now {
                    conn.query_row(
                        "SELECT COALESCE(SUM(cost_usd), 0.0)
                         FROM usage_event
                         WHERE plan_id = ?1
                           AND cost_kind = 'real'
                           AND ts_utc >= ?2
                           AND ts_utc <= ?3",
                        params![&plan.id, &fmt_utc(month_start), &fmt_utc(now)],
                        |r| r.get(0),
                    )?
                } else {
                    0.0
                };

                if (spent_usd / budget_usd * 100.0) < level {
                    continue;
                }

                let level_str = fmt_level(level);
                let balance_usd = if plan.id == "deepseek" {
                    read_setting(conn, "deepseek.balance_usd")?
                        .and_then(|s| s.trim().parse::<f64>().ok())
                        .filter(|v| v.is_finite())
                        .unwrap_or_else(|| (budget_usd - spent_usd).max(0.0))
                } else {
                    (budget_usd - spent_usd).max(0.0)
                };

                out.push(Alert {
                    rule: format!("budget.{level_str}"),
                    key: format!(
                        "{}:{:04}-{:02}",
                        plan.id,
                        local_now.year(),
                        local_now.month()
                    ),
                    title: format!(
                        "{}: {} $ de {} $ este mes",
                        plan.name,
                        fmt_es_2(spent_usd),
                        fmt_es_compact(budget_usd)
                    ),
                    body: format!("Saldo: {} $.", fmt_es_2(balance_usd)),
                });
            }
        }
    }

    Ok(())
}

fn evaluate_anomaly_alerts(
    conn: &Connection,
    now: DateTime<Utc>,
    out: &mut Vec<Alert>,
) -> Result<()> {
    let epoch = DateTime::<Utc>::from_timestamp(0, 0).unwrap_or(now);
    let filter = Filter {
        from: epoch,
        to: now + Duration::seconds(1),
        tool: None,
        provider: None,
        project: None,
    };
    let mut rows = sessions(conn, &filter)?;
    rows.retain(|s| s.anomalous);
    if rows.is_empty() {
        return Ok(());
    }

    rows.sort_by(|a, b| {
        a.start
            .cmp(&b.start)
            .then_with(|| a.session_id.cmp(&b.session_id))
    });

    let mut stats_by_tool: HashMap<Tool, HashMap<String, (bool, f64)>> = HashMap::new();
    for row in rows {
        let tool_stats = match stats_by_tool.entry(row.tool) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(compute_tool_anomalies(conn, row.tool)?)
            }
        };
        let mean = tool_stats
            .get(&row.session_id)
            .map(|(_, m)| *m)
            .unwrap_or(0.0);
        let total =
            row.tokens.input + row.tokens.cache_write + row.tokens.cache_read + row.tokens.output;
        let ratio = if mean > 0.0 {
            ((total as f64) / mean).round().max(1.0) as u64
        } else {
            1
        };

        out.push(Alert {
            rule: "anomaly".to_string(),
            key: row.session_id,
            title: format!("Sesión fuera de lo normal en {}", tool_label(row.tool)),
            body: format!(
                "{} tokens, {}× tu media. Ábrela en Sesiones.",
                fmt_tokens_es(total),
                ratio
            ),
        });
    }

    Ok(())
}

fn load_alert_settings(conn: &Connection) -> Result<AlertSettings> {
    let window5h_levels = match read_setting(conn, "alert.window5h.levels")? {
        Some(raw) => parse_levels(&raw),
        None => vec![80.0, 95.0],
    };

    let weekly_level = match read_setting(conn, "alert.weekly.level")? {
        Some(raw) => parse_positive_level(&raw),
        None => Some(80.0),
    };

    let budget_level = match read_setting(conn, "alert.budget.level")? {
        Some(raw) => parse_positive_level(&raw),
        None => Some(80.0),
    };

    let anomaly = match read_setting(conn, "alert.anomaly")? {
        Some(raw) => match raw.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "si" | "sí" => true,
            "false" | "0" | "no" => false,
            _ => true,
        },
        None => true,
    };

    let paused_until = read_setting(conn, "alerts.paused_until")?
        .and_then(|s| s.trim().parse::<DateTime<Utc>>().ok());

    Ok(AlertSettings {
        window5h_levels,
        weekly_level,
        budget_level,
        anomaly,
        paused_until,
    })
}

fn read_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row("SELECT value FROM setting WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .optional()?)
}

fn parse_levels(raw: &str) -> Vec<f64> {
    let mut levels: Vec<f64> = if let Ok(arr) = serde_json::from_str::<Vec<f64>>(raw) {
        arr
    } else {
        raw.split(',')
            .filter_map(|part| part.trim().parse::<f64>().ok())
            .collect()
    };
    levels.retain(|v| v.is_finite() && *v > 0.0);
    levels.sort_by(|a, b| a.total_cmp(b));
    levels.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    levels
}

fn parse_positive_level(raw: &str) -> Option<f64> {
    raw.trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v > 0.0)
}

fn format_5h_body<Tz: TimeZone>(
    resets_at: Option<DateTime<Utc>>,
    exhausts_at: Option<DateTime<Utc>>,
    tz: &Tz,
) -> String {
    let mut parts = Vec::new();
    if let Some(reset) = resets_at {
        parts.push(format!(
            "Se reinicia a las {}.",
            reset.with_timezone(tz).time().format("%H:%M")
        ));
    }
    if let Some(exhausts) = exhausts_at {
        parts.push(format!(
            "A este ritmo se agota a las {}.",
            exhausts.with_timezone(tz).time().format("%H:%M")
        ));
    }
    parts.join(" ")
}

fn days_until(now: DateTime<Utc>, target: DateTime<Utc>) -> i64 {
    let secs = (target - now).num_seconds().max(0);
    if secs == 0 {
        0
    } else {
        ((secs as f64) / 86_400.0).ceil() as i64
    }
}

fn fallback_weekly_days(conn: &Connection, plan_id: &str, now: DateTime<Utc>) -> Result<i64> {
    let week_start = fmt_utc(now - Duration::hours(168));
    let now_str = fmt_utc(now);
    let latest: Option<String> = conn
        .query_row(
            "SELECT MAX(ts_utc)
             FROM usage_event
             WHERE plan_id = ?1 AND ts_utc > ?2 AND ts_utc <= ?3",
            params![plan_id, &week_start, &now_str],
            |r| r.get(0),
        )
        .optional()?
        .flatten();

    let Some(ts_str) = latest else {
        return Ok(7);
    };
    let ts = ts_str
        .parse::<DateTime<Utc>>()
        .map_err(|err| CoreError::Parse {
            file: "usage_event".to_string(),
            line: 0,
            reason: format!("fecha UTC inválida '{ts_str}': {err}"),
        })?;
    Ok(days_until(now, ts + Duration::hours(168)).clamp(1, 7))
}

fn local_month_start_utc<Tz: TimeZone>(now: DateTime<Utc>, tz: &Tz) -> Result<DateTime<Utc>> {
    let local_now = now.with_timezone(tz);
    let month_start_naive = NaiveDate::from_ymd_opt(local_now.year(), local_now.month(), 1)
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .ok_or_else(|| CoreError::Parse {
            file: "time".to_string(),
            line: 0,
            reason: "fecha de inicio de mes inválida".to_string(),
        })?;
    Ok(tz
        .from_local_datetime(&month_start_naive)
        .earliest()
        .ok_or_else(|| CoreError::Parse {
            file: "time".to_string(),
            line: 0,
            reason: "conversión local de inicio de mes inválida".to_string(),
        })?
        .with_timezone(&Utc))
}

fn fmt_level(level: f64) -> String {
    if (level - level.round()).abs() < 1e-9 {
        format!("{}", level.round() as i64)
    } else {
        format!("{level}").replace('.', ",")
    }
}

fn fmt_es_2(value: f64) -> String {
    format!("{value:.2}").replace('.', ",")
}

fn fmt_es_compact(value: f64) -> String {
    if (value - value.round()).abs() < 1e-9 {
        format!("{}", value.round() as i64)
    } else {
        fmt_es_2(value)
    }
}

fn fmt_tokens_es(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1} B", n as f64 / 1_000_000_000.0).replace('.', ",")
    } else if n >= 1_000_000 {
        format!("{:.1} M", n as f64 / 1_000_000.0).replace('.', ",")
    } else if n >= 1_000 {
        format!("{:.1} K", n as f64 / 1_000.0).replace('.', ",")
    } else {
        n.to_string()
    }
}

fn tool_label(tool: Tool) -> &'static str {
    match tool {
        Tool::ClaudeCode => "Claude Code",
        Tool::Antigravity => "Antigravity",
        Tool::Opencode => "opencode",
    }
}
