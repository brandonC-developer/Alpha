use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Duration, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize, Serializer};

use crate::error::{CoreError, Result};
use crate::model::Provider;
use crate::time::fmt_utc;

/// Ventana temporal de consumo de una suscripción.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WindowName {
    #[serde(rename = "5h")]
    FiveHours,
    #[serde(rename = "weekly")]
    Weekly,
}

impl WindowName {
    /// Identificador textual usado en SQLite y en los comandos de Tauri.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FiveHours => "5h",
            Self::Weekly => "weekly",
        }
    }
}

impl fmt::Display for WindowName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for WindowName {
    type Err = CoreError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "5h" => Ok(Self::FiveHours),
            "weekly" => Ok(Self::Weekly),
            _ => Err(CoreError::Parse {
                file: "plan".to_string(),
                line: 0,
                reason: format!("ventana desconocida: {s}"),
            }),
        }
    }
}

/// Modalidad de facturación del plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlanKind {
    Subscription,
    Payg,
}

impl PlanKind {
    /// Identificador textual almacenado en la tabla `plan`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Subscription => "subscription",
            Self::Payg => "payg",
        }
    }
}

impl fmt::Display for PlanKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for PlanKind {
    type Err = CoreError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "subscription" => Ok(Self::Subscription),
            "payg" => Ok(Self::Payg),
            _ => Err(CoreError::Parse {
                file: "plan".to_string(),
                line: 0,
                reason: format!("tipo de plan desconocido: {s}"),
            }),
        }
    }
}

/// Origen de una calibración registrada para una ventana.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CalibrationSource {
    Agy,
    User,
}

impl CalibrationSource {
    /// Identificador textual guardado en `plan_calibration.source`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Agy => "agy",
            Self::User => "user",
        }
    }
}

impl fmt::Display for CalibrationSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for CalibrationSource {
    type Err = CoreError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "agy" => Ok(Self::Agy),
            "user" => Ok(Self::User),
            _ => Err(CoreError::Parse {
                file: "plan_calibration".to_string(),
                line: 0,
                reason: format!("origen de calibración desconocido: {s}"),
            }),
        }
    }
}

/// Definición de una ventana configurada en la columna `plan.windows`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanWindowSpec {
    pub name: WindowName,
    pub hours: i64,
}

/// Configuración de un plan almacenado en la tabla `plan`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub id: String,
    pub name: String,
    pub provider: Provider,
    pub kind: PlanKind,
    pub monthly_fee_usd: f64,
    pub budget_usd: Option<f64>,
    pub windows: Vec<PlanWindowSpec>,
}

/// Bloque activo de consumo con inicio, fin y peso acumulado.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Block {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub used: f64,
}

fn serialize_opt_utc<S: Serializer>(
    dt: &Option<DateTime<Utc>>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error> {
    match dt {
        Some(ts) => serializer.serialize_str(&fmt_utc(*ts)),
        None => serializer.serialize_none(),
    }
}

/// Estado actual de una ventana de suscripción conforme al contrato de Tauri.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowStatus {
    pub name: WindowName,
    pub used: f64,
    pub limit: Option<f64>,
    pub percent: Option<f64>,
    #[serde(serialize_with = "serialize_opt_utc")]
    pub resets_at: Option<DateTime<Utc>>,
    #[serde(serialize_with = "serialize_opt_utc")]
    pub exhausts_at: Option<DateTime<Utc>>,
    pub unpriced_events: u64,
    pub calibrated_by: Option<CalibrationSource>,
    #[serde(serialize_with = "serialize_opt_utc")]
    pub calibrated_at: Option<DateTime<Utc>>,
}

/// Lee todos los planes de la base de datos ordenados por su orden de siembra/identificador.
pub fn list_plans(conn: &Connection) -> Result<Vec<Plan>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, provider, kind, monthly_fee_usd, budget_usd, windows
         FROM plan
         ORDER BY rowid ASC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, f64>(4)?,
            r.get::<_, Option<f64>>(5)?,
            r.get::<_, String>(6)?,
        ))
    })?;

    let mut plans = Vec::new();
    for row in rows {
        let (id, name, provider_str, kind_str, monthly_fee_usd, budget_usd, windows_json) = row?;
        plans.push(build_plan(
            id,
            name,
            &provider_str,
            &kind_str,
            monthly_fee_usd,
            budget_usd,
            &windows_json,
        )?);
    }
    Ok(plans)
}

