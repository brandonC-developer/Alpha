use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};
use crate::model::UsageEvent;
use crate::pricing::{Price, PriceInput, compile_pattern, find_price, refresh_valid_to};
use crate::time::{fmt_utc, hour_utc_str};

/// Clasificación económica del coste registrado para un evento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostKind {
    Real,
    ApiEquivalent,
    Unknown,
}

impl CostKind {
    /// Identificador textual almacenado en SQLite y devuelto por la API.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Real => "real",
            Self::ApiEquivalent => "api_equivalent",
            Self::Unknown => "unknown",
        }
    }
}

impl fmt::Display for CostKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for CostKind {
    type Err = CoreError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "real" => Ok(Self::Real),
            "api_equivalent" => Ok(Self::ApiEquivalent),
            "unknown" => Ok(Self::Unknown),
            _ => Err(CoreError::Parse {
                file: String::new(),
                line: 0,
                reason: format!("tipo de coste desconocido: {s}"),
            }),
        }
    }
}

/// Resumen del recálculo de costes tras crear o modificar una tarifa.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecalcResult {
    pub events: u64,
    pub old_total: f64,
    pub new_total: f64,
}

/// Calcula el coste en dólares de `e` según la tarifa `p`; `output` ya incluye `reasoning`.
pub fn compute_cost(e: &UsageEvent, p: &Price) -> f64 {
    compute_cost_tokens(e.input, e.output, e.cache_write, e.cache_read, p)
}

fn compute_cost_tokens(
    input: u64,
    output: u64,
    cache_write: u64,
    cache_read: u64,
    p: &Price,
) -> f64 {
    (input as f64 * p.input
        + output as f64 * p.output
        + cache_write as f64 * p.cache_write
        + cache_read as f64 * p.cache_read)
        / 1_000_000.0
}

/// Calcula y guarda el coste del evento identificado por `dedup_key`.
pub fn apply_cost(conn: &Connection, dedup_key: &str) -> Result<()> {
    let (model, ts_utc_str, plan_id, input, cache_write, cache_read, output, plan_kind): (
        String,
        String,
        String,
        i64,
        i64,
        i64,
        i64,
        String,
    ) = conn.query_row(
        "SELECT e.model, e.ts_utc, e.plan_id, e.input, e.cache_write, e.cache_read, e.output, p.kind
         FROM usage_event e
         JOIN plan p ON p.id = e.plan_id
         WHERE e.dedup_key = ?1",
        [dedup_key],
        |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
            ))
        },
    )?;

    let ts = parse_utc(&ts_utc_str, "usage_event")?;
    let event_date = ts.date_naive();
    let computed_at = fmt_utc(Utc::now());

    let (cost_usd, cost_kind, price_id) = match find_price(conn, &model, event_date)? {
        Some(price) => {
            let cost = compute_cost_tokens(
                input.max(0) as u64,
                output.max(0) as u64,
                cache_write.max(0) as u64,
                cache_read.max(0) as u64,
                &price,
            );
            let kind = cost_kind_from_plan_kind(&plan_kind, &plan_id)?;
            (Some(cost), kind, Some(price.id))
        }
        None => (None, CostKind::Unknown, None),
    };

    conn.execute(
        "UPDATE usage_event
         SET cost_usd = ?1,
             cost_kind = ?2,
             price_id = ?3,
             cost_computed_at = ?4
         WHERE dedup_key = ?5",
        params![
            cost_usd,
            cost_kind.as_str(),
            price_id,
            &computed_at,
            dedup_key
        ],
    )?;

    Ok(())
}