/// Lee un plan concreto por su `plan_id`.
pub fn get_plan(conn: &Connection, plan_id: &str) -> Result<Plan> {
    let (id, name, provider_str, kind_str, monthly_fee_usd, budget_usd, windows_json): (
        String,
        String,
        String,
        String,
        f64,
        Option<f64>,
        String,
    ) = conn.query_row(
        "SELECT id, name, provider, kind, monthly_fee_usd, budget_usd, windows
         FROM plan
         WHERE id = ?1",
        [plan_id],
        |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ))
        },
    )?;

    build_plan(
        id,
        name,
        &provider_str,
        &kind_str,
        monthly_fee_usd,
        budget_usd,
        &windows_json,
    )
}

/// Actualiza la cuota mensual y el presupuesto opcional de un plan.
pub fn update_plan(
    conn: &Connection,
    plan_id: &str,
    monthly_fee_usd: f64,
    budget_usd: Option<f64>,
) -> Result<Plan> {
    if !monthly_fee_usd.is_finite() || monthly_fee_usd < 0.0 {
        return Err(CoreError::Parse {
            file: "plan".to_string(),
            line: 0,
            reason: "La cuota mensual debe ser un número no negativo".to_string(),
        });
    }
    if let Some(budget) = budget_usd
        && (!budget.is_finite() || budget < 0.0)
    {
        return Err(CoreError::Parse {
            file: "plan".to_string(),
            line: 0,
            reason: "El presupuesto debe ser un número no negativo".to_string(),
        });
    }

    let changed = conn.execute(
        "UPDATE plan SET monthly_fee_usd = ?1, budget_usd = ?2 WHERE id = ?3",
        params![monthly_fee_usd, budget_usd, plan_id],
    )?;
    if changed == 0 {
        return Err(CoreError::Db(rusqlite::Error::QueryReturnedNoRows));
    }
    get_plan(conn, plan_id)
}

/// Reconstruye el bloque activo en `now` a partir de puntos `(ts, peso)` ordenados.
pub fn current_block(
    points: &[(DateTime<Utc>, f64)],
    now: DateTime<Utc>,
    hours: i64,
) -> Option<Block> {
    if hours <= 0 {
        return None;
    }
    let duration = Duration::hours(hours);
    let mut current: Option<Block> = None;

    for &(ts, weight) in points {
        if ts > now {
            break;
        }
        match current {
            Some(ref mut block) if ts < block.end => {
                block.used += weight;
            }
            _ => {
                current = Some(Block {
                    start: ts,
                    end: ts + duration,
                    used: weight,
                });
            }
        }
    }

    current.filter(|block| now < block.end)
}

/// Registra una calibración manual del usuario (`percent` en `(0, 100]`) y devuelve el nuevo límite.
pub fn calibrate(
    conn: &Connection,
    plan_id: &str,
    window: WindowName,
    percent: f64,
    now: DateTime<Utc>,
) -> Result<f64> {
    calibrate_with_source(
        conn,
        plan_id,
        window,
        percent,
        CalibrationSource::User,
        None,
        now,
    )
}

pub(crate) fn calibrate_with_source(
    conn: &Connection,
    plan_id: &str,
    window: WindowName,
    percent: f64,
    source: CalibrationSource,
    resets_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> Result<f64> {
    match try_calibrate_with_source(conn, plan_id, window, percent, source, resets_at, now)? {
        Some(limit) => Ok(limit),
        None => Err(CoreError::Parse {
            file: "plan_calibration".to_string(),
            line: 0,
            reason: "No hay uso en esta ventana: usa la herramienta y vuelve a calibrar"
                .to_string(),
        }),
    }
}

pub(crate) fn try_calibrate_with_source(
    conn: &Connection,
    plan_id: &str,
    window: WindowName,
    percent: f64,
    source: CalibrationSource,
    resets_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> Result<Option<f64>> {
    if !percent.is_finite() || percent <= 0.0 || percent > 100.0 {
        return Err(CoreError::Parse {
            file: "plan_calibration".to_string(),
            line: 0,
            reason: "El porcentaje debe estar entre 1 y 100".to_string(),
        });
    }

    let plan = get_plan(conn, plan_id)?;
    if !plan.windows.iter().any(|w| w.name == window) {
        return Err(CoreError::Parse {
            file: "plan".to_string(),
            line: 0,
            reason: format!("el plan '{plan_id}' no tiene ventana '{}'", window.as_str()),
        });
    }

    let events = load_plan_events(conn, plan_id, now)?;
    let measurement = measure_window(&events, window, now);
    if measurement.used <= 0.0 {
        return Ok(None);
    }

    let limit_est = measurement.used / (percent / 100.0);
    let at_str = fmt_utc(now);
    let resets_str = resets_at.map(fmt_utc);

    conn.execute(
        "INSERT INTO plan_calibration (
            plan_id, win_name, at, percent, used, limit_est, source, resets_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            plan_id,
            window.as_str(),
            &at_str,
            percent,
            measurement.used,
            limit_est,
            source.as_str(),
            resets_str.as_deref(),
        ],
    )?;

    let meta = load_calibration_meta(conn, plan_id, window)?;
    Ok(meta.limit)
}

/// Calcula el estado actual de todas las ventanas de `plan_id` en el instante `now`.
pub fn window_status(
    conn: &Connection,
    plan_id: &str,
    now: DateTime<Utc>,
) -> Result<Vec<WindowStatus>> {
    let plan = get_plan(conn, plan_id)?;
    if plan.windows.is_empty() {
        return Ok(Vec::new());
    }

    let events = load_plan_events(conn, plan_id, now)?;
    let mut statuses = Vec::with_capacity(plan.windows.len());

    for spec in &plan.windows {
        let measurement = measure_window(&events, spec.name, now);
        let cal = load_calibration_meta(conn, plan_id, spec.name)?;

        let percent = cal.limit.and_then(|lim| {
            if lim > 0.0 {
                Some(measurement.used / lim * 100.0)
            } else {
                None
            }
        });

        let official_reset = cal.latest_agy.and_then(|(agy_at, agy_reset)| {
            let age = now.signed_duration_since(agy_at);
            if age >= Duration::zero() && age < Duration::minutes(20) {
                agy_reset
            } else {
                None
            }
        });
        let resets_at = official_reset.or(measurement.app_resets_at);

        let exhausts_at = cal.limit.and_then(|lim| {
            let remaining = lim - measurement.used;
            if remaining <= 0.0 || measurement.recent_30m_used <= 0.0 {
                return None;
            }
            let secs = (remaining / measurement.recent_30m_used) * 1800.0;
            if !secs.is_finite() || secs < 0.0 {
                return None;
            }
            let eta = now + Duration::milliseconds((secs * 1000.0).round() as i64);
            if resets_at.is_none_or(|end| eta < end) {
                Some(eta)
            } else {
                None
            }
        });

        statuses.push(WindowStatus {
            name: spec.name,
            used: measurement.used,
            limit: cal.limit,
            percent,
            resets_at,
            exhausts_at,
            unpriced_events: measurement.unpriced_events,
            calibrated_by: cal.calibrated_by,
            calibrated_at: cal.calibrated_at,
        });
    }

    Ok(statuses)
}

pub(crate) fn active_5h_block_start(
    conn: &Connection,
    plan_id: &str,
    now: DateTime<Utc>,
) -> Result<Option<DateTime<Utc>>> {
    let events = load_plan_events(conn, plan_id, now)?;
    let points: Vec<(DateTime<Utc>, f64)> = events
        .into_iter()
        .map(|ev| (ev.ts, ev.cost_usd.unwrap_or(0.0)))
        .collect();
    Ok(current_block(&points, now, 5).map(|b| b.start))
}

struct PlanEvent {
    ts: DateTime<Utc>,
    cost_usd: Option<f64>,
}

struct WindowMeasurement {
    used: f64,
    unpriced_events: u64,
    app_resets_at: Option<DateTime<Utc>>,
    recent_30m_used: f64,
}

struct CalibrationMeta {
    limit: Option<f64>,
    calibrated_by: Option<CalibrationSource>,
    calibrated_at: Option<DateTime<Utc>>,
    latest_agy: Option<(DateTime<Utc>, Option<DateTime<Utc>>)>,
}

fn build_plan(
    id: String,
    name: String,
    provider_str: &str,
    kind_str: &str,
    monthly_fee_usd: f64,
    budget_usd: Option<f64>,
    windows_json: &str,
) -> Result<Plan> {
    let provider: Provider = provider_str.parse()?;
    let kind: PlanKind = kind_str.parse()?;
    let windows: Vec<PlanWindowSpec> =
        serde_json::from_str(windows_json).map_err(|err| CoreError::Parse {
            file: "plan".to_string(),
            line: 0,
            reason: format!("windows JSON inválido en plan '{id}': {err}"),
        })?;
    Ok(Plan {
        id,
        name,
        provider,
        kind,
        monthly_fee_usd,
        budget_usd,
        windows,
    })
}

fn load_plan_events(
    conn: &Connection,
    plan_id: &str,
    now: DateTime<Utc>,
) -> Result<Vec<PlanEvent>> {
    // 240 h cubren la ventana semanal (168 h) más margen suficiente para estabilizar la cadena de bloques de 5 h.
    let lookback_start = fmt_utc(now - Duration::hours(240));
    let now_str = fmt_utc(now);
    let mut stmt = conn.prepare(
        "SELECT ts_utc, cost_usd
         FROM usage_event
         WHERE plan_id = ?1 AND ts_utc >= ?2 AND ts_utc <= ?3
         ORDER BY ts_utc ASC, dedup_key ASC",
    )?;
    let rows = stmt.query_map(params![plan_id, &lookback_start, &now_str], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, Option<f64>>(1)?))
    })?;

    let mut events = Vec::new();
    for row in rows {
        let (ts_str, cost_usd) = row?;
        let ts = parse_utc(&ts_str, "usage_event")?;
        events.push(PlanEvent { ts, cost_usd });
    }
    Ok(events)
}