/// Crea o actualiza un precio (`user_edited = 1`), recalcula solo los eventos afectados y registra el log.
pub fn upsert_price(conn: &mut Connection, p: PriceInput) -> Result<RecalcResult> {
    let valid_from_str = p.valid_from.format("%Y-%m-%d").to_string();
    let matcher = compile_pattern(&p.model_pattern)?;
    let now_str = fmt_utc(Utc::now());

    let tx = conn.transaction()?;

    tx.execute(
        "INSERT INTO price (
            provider, model_pattern, valid_from, valid_to,
            input, output, cache_write, cache_read,
            source_url, approximate, user_edited
         ) VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7, ?8, ?9, 1)
         ON CONFLICT(model_pattern, valid_from) DO UPDATE SET
            provider = excluded.provider,
            input = excluded.input,
            output = excluded.output,
            cache_write = excluded.cache_write,
            cache_read = excluded.cache_read,
            source_url = excluded.source_url,
            approximate = excluded.approximate,
            user_edited = 1",
        params![
            p.provider.as_str(),
            &p.model_pattern,
            &valid_from_str,
            p.input,
            p.output,
            p.cache_write,
            p.cache_read,
            &p.source_url,
            i64::from(p.approximate),
        ],
    )?;

    refresh_valid_to(&tx, &p.model_pattern)?;

    let (upserted_id, valid_to_str): (i64, Option<String>) = tx.query_row(
        "SELECT id, valid_to FROM price WHERE model_pattern = ?1 AND valid_from = ?2",
        params![&p.model_pattern, &valid_from_str],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let valid_to = valid_to_str
        .as_deref()
        .map(|s| {
            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|err| CoreError::Parse {
                file: "price".to_string(),
                line: 0,
                reason: format!("fecha valid_to inválida '{s}': {err}"),
            })
        })
        .transpose()?;

    struct EventRow {
        dedup_key: String,
        tool: String,
        provider: String,
        plan_id: String,
        plan_kind: String,
        model: String,
        ts_utc: String,
        project: Option<String>,
        input: i64,
        cache_write: i64,
        cache_read: i64,
        output: i64,
        old_cost: Option<f64>,
        old_kind: String,
    }

    let candidates: Vec<EventRow> = {
        let mut stmt = tx.prepare(
            "SELECT e.dedup_key, e.tool, e.provider, e.plan_id, pl.kind,
                    e.model, e.ts_utc, e.project,
                    e.input, e.cache_write, e.cache_read, e.output,
                    e.cost_usd, e.cost_kind
             FROM usage_event e
             JOIN plan pl ON pl.id = e.plan_id
             WHERE substr(e.ts_utc, 1, 10) >= ?1",
        )?;
        stmt.query_map([&valid_from_str], |r| {
            Ok(EventRow {
                dedup_key: r.get(0)?,
                tool: r.get(1)?,
                provider: r.get(2)?,
                plan_id: r.get(3)?,
                plan_kind: r.get(4)?,
                model: r.get(5)?,
                ts_utc: r.get(6)?,
                project: r.get(7)?,
                input: r.get(8)?,
                cache_write: r.get(9)?,
                cache_read: r.get(10)?,
                output: r.get(11)?,
                old_cost: r.get(12)?,
                old_kind: r.get(13)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?
    };

    let mut events_count: u64 = 0;
    let mut old_total: f64 = 0.0;
    let mut new_total: f64 = 0.0;

    {
        let mut update_event_stmt = tx.prepare(
            "UPDATE usage_event
             SET cost_usd = ?1,
                 cost_kind = ?2,
                 price_id = ?3,
                 cost_computed_at = ?4
             WHERE dedup_key = ?5",
        )?;
        let mut update_hourly_stmt = tx.prepare(
            "UPDATE hourly_agg
             SET cost_real = cost_real + ?1,
                 cost_equiv = cost_equiv + ?2,
                 unpriced = unpriced + ?3
             WHERE hour_utc = ?4
               AND tool = ?5
               AND provider = ?6
               AND plan_id = ?7
               AND model = ?8
               AND project = ?9",
        )?;

        for ev in candidates {
            if !matcher.is_match(&ev.model) {
                continue;
            }
            let ts = parse_utc(&ev.ts_utc, "usage_event")?;
            let ev_date = ts.date_naive();
            if ev_date < p.valid_from || valid_to.is_some_and(|vt| ev_date >= vt) {
                continue;
            }

            let Some(best_price) = find_price(&tx, &ev.model, ev_date)? else {
                continue;
            };
            if best_price.id != upserted_id {
                continue;
            }

            let new_cost = compute_cost_tokens(
                ev.input.max(0) as u64,
                ev.output.max(0) as u64,
                ev.cache_write.max(0) as u64,
                ev.cache_read.max(0) as u64,
                &best_price,
            );
            let new_kind = cost_kind_from_plan_kind(&ev.plan_kind, &ev.plan_id)?;

            events_count += 1;
            old_total += ev.old_cost.unwrap_or(0.0);
            new_total += new_cost;

            update_event_stmt.execute(params![
                new_cost,
                new_kind.as_str(),
                best_price.id,
                &now_str,
                &ev.dedup_key,
            ])?;

            let (old_real, old_equiv, old_unpriced) = split_hourly_cost(&ev.old_kind, ev.old_cost);
            let (new_real, new_equiv, new_unpriced) =
                split_hourly_cost(new_kind.as_str(), Some(new_cost));

            let hour_str = hour_utc_str(ts);
            let project_key = ev.project.as_deref().unwrap_or("");
            update_hourly_stmt.execute(params![
                new_real - old_real,
                new_equiv - old_equiv,
                new_unpriced - old_unpriced,
                &hour_str,
                &ev.tool,
                &ev.provider,
                &ev.plan_id,
                &ev.model,
                project_key,
            ])?;
        }
    }

    tx.execute(
        "INSERT INTO cost_recalc_log (at, price_id, events, old_total, new_total)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            &now_str,
            upserted_id,
            events_count as i64,
            old_total,
            new_total
        ],
    )?;

    tx.commit()?;

    Ok(RecalcResult {
        events: events_count,
        old_total,
        new_total,
    })
}

fn cost_kind_from_plan_kind(plan_kind: &str, plan_id: &str) -> Result<CostKind> {
    match plan_kind {
        "payg" => Ok(CostKind::Real),
        "subscription" => Ok(CostKind::ApiEquivalent),
        _ => Err(CoreError::Parse {
            file: "plan".to_string(),
            line: 0,
            reason: format!("tipo de plan desconocido '{plan_kind}' para '{plan_id}'"),
        }),
    }
}

fn split_hourly_cost(cost_kind: &str, cost_usd: Option<f64>) -> (f64, f64, i64) {
    match (cost_kind, cost_usd) {
        ("real", Some(c)) => (c, 0.0, 0),
        ("api_equivalent", Some(c)) => (0.0, c, 0),
        _ => (0.0, 0.0, 1),
    }
}

fn parse_utc(s: &str, context: &str) -> Result<DateTime<Utc>> {
    s.parse::<DateTime<Utc>>().map_err(|err| CoreError::Parse {
        file: context.to_string(),
        line: 0,
        reason: format!("fecha UTC inválida '{s}': {err}"),
    })
}