fn measure_window(
    events: &[PlanEvent],
    window: WindowName,
    now: DateTime<Utc>,
) -> WindowMeasurement {
    let recent_start = now - Duration::minutes(30);

    match window {
        WindowName::FiveHours => {
            let points: Vec<(DateTime<Utc>, f64)> = events
                .iter()
                .map(|ev| (ev.ts, ev.cost_usd.unwrap_or(0.0)))
                .collect();
            let Some(block) = current_block(&points, now, 5) else {
                return WindowMeasurement {
                    used: 0.0,
                    unpriced_events: 0,
                    app_resets_at: None,
                    recent_30m_used: 0.0,
                };
            };

            let mut unpriced_events = 0_u64;
            let mut recent_30m_used = 0.0_f64;
            for ev in events {
                if ev.ts < block.start || ev.ts > now {
                    continue;
                }
                if ev.cost_usd.is_none() {
                    unpriced_events += 1;
                }
                if ev.ts >= recent_start {
                    recent_30m_used += ev.cost_usd.unwrap_or(0.0);
                }
            }

            WindowMeasurement {
                used: block.used,
                unpriced_events,
                app_resets_at: Some(block.end),
                recent_30m_used,
            }
        }
        WindowName::Weekly => {
            let week_start = now - Duration::hours(168);
            let mut used = 0.0_f64;
            let mut unpriced_events = 0_u64;
            let mut recent_30m_used = 0.0_f64;

            for ev in events {
                if ev.ts <= week_start || ev.ts > now {
                    continue;
                }
                match ev.cost_usd {
                    Some(c) => used += c,
                    None => unpriced_events += 1,
                }
                if ev.ts >= recent_start {
                    recent_30m_used += ev.cost_usd.unwrap_or(0.0);
                }
            }

            WindowMeasurement {
                used,
                unpriced_events,
                app_resets_at: None,
                recent_30m_used,
            }
        }
    }
}

fn load_calibration_meta(
    conn: &Connection,
    plan_id: &str,
    window: WindowName,
) -> Result<CalibrationMeta> {
    let win_str = window.as_str();
    let latest_overall: Option<(String, String)> = conn
        .query_row(
            "SELECT source, at
             FROM plan_calibration
             WHERE plan_id = ?1 AND win_name = ?2
             ORDER BY at DESC, id DESC
             LIMIT 1",
            params![plan_id, win_str],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;

    let (calibrated_by, calibrated_at, limit) = match latest_overall {
        None => (None, None, None),
        Some((source_str, at_str)) => {
            let source: CalibrationSource = source_str.parse()?;
            let at = parse_utc(&at_str, "plan_calibration")?;
            let avg: Option<f64> = match source {
                CalibrationSource::Agy => conn.query_row(
                    "SELECT AVG(limit_est) FROM (
                         SELECT limit_est
                         FROM plan_calibration
                         WHERE plan_id = ?1 AND win_name = ?2 AND source = 'agy'
                         ORDER BY at DESC, id DESC
                         LIMIT 8
                     )",
                    params![plan_id, win_str],
                    |r| r.get(0),
                )?,
                CalibrationSource::User => conn.query_row(
                    "SELECT AVG(limit_est)
                     FROM plan_calibration
                     WHERE plan_id = ?1 AND win_name = ?2 AND source = 'user'",
                    params![plan_id, win_str],
                    |r| r.get(0),
                )?,
            };
            (Some(source), Some(at), avg)
        }
    };

    let latest_agy_row: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT at, resets_at
             FROM plan_calibration
             WHERE plan_id = ?1 AND win_name = ?2 AND source = 'agy'
             ORDER BY at DESC, id DESC
             LIMIT 1",
            params![plan_id, win_str],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;

    let latest_agy = match latest_agy_row {
        None => None,
        Some((at_str, resets_str)) => {
            let at = parse_utc(&at_str, "plan_calibration")?;
            let resets_at = resets_str
                .as_deref()
                .map(|s| parse_utc(s, "plan_calibration"))
                .transpose()?;
            Some((at, resets_at))
        }
    };

    Ok(CalibrationMeta {
        limit,
        calibrated_by,
        calibrated_at,
        latest_agy,
    })
}

fn parse_utc(s: &str, context: &str) -> Result<DateTime<Utc>> {
    s.parse::<DateTime<Utc>>().map_err(|err| CoreError::Parse {
        file: context.to_string(),
        line: 0,
        reason: format!("fecha UTC inválida '{s}': {err}"),
    })
}
