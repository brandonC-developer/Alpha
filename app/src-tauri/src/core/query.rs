use std::collections::{BTreeMap, HashMap};
use std::hash::{BuildHasherDefault, Hasher};
use std::time::Instant;

use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Timelike, Utc};
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, params_from_iter};
use serde::{Deserialize, Serialize, Serializer};

#[derive(Default)]
struct FastHasher(u64);

impl Hasher for FastHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut h = 0xcbf2_9ce4_8422_2325_u64 ^ self.0;
        for &b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        self.0 = h;
    }
}

type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<FastHasher>>;

use crate::cost::CostKind;
use crate::error::{CoreError, Result};
use crate::model::{Provider, Tool};
use crate::time::{fmt_utc, local_date};

fn serialize_utc<S: Serializer>(
    dt: &DateTime<Utc>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error> {
    serializer.serialize_str(&fmt_utc(*dt))
}

/// Filtros comunes para las consultas de uso, costes y sesiones.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Filter {
    #[serde(serialize_with = "serialize_utc")]
    pub from: DateTime<Utc>,
    #[serde(serialize_with = "serialize_utc")]
    pub to: DateTime<Utc>,
    pub tool: Option<Tool>,
    pub provider: Option<Provider>,
    pub project: Option<String>,
}

/// Desglose de tokens por categoría (sin incluir razonamiento aparte porque va en `output`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tokens {
    pub input: u64,
    pub cache_write: u64,
    pub cache_read: u64,
    pub output: u64,
}

impl Tokens {
    fn input_total(&self) -> u64 {
        self.input + self.cache_write + self.cache_read
    }

    fn total(&self) -> u64 {
        self.input_total() + self.output
    }

    fn add_assign(&mut self, other: Self) {
        self.input += other.input;
        self.cache_write += other.cache_write;
        self.cache_read += other.cache_read;
        self.output += other.output;
    }
}

/// Indicador numérico con variación respecto al periodo anterior y serie diaria.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Kpi {
    pub value: f64,
    pub delta_pct: Option<f64>,
    pub spark: Vec<f64>,
}

/// Bloque de KPIs del Panel principal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardKpis {
    pub input_total: Kpi,
    pub cache_pct: f64,
    pub output: Kpi,
    pub output_per_turn: f64,
    pub turns: Kpi,
    pub sessions: u64,
    pub active_secs: Kpi,
    pub max_concurrency: u64,
}

/// Totales de entrada y salida para un día local.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayTokens {
    pub date: String,
    pub input_total: u64,
    pub output: u64,
}

/// Celda máxima del mapa de calor semanal (`weekday`: 0 = lunes .. 6 = domingo).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeatmapPeak {
    pub weekday: u8,
    pub hour: u8,
}

/// Uso agregado por modelo en el Panel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsage {
    pub model: String,
    pub provider: Provider,
    pub tokens: Tokens,
    pub turns: u64,
    pub cost_usd: Option<f64>,
}

/// Cuota porcentual de tokens totales por proveedor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderShare {
    pub provider: Provider,
    pub pct: f64,
}

/// Uso agregado por herramienta en el Panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolUsage {
    pub tool: Tool,
    pub tokens: Tokens,
    pub active_secs: u64,
}

/// Datos completos de la pantalla Panel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    pub kpis: DashboardKpis,
    pub by_day: Vec<DayTokens>,
    pub days_with_use: u64,
    pub heatmap: Vec<Vec<u64>>,
    pub peak: Option<HeatmapPeak>,
    pub by_model: Vec<ModelUsage>,
    pub by_provider: Vec<ProviderShare>,
    pub by_tool: Vec<ToolUsage>,
}

/// Rendimiento equivalente API frente a cuota mensual prorrateada por suscripción.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanYield {
    pub plan_id: String,
    pub name: String,
    pub fee_usd: f64,
    pub equivalent_usd: f64,
    pub ratio: Option<f64>,
}

/// Coste diario desglosado por proveedor y tipo de coste.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayCost {
    pub date: String,
    pub provider: Provider,
    pub kind: CostKind,
    pub usd: f64,
}

/// Coste agregado por modelo y tipo de coste.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCost {
    pub model: String,
    pub kind: CostKind,
    pub usd: f64,
    pub turns: u64,
}

/// Coste agregado por herramienta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCost {
    pub tool: Tool,
    pub usd: f64,
}

/// Coste agregado por proyecto con número de sesiones y media por sesión.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCost {
    pub project: String,
    pub usd: Option<f64>,
    pub sessions: u64,
    pub avg_per_session: Option<f64>,
}

/// Proyección del gasto real y equivalente API al cierre del mes local en curso.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonthProjection {
    pub real_usd: f64,
    pub api_equivalent_usd: f64,
}

/// Estado del presupuesto mensual de DeepSeek.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepSeekBudget {
    pub budget_usd: f64,
    pub spent_usd: f64,
    pub balance_usd: Option<f64>,
}

/// Eventos sin tarifa aplicable agrupados por modelo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnpricedModel {
    pub model: String,
    pub events: u64,
}

/// Datos completos de la pantalla Costes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CostsView {
    pub real_usd: f64,
    pub api_equivalent_usd: f64,
    pub fees_usd: f64,
    #[serde(rename = "yield")]
    pub r#yield: Vec<PlanYield>,
    pub by_day: Vec<DayCost>,
    pub by_model: Vec<ModelCost>,
    pub by_tool: Vec<ToolCost>,
    pub by_project: Vec<ProjectCost>,
    pub month_projection: MonthProjection,
    pub deepseek_budget: Option<DeepSeekBudget>,
    pub unpriced: Vec<UnpricedModel>,
}

/// Fila resumida de la pantalla Sesiones.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRow {
    pub session_id: String,
    pub tool: Tool,
    pub project: Option<String>,
    pub main_model: String,
    #[serde(serialize_with = "serialize_utc")]
    pub start: DateTime<Utc>,
    #[serde(serialize_with = "serialize_utc")]
    pub end: DateTime<Utc>,
    pub active_secs: u64,
    pub turns: u64,
    pub tokens: Tokens,
    pub cost_usd: Option<f64>,
    pub kind: CostKind,
    pub anomalous: bool,
}

/// Llamada individual dentro del desglose de una sesión.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallRow {
    #[serde(serialize_with = "serialize_utc")]
    pub ts: DateTime<Utc>,
    pub model: String,
    pub tokens: Tokens,
    pub reasoning: u64,
    pub cost_usd: Option<f64>,
    pub kind: CostKind,
}

struct AggRow {
    hour_utc: DateTime<Utc>,
    tool: Tool,
    provider: Provider,
    plan_id: String,
    model: String,
    project: String,
    turns: u64,
    cost_real: f64,
    cost_equiv: f64,
    unpriced: u64,
}

struct PeriodTotals {
    input_total: u64,
    output: u64,
    turns: u64,
}

struct SessionMetrics {
    sessions: u64,
    total_active_secs: u64,
    active_by_day: HashMap<NaiveDate, u64>,
    active_by_tool: HashMap<Tool, u64>,
    max_concurrency: u64,
}

#[doc(hidden)]
#[derive(Debug, Default, Clone, Copy)]
pub struct DashboardProfile {
    pub hourly_agg_ms: f64,
    pub prev_totals_ms: f64,
    pub session_sql_ms: f64,
    pub session_calc_ms: f64,
    pub prev_active_ms: f64,
    pub aggregate_ms: f64,
    pub total_ms: f64,
}

/// Calcula los datos agregados de la pantalla Panel para el rango y filtros indicados.
pub fn dashboard<Tz: TimeZone>(conn: &Connection, f: &Filter, tz: &Tz) -> Result<Dashboard> {
    dashboard_inner(conn, f, tz, None)
}

#[doc(hidden)]
pub fn dashboard_profiled<Tz: TimeZone>(
    conn: &Connection,
    f: &Filter,
    tz: &Tz,
) -> Result<(Dashboard, DashboardProfile)> {
    let mut prof = DashboardProfile::default();
    let d = dashboard_inner(conn, f, tz, Some(&mut prof))?;
    Ok((d, prof))
}

fn dashboard_inner<Tz: TimeZone>(
    conn: &Connection,
    f: &Filter,
    tz: &Tz,
    mut prof: Option<&mut DashboardProfile>,
) -> Result<Dashboard> {
    let t_total = Instant::now();
    if f.from >= f.to {
        return Ok(empty_dashboard());
    }
    let f = &normalize_filter(f);

    let prev_filter = previous_filter(f);

    let t0 = Instant::now();
    let hourly = query_dashboard_hourly(conn, f, tz)?;
    if let Some(p) = prof.as_deref_mut() {
        p.hourly_agg_ms = t0.elapsed().as_secs_f64() * 1000.0;
    }

    let t1 = Instant::now();
    let prev_totals = query_hourly_totals(conn, &prev_filter)?;
    if let Some(p) = prof.as_deref_mut() {
        p.prev_totals_ms = t1.elapsed().as_secs_f64() * 1000.0;
    }

    let session_metrics = query_session_metrics(conn, f, tz, prof.as_deref_mut())?;

    let t3 = Instant::now();
    let prev_active_secs = query_total_active_secs(conn, &prev_filter)?;
    if let Some(p) = prof.as_deref_mut() {
        p.prev_active_ms = t3.elapsed().as_secs_f64() * 1000.0;
    }

    let t4 = Instant::now();

    let mut day_active = vec![0_u64; hourly.days.len()];
    for (d, secs) in &session_metrics.active_by_day {
        if let Some(&idx) = hourly.day_index.get(d) {
            day_active[idx] += *secs;
        }
    }

    let input_total_val = hourly.total_tokens.input_total();
    let cache_pct = if input_total_val > 0 {
        hourly.total_tokens.cache_read as f64 / input_total_val as f64 * 100.0
    } else {
        0.0
    };
    let output_per_turn = if hourly.total_turns > 0 {
        hourly.total_tokens.output as f64 / hourly.total_turns as f64
    } else {
        0.0
    };

    let by_day: Vec<DayTokens> = hourly
        .days
        .iter()
        .enumerate()
        .map(|(i, d)| DayTokens {
            date: d.format("%Y-%m-%d").to_string(),
            input_total: hourly.day_input_total[i],
            output: hourly.day_output[i],
        })
        .collect();
    let days_with_use = hourly.day_turns.iter().filter(|&&t| t > 0).count() as u64;

    let peak = find_heatmap_peak(&hourly.heatmap);

    let mut by_model: Vec<ModelUsage> = hourly
        .models
        .into_iter()
        .map(|m| ModelUsage {
            model: m.model,
            provider: m.provider,
            tokens: m.tokens,
            turns: m.turns,
            cost_usd: if m.unpriced == m.turns {
                None
            } else {
                Some(m.cost_sum)
            },
        })
        .collect();
    by_model.sort_by(|a, b| {
        b.tokens
            .total()
            .cmp(&a.tokens.total())
            .then_with(|| a.model.cmp(&b.model))
    });

    let grand_total = hourly.total_tokens.total();
    let mut by_provider: Vec<ProviderShare> = if grand_total > 0 {
        [
            Provider::Anthropic,
            Provider::Google,
            Provider::DeepSeek,
            Provider::Other,
        ]
        .into_iter()
        .enumerate()
        .filter_map(|(idx, provider)| {
            let t = hourly.provider_totals[idx];
            if t > 0 {
                Some(ProviderShare {
                    provider,
                    pct: t as f64 / grand_total as f64 * 100.0,
                })
            } else {
                None
            }
        })
        .collect()
    } else {
        Vec::new()
    };
    by_provider.sort_by(|a, b| {
        b.pct
            .total_cmp(&a.pct)
            .then_with(|| a.provider.as_str().cmp(b.provider.as_str()))
    });

    let mut by_tool: Vec<ToolUsage> = [Tool::ClaudeCode, Tool::Antigravity, Tool::Opencode]
        .into_iter()
        .enumerate()
        .filter_map(|(idx, tool)| {
            if hourly.tool_seen[idx] {
                Some(ToolUsage {
                    tool,
                    tokens: hourly.tool_tokens[idx],
                    active_secs: *session_metrics.active_by_tool.get(&tool).unwrap_or(&0),
                })
            } else {
                None
            }
        })
        .collect();
    by_tool.sort_by(|a, b| {
        b.tokens
            .total()
            .cmp(&a.tokens.total())
            .then_with(|| a.tool.as_str().cmp(b.tool.as_str()))
    });

    if let Some(p) = prof {
        p.aggregate_ms = t4.elapsed().as_secs_f64() * 1000.0;
        p.total_ms = t_total.elapsed().as_secs_f64() * 1000.0;
    }

    Ok(Dashboard {
        kpis: DashboardKpis {
            input_total: Kpi {
                value: input_total_val as f64,
                delta_pct: delta_pct(input_total_val as f64, prev_totals.input_total as f64),
                spark: hourly
                    .day_input_total
                    .into_iter()
                    .map(|v| v as f64)
                    .collect(),
            },
            cache_pct,
            output: Kpi {
                value: hourly.total_tokens.output as f64,
                delta_pct: delta_pct(hourly.total_tokens.output as f64, prev_totals.output as f64),
                spark: hourly.day_output.into_iter().map(|v| v as f64).collect(),
            },
            output_per_turn,
            turns: Kpi {
                value: hourly.total_turns as f64,
                delta_pct: delta_pct(hourly.total_turns as f64, prev_totals.turns as f64),
                spark: hourly.day_turns.into_iter().map(|v| v as f64).collect(),
            },
            sessions: session_metrics.sessions,
            active_secs: Kpi {
                value: session_metrics.total_active_secs as f64,
                delta_pct: delta_pct(
                    session_metrics.total_active_secs as f64,
                    prev_active_secs as f64,
                ),
                spark: day_active.into_iter().map(|v| v as f64).collect(),
            },
            max_concurrency: session_metrics.max_concurrency,
        },
        by_day,
        days_with_use,
        heatmap: hourly.heatmap,
        peak,
        by_model,
        by_provider,
        by_tool,
    })
}

/// Calcula los datos económicos de la pantalla Costes para el rango y la fecha actuales.
pub fn costs<Tz: TimeZone>(
    conn: &Connection,
    f: &Filter,
    tz: &Tz,
    now: DateTime<Utc>,
) -> Result<CostsView> {
    let norm;
    let f = if f.from < f.to {
        norm = normalize_filter(f);
        &norm
    } else {
        f
    };

    let rows = if f.from < f.to {
        query_hourly_agg(conn, f)?
    } else {
        Vec::new()
    };

    let mut real_usd = 0.0_f64;
    let mut api_equivalent_usd = 0.0_f64;

    let mut equiv_by_plan: HashMap<String, f64> = HashMap::new();
    let mut day_costs: HashMap<(String, Provider, bool), f64> = HashMap::new();

    #[derive(Default)]
    struct ModelCostAcc {
        real_usd: f64,
        real_turns: u64,
        equiv_usd: f64,
        equiv_turns: u64,
        unknown_turns: u64,
    }
    let mut by_model_acc: HashMap<String, ModelCostAcc> = HashMap::new();
    let mut by_tool_acc: HashMap<Tool, (f64, u64)> = HashMap::new();
    let mut by_project_acc: HashMap<String, (f64, u64)> = HashMap::new();
    let mut unpriced_acc: HashMap<String, u64> = HashMap::new();

    for row in &rows {
        real_usd += row.cost_real;
        api_equivalent_usd += row.cost_equiv;
        *equiv_by_plan.entry(row.plan_id.clone()).or_insert(0.0) += row.cost_equiv;

        let priced_turns = row.turns.saturating_sub(row.unpriced);
        let is_payg = row.plan_id == "deepseek";

        if priced_turns > 0 {
            let local_day = local_date(row.hour_utc, tz).format("%Y-%m-%d").to_string();
            let usd = if is_payg {
                row.cost_real
            } else {
                row.cost_equiv
            };
            *day_costs
                .entry((local_day, row.provider, is_payg))
                .or_insert(0.0) += usd;
        }

        let m = by_model_acc.entry(row.model.clone()).or_default();
        if priced_turns > 0 {
            if is_payg {
                m.real_usd += row.cost_real;
                m.real_turns += priced_turns;
            } else {
                m.equiv_usd += row.cost_equiv;
                m.equiv_turns += priced_turns;
            }
        }
        m.unknown_turns += row.unpriced;

        let t = by_tool_acc.entry(row.tool).or_insert((0.0, 0));
        t.0 += row.cost_real + row.cost_equiv;
        t.1 += priced_turns;

        let p = by_project_acc
            .entry(row.project.clone())
            .or_insert((0.0, 0));
        if priced_turns > 0 {
            p.0 += row.cost_real + row.cost_equiv;
            p.1 += priced_turns;
        }

        if row.unpriced > 0 {
            *unpriced_acc.entry(row.model.clone()).or_insert(0) += row.unpriced;
        }
    }

    let range_days = fee_range_days(f, &rows);
    let yield_rows = compute_plan_yields(conn, range_days, &equiv_by_plan)?;
    let fees_usd: f64 = yield_rows.iter().map(|y| y.fee_usd).sum();

    let mut by_day: Vec<DayCost> = day_costs
        .into_iter()
        .map(|((date, provider, is_payg), usd)| DayCost {
            date,
            provider,
            kind: if is_payg {
                CostKind::Real
            } else {
                CostKind::ApiEquivalent
            },
            usd,
        })
        .collect();
    by_day.sort_by(|a, b| {
        a.date
            .cmp(&b.date)
            .then_with(|| a.provider.as_str().cmp(b.provider.as_str()))
            .then_with(|| a.kind.as_str().cmp(b.kind.as_str()))
    });

    let mut by_model = Vec::new();
    for (model, acc) in by_model_acc {
        if acc.real_turns > 0 {
            by_model.push(ModelCost {
                model: model.clone(),
                kind: CostKind::Real,
                usd: acc.real_usd,
                turns: acc.real_turns,
            });
        }
        if acc.equiv_turns > 0 {
            by_model.push(ModelCost {
                model: model.clone(),
                kind: CostKind::ApiEquivalent,
                usd: acc.equiv_usd,
                turns: acc.equiv_turns,
            });
        }
        if acc.unknown_turns > 0 {
            by_model.push(ModelCost {
                model,
                kind: CostKind::Unknown,
                usd: 0.0,
                turns: acc.unknown_turns,
            });
        }
    }
    by_model.sort_by(|a, b| {
        b.usd
            .total_cmp(&a.usd)
            .then_with(|| b.turns.cmp(&a.turns))
            .then_with(|| a.model.cmp(&b.model))
    });

    let mut by_tool: Vec<ToolCost> = by_tool_acc
        .into_iter()
        .filter(|(_, (_, priced_turns))| *priced_turns > 0)
        .map(|(tool, (usd, _))| ToolCost { tool, usd })
        .collect();
    by_tool.sort_by(|a, b| {
        b.usd
            .total_cmp(&a.usd)
            .then_with(|| a.tool.as_str().cmp(b.tool.as_str()))
    });

    let sessions_by_project = if f.from < f.to {
        query_sessions_by_project(conn, f)?
    } else {
        HashMap::new()
    };
    let mut all_projects: BTreeMap<String, (f64, u64, u64, u64)> = BTreeMap::new();
    for (project, (usd, priced_turns)) in by_project_acc {
        let entry = all_projects.entry(project).or_insert((0.0, 0, 0, 0));
        entry.0 += usd;
        entry.1 += priced_turns;
    }
    for (project, (sessions, priced_sessions)) in sessions_by_project {
        let entry = all_projects.entry(project).or_insert((0.0, 0, 0, 0));
        entry.2 = sessions;
        entry.3 = priced_sessions;
    }
    let mut by_project: Vec<ProjectCost> = all_projects
        .into_iter()
        .map(
            |(project, (usd_sum, priced_turns, sessions, priced_sessions))| {
                let (usd, avg_per_session) = if priced_turns > 0 && priced_sessions > 0 {
                    (Some(usd_sum), Some(usd_sum / priced_sessions as f64))
                } else if priced_turns > 0 {
                    (Some(usd_sum), None)
                } else {
                    (None, None)
                };
                ProjectCost {
                    project,
                    usd,
                    sessions,
                    avg_per_session,
                }
            },
        )
        .collect();
    by_project.sort_by(|a, b| {
        match (a.usd, b.usd) {
            (Some(au), Some(bu)) => bu.total_cmp(&au),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
        .then_with(|| b.sessions.cmp(&a.sessions))
        .then_with(|| a.project.cmp(&b.project))
    });

    let mut unpriced: Vec<UnpricedModel> = unpriced_acc
        .into_iter()
        .map(|(model, events)| UnpricedModel { model, events })
        .collect();
    unpriced.sort_by(|a, b| b.events.cmp(&a.events).then_with(|| a.model.cmp(&b.model)));

    let (month_projection, deepseek_budget) = compute_month_metrics(conn, f, tz, now)?;

    Ok(CostsView {
        real_usd,
        api_equivalent_usd,
        fees_usd,
        r#yield: yield_rows,
        by_day,
        by_model,
        by_tool,
        by_project,
        month_projection,
        deepseek_budget,
        unpriced,
    })
}

/// Devuelve la lista de sesiones que coinciden con `f`, marcando las anómalas.
pub fn sessions(conn: &Connection, f: &Filter) -> Result<Vec<SessionRow>> {
    if f.from >= f.to {
        return Ok(Vec::new());
    }
    let f = &normalize_filter(f);

    let (where_sql, params) = build_event_where(f);
    let sql = format!(
        "SELECT session_id, tool, project, model, ts_utc,
                input, cache_write, cache_read, output, cost_usd, cost_kind
         FROM usage_event
         WHERE {where_sql}
         ORDER BY tool ASC, session_id ASC, ts_utc ASC, dedup_key ASC"
    );

    struct SessionAcc {
        session_id: String,
        tool: Tool,
        project: Option<String>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        last_ts: DateTime<Utc>,
        active_secs: u64,
        turns: u64,
        tokens: Tokens,
        real_sum: f64,
        real_count: u64,
        equiv_sum: f64,
        equiv_count: u64,
        model_tokens: HashMap<String, (u64, u64)>,
    }

    let mut grouped: Vec<SessionAcc> = Vec::new();
    let mut index_by_key: HashMap<(String, Tool), usize> = HashMap::new();

    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params_from_iter(params))?;
    while let Some(r) = rows.next()? {
        let session_id: String = r.get(0)?;
        let tool: Tool = r.get::<_, String>(1)?.parse()?;
        let project: Option<String> = r.get(2)?;
        let model: String = r.get(3)?;
        let ts_str: String = r.get(4)?;
        let ts = parse_utc(&ts_str, "usage_event")?;
        let tokens = Tokens {
            input: r.get::<_, i64>(5)?.max(0) as u64,
            cache_write: r.get::<_, i64>(6)?.max(0) as u64,
            cache_read: r.get::<_, i64>(7)?.max(0) as u64,
            output: r.get::<_, i64>(8)?.max(0) as u64,
        };
        let cost_usd: Option<f64> = r.get(9)?;
        let cost_kind: CostKind = r.get::<_, String>(10)?.parse()?;

        let key = (session_id.clone(), tool);
        if let Some(&idx) = index_by_key.get(&key) {
            let acc = &mut grouped[idx];
            if acc.project.is_none() && project.is_some() {
                acc.project = project;
            }
            if ts < acc.start {
                acc.start = ts;
            }
            if ts > acc.end {
                acc.end = ts;
            }
            let gap = (ts - acc.last_ts).num_seconds().clamp(0, 300) as u64;
            acc.active_secs += gap;
            acc.last_ts = ts;
            acc.turns += 1;
            acc.tokens.add_assign(tokens);
            accumulate_session_cost(
                cost_usd,
                cost_kind,
                &mut acc.real_sum,
                &mut acc.real_count,
                &mut acc.equiv_sum,
                &mut acc.equiv_count,
            );
            let mt = acc.model_tokens.entry(model).or_insert((0, 0));
            mt.0 += tokens.total();
            mt.1 += 1;
        } else {
            let mut model_tokens = HashMap::new();
            model_tokens.insert(model, (tokens.total(), 1));
            let mut real_sum = 0.0;
            let mut real_count = 0;
            let mut equiv_sum = 0.0;
            let mut equiv_count = 0;
            accumulate_session_cost(
                cost_usd,
                cost_kind,
                &mut real_sum,
                &mut real_count,
                &mut equiv_sum,
                &mut equiv_count,
            );
            let idx = grouped.len();
            index_by_key.insert(key, idx);
            grouped.push(SessionAcc {
                session_id,
                tool,
                project,
                start: ts,
                end: ts,
                last_ts: ts,
                active_secs: 0,
                turns: 1,
                tokens,
                real_sum,
                real_count,
                equiv_sum,
                equiv_count,
                model_tokens,
            });
        }
    }

    let mut tools_present: Vec<Tool> = grouped.iter().map(|s| s.tool).collect();
    tools_present.sort_by_key(|t| t.as_str());
    tools_present.dedup();

    let mut anomaly_map: HashMap<(Tool, String), bool> = HashMap::new();
    for tool in tools_present {
        compute_anomalies_for_tool(conn, tool, &mut anomaly_map)?;
    }

    let mut result: Vec<SessionRow> = grouped
        .into_iter()
        .map(|acc| {
            let main_model = acc
                .model_tokens
                .into_iter()
                .max_by(|(m1, (t1, c1)), (m2, (t2, c2))| {
                    t1.cmp(t2).then_with(|| c1.cmp(c2)).then_with(|| m2.cmp(m1))
                })
                .map(|(m, _)| m)
                .unwrap_or_default();

            let (cost_usd, kind) = match (acc.real_count, acc.equiv_count) {
                (0, 0) => (None, CostKind::Unknown),
                (r, 0) if r > 0 => (Some(acc.real_sum), CostKind::Real),
                (0, e) if e > 0 => (Some(acc.equiv_sum), CostKind::ApiEquivalent),
                _ => (Some(acc.real_sum + acc.equiv_sum), CostKind::Real),
            };

            let anomalous = *anomaly_map
                .get(&(acc.tool, acc.session_id.clone()))
                .unwrap_or(&false);

            SessionRow {
                session_id: acc.session_id,
                tool: acc.tool,
                project: acc.project,
                main_model,
                start: acc.start,
                end: acc.end,
                active_secs: acc.active_secs,
                turns: acc.turns,
                tokens: acc.tokens,
                cost_usd,
                kind,
                anomalous,
            }
        })
        .collect();

    result.sort_by(|a, b| {
        b.start
            .cmp(&a.start)
            .then_with(|| a.session_id.cmp(&b.session_id))
    });

    Ok(result)
}

/// Devuelve el desglose cronológico de llamadas de una sesión sin contenido privado.
pub fn session_detail(conn: &Connection, session_id: &str) -> Result<Vec<CallRow>> {
    let mut stmt = conn.prepare(
        "SELECT ts_utc, model, input, cache_write, cache_read, output, reasoning, cost_usd, cost_kind
         FROM usage_event
         WHERE session_id = ?1
         ORDER BY ts_utc ASC, dedup_key ASC",
    )?;
    let rows = stmt.query_map([session_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, i64>(5)?,
            r.get::<_, i64>(6)?,
            r.get::<_, Option<f64>>(7)?,
            r.get::<_, String>(8)?,
        ))
    })?;

    let mut calls = Vec::new();
    for row in rows {
        let (ts_str, model, input, cache_write, cache_read, output, reasoning, cost_usd, kind_str) =
            row?;
        calls.push(CallRow {
            ts: parse_utc(&ts_str, "usage_event")?,
            model,
            tokens: Tokens {
                input: input.max(0) as u64,
                cache_write: cache_write.max(0) as u64,
                cache_read: cache_read.max(0) as u64,
                output: output.max(0) as u64,
            },
            reasoning: reasoning.max(0) as u64,
            cost_usd,
            kind: kind_str.parse()?,
        });
    }
    Ok(calls)
}

fn empty_dashboard() -> Dashboard {
    Dashboard {
        kpis: DashboardKpis {
            input_total: Kpi {
                value: 0.0,
                delta_pct: None,
                spark: Vec::new(),
            },
            cache_pct: 0.0,
            output: Kpi {
                value: 0.0,
                delta_pct: None,
                spark: Vec::new(),
            },
            output_per_turn: 0.0,
            turns: Kpi {
                value: 0.0,
                delta_pct: None,
                spark: Vec::new(),
            },
            sessions: 0,
            active_secs: Kpi {
                value: 0.0,
                delta_pct: None,
                spark: Vec::new(),
            },
            max_concurrency: 0,
        },
        by_day: Vec::new(),
        days_with_use: 0,
        heatmap: vec![vec![0; 24]; 7],
        peak: None,
        by_model: Vec::new(),
        by_provider: Vec::new(),
        by_tool: Vec::new(),
    }
}

fn floor_hour_utc(ts: DateTime<Utc>) -> DateTime<Utc> {
    ts.with_minute(0)
        .and_then(|d| d.with_second(0))
        .and_then(|d| d.with_nanosecond(0))
        .unwrap_or(ts)
}

fn ceil_hour_utc(ts: DateTime<Utc>) -> DateTime<Utc> {
    let floored = floor_hour_utc(ts);
    if ts == floored {
        floored
    } else {
        floored.checked_add_signed(Duration::hours(1)).unwrap_or(ts)
    }
}

fn normalize_filter(f: &Filter) -> Filter {
    if f.from >= f.to {
        return f.clone();
    }
    Filter {
        from: floor_hour_utc(f.from),
        to: ceil_hour_utc(f.to),
        tool: f.tool,
        provider: f.provider,
        project: f.project.clone(),
    }
}

fn previous_filter(f: &Filter) -> Filter {
    let dur = f.to - f.from;
    Filter {
        from: f.from - dur,
        to: f.from,
        tool: f.tool,
        provider: f.provider,
        project: f.project.clone(),
    }
}

fn delta_pct(current: f64, previous: f64) -> Option<f64> {
    if previous <= 0.0 {
        None
    } else {
        Some((current - previous) / previous * 100.0)
    }
}

fn find_heatmap_peak(heatmap: &[Vec<u64>]) -> Option<HeatmapPeak> {
    let mut best: Option<(u64, HeatmapPeak)> = None;
    for (wd, hours) in heatmap.iter().enumerate() {
        for (hr, &val) in hours.iter().enumerate() {
            if val == 0 {
                continue;
            }
            if best.is_none_or(|(best_val, _)| val > best_val) {
                best = Some((
                    val,
                    HeatmapPeak {
                        weekday: wd as u8,
                        hour: hr as u8,
                    },
                ));
            }
        }
    }
    best.map(|(_, peak)| peak)
}

fn build_hourly_where(f: &Filter) -> (String, Vec<Value>) {
    let mut clauses = vec!["hour_utc >= ?1".to_string(), "hour_utc < ?2".to_string()];
    let mut params = vec![Value::Text(fmt_utc(f.from)), Value::Text(fmt_utc(f.to))];
    push_dimension_filters(f, &mut clauses, &mut params, true);
    (clauses.join(" AND "), params)
}

fn build_event_where(f: &Filter) -> (String, Vec<Value>) {
    let mut clauses = vec!["ts_utc >= ?1".to_string(), "ts_utc < ?2".to_string()];
    let mut params = vec![Value::Text(fmt_utc(f.from)), Value::Text(fmt_utc(f.to))];
    push_dimension_filters(f, &mut clauses, &mut params, false);
    (clauses.join(" AND "), params)
}

fn push_dimension_filters(
    f: &Filter,
    clauses: &mut Vec<String>,
    params: &mut Vec<Value>,
    project_empty_string: bool,
) {
    if let Some(tool) = f.tool {
        let idx = params.len() + 1;
        clauses.push(format!("tool = ?{idx}"));
        params.push(Value::Text(tool.as_str().to_string()));
    }
    if let Some(provider) = f.provider {
        let idx = params.len() + 1;
        clauses.push(format!("provider = ?{idx}"));
        params.push(Value::Text(provider.as_str().to_string()));
    }
    if let Some(ref project) = f.project {
        let idx = params.len() + 1;
        if project_empty_string || !project.is_empty() {
            clauses.push(format!("project = ?{idx}"));
        } else {
            clauses.push(format!("COALESCE(project, '') = ?{idx}"));
        }
        params.push(Value::Text(project.clone()));
    }
}

struct DashboardModelAcc {
    model: String,
    provider: Provider,
    tokens: Tokens,
    turns: u64,
    cost_sum: f64,
    unpriced: u64,
}

struct DashboardHourly {
    days: Vec<NaiveDate>,
    day_index: HashMap<NaiveDate, usize>,
    day_input_total: Vec<u64>,
    day_output: Vec<u64>,
    day_turns: Vec<u64>,
    heatmap: Vec<Vec<u64>>,
    total_tokens: Tokens,
    total_turns: u64,
    models: Vec<DashboardModelAcc>,
    provider_totals: [u64; 4],
    tool_tokens: [Tokens; 3],
    tool_seen: [bool; 3],
}

fn build_day_axis_range<Tz: TimeZone>(
    conn: &Connection,
    f: &Filter,
    tz: &Tz,
    where_sql: &str,
    params: &[Value],
) -> Result<Vec<NaiveDate>> {
    let last_instant = f.to - Duration::nanoseconds(1);
    let end_date = local_date(last_instant, tz);
    let mut start_date = local_date(f.from, tz);

    if (end_date - start_date).num_days() > 366 {
        let min_sql = if where_sql.is_empty() {
            "SELECT MIN(hour_utc) FROM hourly_agg".to_string()
        } else {
            format!("SELECT MIN(hour_utc) FROM hourly_agg WHERE {where_sql}")
        };
        let mut min_stmt = conn.prepare_cached(&min_sql)?;
        let min_hour: Option<String> =
            min_stmt.query_row(params_from_iter(params.iter().cloned()), |r| r.get(0))?;
        let Some(min_str) = min_hour else {
            return Ok(Vec::new());
        };
        let min_dt = parse_utc(&min_str, "hourly_agg")?;
        let min_date = local_date(min_dt, tz);
        if min_date > start_date {
            start_date = min_date;
        }
    }

    let mut days = Vec::new();
    let mut d = start_date;
    while d <= end_date {
        days.push(d);
        let Some(next) = d.succ_opt() else {
            break;
        };
        d = next;
    }
    Ok(days)
}

fn build_dimension_where(f: &Filter) -> (String, Vec<Value>) {
    let mut clauses = Vec::new();
    let mut params = Vec::new();
    push_dimension_filters(f, &mut clauses, &mut params, true);
    (clauses.join(" AND "), params)
}

fn build_session_hour_where(f: &Filter, include_range: bool) -> (String, Vec<Value>) {
    let mut clauses = Vec::with_capacity(5);
    let mut params = Vec::with_capacity(5);
    if include_range {
        clauses.push("hour_utc >= ?1".to_string());
        clauses.push("hour_utc < ?2".to_string());
        params.push(Value::Text(fmt_utc(f.from)));
        params.push(Value::Text(fmt_utc(f.to)));
    }
    let idx = params.len() + 1;
    clauses.push(format!("provider = ?{idx}"));
    params.push(Value::Text(
        f.provider.map_or("*", |p| p.as_str()).to_string(),
    ));
    let idx = params.len() + 1;
    clauses.push(format!("project = ?{idx}"));
    params.push(Value::Text(f.project.as_deref().unwrap_or("*").to_string()));
    if let Some(tool) = f.tool {
        let idx = params.len() + 1;
        clauses.push(format!("tool = ?{idx}"));
        params.push(Value::Text(tool.as_str().to_string()));
    }
    (clauses.join(" AND "), params)
}

fn table_covers_full_range(
    conn: &Connection,
    table: &str,
    from_str: &str,
    to_str: &str,
) -> Result<(bool, bool)> {
    let (min_sql, max_sql) = match table {
        "hourly_agg" => (
            "SELECT MIN(hour_utc) FROM hourly_agg",
            "SELECT MAX(hour_utc) FROM hourly_agg",
        ),
        "session_hour" => (
            "SELECT MIN(hour_utc) FROM session_hour",
            "SELECT MAX(hour_utc) FROM session_hour",
        ),
        _ => unreachable!(),
    };
    let min_h: Option<String> = conn.prepare_cached(min_sql)?.query_row([], |r| r.get(0))?;
    let Some(min_h) = min_h else {
        return Ok((true, false));
    };
    if to_str <= min_h.as_str() {
        return Ok((true, false));
    }
    let max_h: Option<String> = conn.prepare_cached(max_sql)?.query_row([], |r| r.get(0))?;
    let Some(max_h) = max_h else {
        return Ok((true, false));
    };
    let empty_range = from_str > max_h.as_str();
    let full_range = from_str <= min_h.as_str() && max_h.as_str() < to_str;
    Ok((empty_range, full_range))
}

fn query_dashboard_hourly<Tz: TimeZone>(
    conn: &Connection,
    f: &Filter,
    tz: &Tz,
) -> Result<DashboardHourly> {
    let from_str = fmt_utc(f.from);
    let to_str = fmt_utc(f.to);
    let (empty_range, full_range) =
        table_covers_full_range(conn, "hourly_agg", &from_str, &to_str)?;

    let (where_sql, params) = if full_range {
        build_dimension_where(f)
    } else {
        build_hourly_where(f)
    };
    let days = if empty_range {
        build_day_axis_range(conn, f, tz, "1 = 0", &[])?
    } else {
        build_day_axis_range(conn, f, tz, &where_sql, &params)?
    };
    let day_index: HashMap<NaiveDate, usize> =
        days.iter().enumerate().map(|(i, d)| (*d, i)).collect();

    let mut day_input_total = vec![0_u64; days.len()];
    let mut day_output = vec![0_u64; days.len()];
    let mut day_turns = vec![0_u64; days.len()];
    let mut heatmap = vec![vec![0_u64; 24]; 7];
    let mut total_tokens = Tokens::default();
    let mut total_turns = 0_u64;

    let mut models: Vec<DashboardModelAcc> = Vec::new();
    let mut model_map: FastMap<String, [Option<usize>; 4]> = FastMap::default();
    let mut provider_totals = [0_u64; 4];
    let mut tool_tokens = [Tokens::default(); 3];
    let mut tool_seen = [false; 3];

    if empty_range {
        return Ok(DashboardHourly {
            days,
            day_index,
            day_input_total,
            day_output,
            day_turns,
            heatmap,
            total_tokens,
            total_turns,
            models,
            provider_totals,
            tool_tokens,
            tool_seen,
        });
    }

    let sql = if where_sql.is_empty() {
        "SELECT hour_utc, tool, provider, model,
                turns, input, cache_write, cache_read, output,
                cost_real, cost_equiv, unpriced
         FROM hourly_agg"
            .to_string()
    } else {
        format!(
            "SELECT hour_utc, tool, provider, model,
                    turns, input, cache_write, cache_read, output,
                    cost_real, cost_equiv, unpriced
             FROM hourly_agg
             WHERE {where_sql}"
        )
    };
    let mut stmt = conn.prepare_cached(&sql)?;
    let mut rows = stmt.query(params_from_iter(params))?;

    let mut hour_cache: FastMap<[u8; 13], (Option<usize>, usize, usize)> = FastMap::default();
    let mut last_hour: ([u8; 13], (Option<usize>, usize, usize)) = ([0; 13], (None, 0, 0));

    while let Some(r) = rows.next()? {
        let hour_bytes = col_bytes(r, 0)?;
        let tool_bytes = col_bytes(r, 1)?;
        let provider_bytes = col_bytes(r, 2)?;
        let model_str = col_str(r, 3)?;

        let turns = col_i64(r, 4)?.max(0) as u64;
        let tokens = Tokens {
            input: col_i64(r, 5)?.max(0) as u64,
            cache_write: col_i64(r, 6)?.max(0) as u64,
            cache_read: col_i64(r, 7)?.max(0) as u64,
            output: col_i64(r, 8)?.max(0) as u64,
        };
        let cost_real = col_f64(r, 9)?;
        let cost_equiv = col_f64(r, 10)?;
        let unpriced = col_i64(r, 11)?.max(0) as u64;

        let tool_idx = match tool_bytes {
            b"claude-code" => 0,
            b"antigravity" => 1,
            b"opencode" => 2,
            _ => return Err(col_str(r, 1)?.parse::<Tool>().unwrap_err()),
        };
        let (provider, provider_idx) = match provider_bytes {
            b"anthropic" => (Provider::Anthropic, 0),
            b"google" => (Provider::Google, 1),
            b"deepseek" => (Provider::DeepSeek, 2),
            b"other" => (Provider::Other, 3),
            _ => return Err(col_str(r, 2)?.parse::<Provider>().unwrap_err()),
        };

        let (day_opt, wd, hr) = if hour_bytes.len() >= 13 {
            let key: [u8; 13] = hour_bytes[..13].try_into().unwrap();
            if key == last_hour.0 {
                last_hour.1
            } else if let Some(&cached) = hour_cache.get(&key) {
                last_hour = (key, cached);
                cached
            } else {
                let hour_ep = fast_epoch_secs(hour_bytes)?;
                let hour_utc =
                    DateTime::from_timestamp(hour_ep, 0).ok_or_else(|| CoreError::Parse {
                        file: "hourly_agg".to_string(),
                        line: 0,
                        reason: format!("timestamp de hora inválido: {hour_ep}"),
                    })?;
                let local_dt = hour_utc.with_timezone(tz);
                let val = (
                    day_index.get(&local_dt.date_naive()).copied(),
                    local_dt.weekday().num_days_from_monday() as usize,
                    local_dt.hour() as usize,
                );
                hour_cache.insert(key, val);
                last_hour = (key, val);
                val
            }
        } else {
            let hour_utc = parse_utc(col_str(r, 0)?, "hourly_agg")?;
            let local_dt = hour_utc.with_timezone(tz);
            (
                day_index.get(&local_dt.date_naive()).copied(),
                local_dt.weekday().num_days_from_monday() as usize,
                local_dt.hour() as usize,
            )
        };

        let input_total = tokens.input_total();
        let total = input_total + tokens.output;

        if let Some(idx) = day_opt {
            day_input_total[idx] += input_total;
            day_output[idx] += tokens.output;
            day_turns[idx] += turns;
        }

        heatmap[wd][hr] += total;
        total_tokens.add_assign(tokens);
        total_turns += turns;
        provider_totals[provider_idx] += total;
        tool_seen[tool_idx] = true;
        tool_tokens[tool_idx].add_assign(tokens);

        if models.len() <= 16 {
            if let Some(acc) = models
                .iter_mut()
                .find(|m| m.provider == provider && m.model == model_str)
            {
                acc.tokens.add_assign(tokens);
                acc.turns += turns;
                acc.cost_sum += cost_real + cost_equiv;
                acc.unpriced += unpriced;
            } else {
                let m_idx = models.len();
                model_map.entry(model_str.to_owned()).or_insert([None; 4])[provider_idx] =
                    Some(m_idx);
                models.push(DashboardModelAcc {
                    model: model_str.to_owned(),
                    provider,
                    tokens,
                    turns,
                    cost_sum: cost_real + cost_equiv,
                    unpriced,
                });
            }
        } else {
            let slot = if let Some(slots) = model_map.get_mut(model_str) {
                slots
            } else {
                model_map.entry(model_str.to_owned()).or_insert([None; 4])
            };
            if let Some(m_idx) = slot[provider_idx] {
                let acc = &mut models[m_idx];
                acc.tokens.add_assign(tokens);
                acc.turns += turns;
                acc.cost_sum += cost_real + cost_equiv;
                acc.unpriced += unpriced;
            } else {
                let m_idx = models.len();
                slot[provider_idx] = Some(m_idx);
                models.push(DashboardModelAcc {
                    model: model_str.to_owned(),
                    provider,
                    tokens,
                    turns,
                    cost_sum: cost_real + cost_equiv,
                    unpriced,
                });
            }
        }
    }

    Ok(DashboardHourly {
        days,
        day_index,
        day_input_total,
        day_output,
        day_turns,
        heatmap,
        total_tokens,
        total_turns,
        models,
        provider_totals,
        tool_tokens,
        tool_seen,
    })
}

fn query_hourly_agg(conn: &Connection, f: &Filter) -> Result<Vec<AggRow>> {
    let (where_sql, params) = build_hourly_where(f);
    let sql = format!(
        "SELECT hour_utc, tool, provider, plan_id, model, project,
                turns, cost_real, cost_equiv, unpriced
         FROM hourly_agg
         WHERE {where_sql}"
    );
    let mut stmt = conn.prepare_cached(&sql)?;
    let mut rows = stmt.query(params_from_iter(params))?;
    let mut out = Vec::new();
    while let Some(r) = rows.next()? {
        let hour_str: String = r.get(0)?;
        let tool_str: String = r.get(1)?;
        let provider_str: String = r.get(2)?;
        out.push(AggRow {
            hour_utc: parse_utc(&hour_str, "hourly_agg")?,
            tool: tool_str.parse()?,
            provider: provider_str.parse()?,
            plan_id: r.get(3)?,
            model: r.get(4)?,
            project: r.get(5)?,
            turns: r.get::<_, i64>(6)?.max(0) as u64,
            cost_real: r.get(7)?,
            cost_equiv: r.get(8)?,
            unpriced: r.get::<_, i64>(9)?.max(0) as u64,
        });
    }
    Ok(out)
}

fn query_hourly_totals(conn: &Connection, f: &Filter) -> Result<PeriodTotals> {
    if f.from >= f.to {
        return Ok(PeriodTotals {
            input_total: 0,
            output: 0,
            turns: 0,
        });
    }
    let (where_sql, params) = build_hourly_where(f);
    let sql = format!(
        "SELECT COALESCE(SUM(input + cache_write + cache_read), 0),
                COALESCE(SUM(output), 0),
                COALESCE(SUM(turns), 0)
         FROM hourly_agg
         WHERE {where_sql}"
    );
    let mut stmt = conn.prepare_cached(&sql)?;
    let (input_total, output, turns): (i64, i64, i64) = stmt
        .query_row(params_from_iter(params), |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?;
    Ok(PeriodTotals {
        input_total: input_total.max(0) as u64,
        output: output.max(0) as u64,
        turns: turns.max(0) as u64,
    })
}

#[derive(Clone, Copy)]
struct InternSlot {
    hash: u64,
    offset: u32,
    len: u32,
    idx: u32,
}

struct SessionInterner {
    arena: Vec<u8>,
    slots: Vec<InternSlot>,
    count: usize,
}

impl SessionInterner {
    fn new() -> Self {
        Self {
            arena: Vec::with_capacity(4096),
            slots: vec![
                InternSlot {
                    hash: 0,
                    offset: 0,
                    len: 0,
                    idx: u32::MAX,
                };
                64
            ],
            count: 0,
        }
    }

    #[inline]
    fn hash_bytes(bytes: &[u8]) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325_u64;
        for &b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        h ^= h >> 33;
        h.wrapping_mul(0xff51_afd7_ed55_8ccd)
    }

    fn grow(&mut self) {
        let new_cap = self.slots.len() * 2;
        let mask = new_cap - 1;
        let mut new_slots = vec![
            InternSlot {
                hash: 0,
                offset: 0,
                len: 0,
                idx: u32::MAX,
            };
            new_cap
        ];
        for &slot in &self.slots {
            if slot.idx == u32::MAX {
                continue;
            }
            let mut pos = (slot.hash as usize) & mask;
            while new_slots[pos].idx != u32::MAX {
                pos = (pos + 1) & mask;
            }
            new_slots[pos] = slot;
        }
        self.slots = new_slots;
    }

    #[inline]
    fn intern(&mut self, bytes: &[u8]) -> (usize, bool) {
        if (self.count + 1) * 10 >= self.slots.len() * 7 {
            self.grow();
        }
        let hash = Self::hash_bytes(bytes);
        let mask = self.slots.len() - 1;
        let mut pos = (hash as usize) & mask;
        loop {
            let slot = self.slots[pos];
            if slot.idx == u32::MAX {
                let idx = self.count;
                let offset = self.arena.len() as u32;
                let len = bytes.len() as u32;
                self.arena.extend_from_slice(bytes);
                self.slots[pos] = InternSlot {
                    hash,
                    offset,
                    len,
                    idx: idx as u32,
                };
                self.count += 1;
                return (idx, true);
            }
            if slot.hash == hash && slot.len as usize == bytes.len() {
                let start = slot.offset as usize;
                let end = start + slot.len as usize;
                if &self.arena[start..end] == bytes {
                    return (slot.idx as usize, false);
                }
            }
            pos = (pos + 1) & mask;
        }
    }
}

#[derive(Clone, Copy)]
struct SessionHourEntry {
    hour_ep: i64,
    tool_idx: u8,
    first_ep: i64,
    last_ep: i64,
    active_secs: u64,
    minute_mask: u64,
}

struct SessionNode {
    next: u32,
    row: SessionHourEntry,
}

fn query_session_metrics<Tz: TimeZone>(
    conn: &Connection,
    f: &Filter,
    tz: &Tz,
    prof: Option<&mut DashboardProfile>,
) -> Result<SessionMetrics> {
    let t_sql = Instant::now();
    let from_str = fmt_utc(f.from);
    let to_str = fmt_utc(f.to);
    let (empty_range, full_range) =
        table_covers_full_range(conn, "session_hour", &from_str, &to_str)?;
    if empty_range {
        if let Some(p) = prof {
            p.session_sql_ms = t_sql.elapsed().as_secs_f64() * 1000.0;
        }
        return Ok(SessionMetrics {
            sessions: 0,
            total_active_secs: 0,
            active_by_day: HashMap::new(),
            active_by_tool: HashMap::new(),
            max_concurrency: 0,
        });
    }

    let (where_sql, params) = build_session_hour_where(f, !full_range);
    let sql = format!(
        "SELECT session_id, tool, hour_utc, first_ts, last_ts, active_secs, minute_mask
         FROM session_hour
         WHERE {where_sql}"
    );

    let mut stmt = conn.prepare_cached(&sql)?;
    let mut rows = stmt.query(params_from_iter(params))?;

    let mut interner = SessionInterner::new();
    let mut heads: Vec<u32> = Vec::new();
    let mut nodes: Vec<SessionNode> = Vec::new();
    let mut cached_hour: ([u8; 13], i64) = ([0; 13], 0);

    while let Some(r) = rows.next()? {
        let session_bytes = col_bytes(r, 0)?;
        let tool_bytes = col_bytes(r, 1)?;
        let hour_bytes = col_bytes(r, 2)?;
        let first_bytes = col_bytes(r, 3)?;
        let last_bytes = col_bytes(r, 4)?;
        let active_secs = col_i64(r, 5)?.max(0) as u64;
        let minute_mask = (col_i64(r, 6)?.max(0) as u64) & ((1_u64 << 60) - 1);

        let tool_idx = match tool_bytes {
            b"claude-code" => 0_u8,
            b"antigravity" => 1_u8,
            b"opencode" => 2_u8,
            _ => return Err(col_str(r, 1)?.parse::<Tool>().unwrap_err()),
        };

        let (hour_ep, first_ep, last_ep) =
            parse_session_hour_times(hour_bytes, first_bytes, last_bytes, &mut cached_hour)?;

        let entry = SessionHourEntry {
            hour_ep,
            tool_idx,
            first_ep,
            last_ep,
            active_secs,
            minute_mask,
        };

        let node_idx = nodes.len() as u32;
        let (s_idx, is_new) = interner.intern(session_bytes);
        if is_new {
            heads.push(node_idx);
            nodes.push(SessionNode {
                next: u32::MAX,
                row: entry,
            });
        } else {
            nodes.push(SessionNode {
                next: heads[s_idx],
                row: entry,
            });
            heads[s_idx] = node_idx;
        }
    }
    let sql_ms = t_sql.elapsed().as_secs_f64() * 1000.0;
    let t_calc = Instant::now();

    let sessions = heads.len() as u64;
    let mut total_active_secs = 0_u64;
    let mut active_by_tool_arr = [0_u64; 3];
    let mut active_by_hour: FastMap<i64, u64> = FastMap::default();
    let mut hour_minute_counts: FastMap<i64, [u32; 60]> = FastMap::default();
    let mut scratch: Vec<SessionHourEntry> = Vec::with_capacity(16);

    for head in heads {
        scratch.clear();
        let mut cur = head;
        while cur != u32::MAX {
            let n = &nodes[cur as usize];
            scratch.push(n.row);
            cur = n.next;
        }

        if scratch.len() == 1 {
            let e = &scratch[0];
            total_active_secs += e.active_secs;
            active_by_tool_arr[e.tool_idx as usize] += e.active_secs;
            if e.active_secs > 0 {
                *active_by_hour.entry(e.hour_ep).or_insert(0) += e.active_secs;
            }
            let mut mask = e.minute_mask;
            if mask != 0 {
                let counts = hour_minute_counts.entry(e.hour_ep).or_insert([0_u32; 60]);
                while mask != 0 {
                    let bit = mask.trailing_zeros() as usize;
                    counts[bit] += 1;
                    mask &= mask - 1;
                }
            }
            continue;
        }

        scratch.sort_unstable_by_key(|e| (e.first_ep, e.last_ep));

        let first = &scratch[0];
        total_active_secs += first.active_secs;
        active_by_tool_arr[first.tool_idx as usize] += first.active_secs;
        if first.active_secs > 0 {
            *active_by_hour.entry(first.hour_ep).or_insert(0) += first.active_secs;
        }

        let mut max_last_ep = first.last_ep;
        for curr in &scratch[1..] {
            let inter_gap = (curr.first_ep - max_last_ep).clamp(0, 300) as u64;
            max_last_ep = max_last_ep.max(curr.last_ep);
            let added = curr.active_secs + inter_gap;
            total_active_secs += added;
            active_by_tool_arr[curr.tool_idx as usize] += added;
            if added > 0 {
                *active_by_hour.entry(curr.hour_ep).or_insert(0) += added;
            }
        }

        scratch.sort_unstable_by_key(|e| e.hour_ep);
        let mut i = 0;
        while i < scratch.len() {
            let hour_ep = scratch[i].hour_ep;
            let mut mask = scratch[i].minute_mask;
            i += 1;
            while i < scratch.len() && scratch[i].hour_ep == hour_ep {
                mask |= scratch[i].minute_mask;
                i += 1;
            }
            if mask != 0 {
                let counts = hour_minute_counts.entry(hour_ep).or_insert([0_u32; 60]);
                while mask != 0 {
                    let bit = mask.trailing_zeros() as usize;
                    counts[bit] += 1;
                    mask &= mask - 1;
                }
            }
        }
    }

    let max_concurrency = hour_minute_counts
        .values()
        .flat_map(|c| c.iter())
        .copied()
        .max()
        .unwrap_or(0) as u64;

    let mut active_by_day: HashMap<NaiveDate, u64> = HashMap::new();
    for (hour_ep, secs) in active_by_hour {
        let hour_utc = DateTime::from_timestamp(hour_ep, 0).ok_or_else(|| CoreError::Parse {
            file: "session_hour".to_string(),
            line: 0,
            reason: format!("timestamp de hora inválido: {hour_ep}"),
        })?;
        let ld = local_date(hour_utc, tz);
        *active_by_day.entry(ld).or_insert(0) += secs;
    }

    let mut active_by_tool = HashMap::new();
    for (idx, tool) in [Tool::ClaudeCode, Tool::Antigravity, Tool::Opencode]
        .into_iter()
        .enumerate()
    {
        if active_by_tool_arr[idx] > 0 {
            active_by_tool.insert(tool, active_by_tool_arr[idx]);
        }
    }

    if let Some(p) = prof {
        p.session_sql_ms = sql_ms;
        p.session_calc_ms = t_calc.elapsed().as_secs_f64() * 1000.0;
    }

    Ok(SessionMetrics {
        sessions,
        total_active_secs,
        active_by_day,
        active_by_tool,
        max_concurrency,
    })
}

fn query_total_active_secs(conn: &Connection, f: &Filter) -> Result<u64> {
    if f.from >= f.to {
        return Ok(0);
    }
    let from_str = fmt_utc(f.from);
    let to_str = fmt_utc(f.to);
    let (empty_range, full_range) =
        table_covers_full_range(conn, "session_hour", &from_str, &to_str)?;
    if empty_range {
        return Ok(0);
    }
    let (where_sql, params) = build_session_hour_where(f, !full_range);
    let sql = format!(
        "SELECT session_id, hour_utc, first_ts, last_ts, active_secs
         FROM session_hour
         WHERE {where_sql}"
    );
    let mut stmt = conn.prepare_cached(&sql)?;
    let mut rows = stmt.query(params_from_iter(params))?;

    struct PrevNode {
        next: u32,
        first_ep: i64,
        last_ep: i64,
        active_secs: u64,
    }
    let mut interner = SessionInterner::new();
    let mut heads: Vec<u32> = Vec::new();
    let mut nodes: Vec<PrevNode> = Vec::new();
    let mut cached_hour: ([u8; 13], i64) = ([0; 13], 0);

    while let Some(r) = rows.next()? {
        let session_bytes = col_bytes(r, 0)?;
        let hour_bytes = col_bytes(r, 1)?;
        let first_bytes = col_bytes(r, 2)?;
        let last_bytes = col_bytes(r, 3)?;
        let active_secs = col_i64(r, 4)?.max(0) as u64;

        let (_, first_ep, last_ep) =
            parse_session_hour_times(hour_bytes, first_bytes, last_bytes, &mut cached_hour)?;

        let node_idx = nodes.len() as u32;
        let (s_idx, is_new) = interner.intern(session_bytes);
        if is_new {
            heads.push(node_idx);
            nodes.push(PrevNode {
                next: u32::MAX,
                first_ep,
                last_ep,
                active_secs,
            });
        } else {
            nodes.push(PrevNode {
                next: heads[s_idx],
                first_ep,
                last_ep,
                active_secs,
            });
            heads[s_idx] = node_idx;
        }
    }

    let mut total = 0_u64;
    let mut scratch: Vec<(i64, i64, u64)> = Vec::with_capacity(16);
    for head in heads {
        scratch.clear();
        let mut cur = head;
        while cur != u32::MAX {
            let n = &nodes[cur as usize];
            scratch.push((n.first_ep, n.last_ep, n.active_secs));
            cur = n.next;
        }
        if scratch.len() > 1 {
            scratch.sort_unstable_by_key(|e| (e.0, e.1));
        }
        total += scratch[0].2;
        let mut max_last_ep = scratch[0].1;
        for curr in &scratch[1..] {
            total += curr.2 + (curr.0 - max_last_ep).clamp(0, 300) as u64;
            max_last_ep = max_last_ep.max(curr.1);
        }
    }
    Ok(total)
}

#[inline]
fn parse_session_hour_times(
    hour_bytes: &[u8],
    first_bytes: &[u8],
    last_bytes: &[u8],
    cached_hour: &mut ([u8; 13], i64),
) -> Result<(i64, i64, i64)> {
    if hour_bytes.len() >= 13 {
        let hkey: [u8; 13] = hour_bytes[..13].try_into().unwrap();
        let hep = if hkey == cached_hour.0 {
            cached_hour.1
        } else {
            let ep = fast_epoch_secs(hour_bytes)?;
            *cached_hour = (hkey, ep);
            ep
        };
        let fep = if first_bytes.len() >= 19 && first_bytes[..13] == hkey {
            if let Some(off) = fast_min_sec(first_bytes) {
                hep + off
            } else {
                fast_epoch_secs(first_bytes)?
            }
        } else {
            fast_epoch_secs(first_bytes)?
        };
        let lep = if last_bytes.len() >= 19 && last_bytes[..13] == hkey {
            if let Some(off) = fast_min_sec(last_bytes) {
                hep + off
            } else {
                fast_epoch_secs(last_bytes)?
            }
        } else {
            fast_epoch_secs(last_bytes)?
        };
        Ok((hep, fep, lep))
    } else {
        Ok((
            fast_epoch_secs(hour_bytes)?,
            fast_epoch_secs(first_bytes)?,
            fast_epoch_secs(last_bytes)?,
        ))
    }
}

#[inline]
fn fast_min_sec(b: &[u8]) -> Option<i64> {
    if b[13] == b':'
        && b[16] == b':'
        && let (Some(mm), Some(ss)) = (parse_2(b, 14), parse_2(b, 17))
    {
        Some((mm as i64) * 60 + (ss as i64))
    } else {
        None
    }
}

#[inline]
fn col_str<'a>(row: &'a rusqlite::Row<'a>, idx: usize) -> Result<&'a str> {
    Ok(row.get_ref(idx)?.as_str().map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(idx, rusqlite::types::Type::Text, Box::new(e))
    })?)
}

#[inline]
fn col_bytes<'a>(row: &'a rusqlite::Row<'a>, idx: usize) -> Result<&'a [u8]> {
    Ok(row.get_ref(idx)?.as_bytes().map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(idx, rusqlite::types::Type::Text, Box::new(e))
    })?)
}

#[inline]
fn col_i64(row: &rusqlite::Row<'_>, idx: usize) -> Result<i64> {
    Ok(row.get_ref(idx)?.as_i64().map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(idx, rusqlite::types::Type::Integer, Box::new(e))
    })?)
}

#[inline]
fn col_f64(row: &rusqlite::Row<'_>, idx: usize) -> Result<f64> {
    Ok(row.get_ref(idx)?.as_f64().map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(idx, rusqlite::types::Type::Real, Box::new(e))
    })?)
}

fn fast_epoch_secs(b: &[u8]) -> Result<i64> {
    if b.len() >= 19
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[13] == b':'
        && b[16] == b':'
        && let (Some(y), Some(m), Some(d), Some(hh), Some(mm), Some(ss)) = (
            parse_4(b, 0),
            parse_2(b, 5),
            parse_2(b, 8),
            parse_2(b, 11),
            parse_2(b, 14),
            parse_2(b, 17),
        )
        && let Some(nd) = NaiveDate::from_ymd_opt(y, m, d)
        && let Some(ndt) = nd.and_hms_opt(hh, mm, ss)
    {
        return Ok(ndt.and_utc().timestamp());
    }
    let s = std::str::from_utf8(b).unwrap_or("");
    Ok(parse_utc(s, "usage_event")?.timestamp())
}

#[inline]
fn parse_2(b: &[u8], i: usize) -> Option<u32> {
    let d0 = b[i].wrapping_sub(b'0');
    let d1 = b[i + 1].wrapping_sub(b'0');
    if d0 < 10 && d1 < 10 {
        Some((d0 as u32) * 10 + (d1 as u32))
    } else {
        None
    }
}

#[inline]
fn parse_4(b: &[u8], i: usize) -> Option<i32> {
    let hi = parse_2(b, i)? as i32;
    let lo = parse_2(b, i + 2)? as i32;
    Some(hi * 100 + lo)
}

fn fee_range_days(f: &Filter, rows: &[AggRow]) -> f64 {
    if f.from >= f.to {
        return 0.0;
    }
    let span = f.to - f.from;
    if span <= Duration::days(366) {
        span.num_seconds() as f64 / 86_400.0
    } else if let Some(earliest) = rows.iter().map(|r| r.hour_utc).min() {
        ((f.to - earliest).num_seconds() as f64 / 86_400.0).max(1.0)
    } else {
        0.0
    }
}

fn compute_plan_yields(
    conn: &Connection,
    range_days: f64,
    equiv_by_plan: &HashMap<String, f64>,
) -> Result<Vec<PlanYield>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, monthly_fee_usd
         FROM plan
         WHERE kind = 'subscription'
         ORDER BY rowid ASC",
    )?;
    let plans: Vec<(String, String, f64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut google_fee_monthly = 0.0_f64;
    let mut google_equiv = 0.0_f64;
    let mut has_google = false;
    let mut yields = Vec::new();

    for (id, name, monthly_fee) in plans {
        if id == "google-ai-pro" || id == "google-ai-pro-claude" {
            has_google = true;
            google_fee_monthly += monthly_fee;
            google_equiv += *equiv_by_plan.get(&id).unwrap_or(&0.0);
        } else {
            let fee_usd = monthly_fee * (range_days / 30.0);
            let equivalent_usd = *equiv_by_plan.get(&id).unwrap_or(&0.0);
            let ratio = if fee_usd > 0.0 && equivalent_usd > 0.0 {
                Some(equivalent_usd / fee_usd)
            } else {
                None
            };
            yields.push(PlanYield {
                plan_id: id,
                name,
                fee_usd,
                equivalent_usd,
                ratio,
            });
        }
    }

    if has_google {
        let fee_usd = google_fee_monthly * (range_days / 30.0);
        let ratio = if fee_usd > 0.0 && google_equiv > 0.0 {
            Some(google_equiv / fee_usd)
        } else {
            None
        };
        yields.push(PlanYield {
            plan_id: "google-ai-pro".to_string(),
            name: "Google AI Pro".to_string(),
            fee_usd,
            equivalent_usd: google_equiv,
            ratio,
        });
    }

    Ok(yields)
}

fn query_sessions_by_project(conn: &Connection, f: &Filter) -> Result<HashMap<String, (u64, u64)>> {
    let (where_sql, params) = build_event_where(f);
    let sql = format!(
        "SELECT COALESCE(project, ''),
                COUNT(DISTINCT session_id),
                COUNT(DISTINCT CASE WHEN cost_usd IS NOT NULL AND cost_kind != 'unknown' THEN session_id END)
         FROM usage_event
         WHERE {where_sql}
         GROUP BY COALESCE(project, '')"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params_from_iter(params))?;
    let mut out = HashMap::new();
    while let Some(r) = rows.next()? {
        let project: String = r.get(0)?;
        let count: i64 = r.get(1)?;
        let priced_count: i64 = r.get(2)?;
        out.insert(project, (count.max(0) as u64, priced_count.max(0) as u64));
    }
    Ok(out)
}

fn compute_month_metrics<Tz: TimeZone>(
    conn: &Connection,
    f: &Filter,
    tz: &Tz,
    now: DateTime<Utc>,
) -> Result<(MonthProjection, Option<DeepSeekBudget>)> {
    let local_now = now.with_timezone(tz);
    let month_start_naive = NaiveDate::from_ymd_opt(local_now.year(), local_now.month(), 1)
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .ok_or_else(|| CoreError::Parse {
            file: "time".to_string(),
            line: 0,
            reason: "fecha de inicio de mes inválida".to_string(),
        })?;
    let (next_year, next_month) = if local_now.month() == 12 {
        (local_now.year() + 1, 1)
    } else {
        (local_now.year(), local_now.month() + 1)
    };
    let next_month_naive = NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .ok_or_else(|| CoreError::Parse {
            file: "time".to_string(),
            line: 0,
            reason: "fecha de fin de mes inválida".to_string(),
        })?;

    let month_start_utc = tz
        .from_local_datetime(&month_start_naive)
        .earliest()
        .ok_or_else(|| CoreError::Parse {
            file: "time".to_string(),
            line: 0,
            reason: "conversión local de inicio de mes inválida".to_string(),
        })?
        .with_timezone(&Utc);
    let next_month_utc = tz
        .from_local_datetime(&next_month_naive)
        .earliest()
        .ok_or_else(|| CoreError::Parse {
            file: "time".to_string(),
            line: 0,
            reason: "conversión local de fin de mes inválida".to_string(),
        })?
        .with_timezone(&Utc);

    let month_filter = normalize_filter(&Filter {
        from: month_start_utc,
        to: now,
        tool: f.tool,
        provider: f.provider,
        project: f.project.clone(),
    });
    let (where_sql, params) = build_hourly_where(&month_filter);
    let sql = format!(
        "SELECT COALESCE(SUM(cost_real), 0.0), COALESCE(SUM(cost_equiv), 0.0)
         FROM hourly_agg
         WHERE {where_sql}"
    );
    let (spent_real, spent_equiv): (f64, f64) = if month_start_utc < now {
        conn.query_row(&sql, params_from_iter(params), |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?
    } else {
        (0.0, 0.0)
    };

    let days_in_month = (next_month_utc - month_start_utc).num_seconds() as f64 / 86_400.0;
    let elapsed_days = (now - month_start_utc).num_seconds() as f64 / 86_400.0;
    let projection = if elapsed_days <= 0.0 {
        MonthProjection {
            real_usd: spent_real,
            api_equivalent_usd: spent_equiv,
        }
    } else {
        let factor = days_in_month / elapsed_days.max(1.0);
        MonthProjection {
            real_usd: spent_real * factor,
            api_equivalent_usd: spent_equiv * factor,
        }
    };

    let budget_opt: Option<f64> = conn
        .query_row(
            "SELECT budget_usd FROM plan WHERE id = 'deepseek'",
            [],
            |r| r.get(0),
        )
        .optional()?
        .flatten();

    let deepseek_budget = match budget_opt {
        Some(budget_usd) => {
            let ds_spent: f64 = if month_start_utc < now {
                conn.query_row(
                    "SELECT COALESCE(SUM(cost_real), 0.0)
                     FROM hourly_agg
                     WHERE hour_utc >= ?1 AND hour_utc < ?2 AND plan_id = 'deepseek'",
                    [&fmt_utc(month_filter.from), &fmt_utc(month_filter.to)],
                    |r| r.get(0),
                )?
            } else {
                0.0
            };
            let balance_usd: Option<f64> = conn
                .query_row(
                    "SELECT value FROM setting WHERE key = 'deepseek.balance_usd'",
                    [],
                    |r| r.get::<_, String>(0),
                )
                .optional()?
                .and_then(|s| s.parse::<f64>().ok());

            Some(DeepSeekBudget {
                budget_usd,
                spent_usd: ds_spent,
                balance_usd,
            })
        }
        None => None,
    };

    Ok((projection, deepseek_budget))
}

fn accumulate_session_cost(
    cost_usd: Option<f64>,
    cost_kind: CostKind,
    real_sum: &mut f64,
    real_count: &mut u64,
    equiv_sum: &mut f64,
    equiv_count: &mut u64,
) {
    if let Some(usd) = cost_usd {
        match cost_kind {
            CostKind::Real => {
                *real_sum += usd;
                *real_count += 1;
            }
            CostKind::ApiEquivalent => {
                *equiv_sum += usd;
                *equiv_count += 1;
            }
            CostKind::Unknown => {}
        }
    }
}

fn compute_anomalies_for_tool(
    conn: &Connection,
    tool: Tool,
    out: &mut HashMap<(Tool, String), bool>,
) -> Result<()> {
    for (session_id, (anomalous, _)) in compute_tool_anomalies(conn, tool)? {
        out.insert((tool, session_id), anomalous);
    }
    Ok(())
}

pub(crate) fn compute_tool_anomalies(
    conn: &Connection,
    tool: Tool,
) -> Result<HashMap<String, (bool, f64)>> {
    let mut stmt = conn.prepare(
        "SELECT session_id, MAX(ts_utc) AS end_ts, SUM(input + cache_write + cache_read + output)
         FROM usage_event
         WHERE tool = ?1
         GROUP BY session_id
         ORDER BY end_ts ASC, session_id ASC",
    )?;
    let list: Vec<(String, u64)> = stmt
        .query_map([tool.as_str()], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(2)?.max(0) as u64))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut out = HashMap::with_capacity(list.len());
    for (idx, (session_id, total_tokens)) in list.iter().enumerate() {
        let hist_start = idx.saturating_sub(30);
        let history = &list[hist_start..idx];
        let (anomalous, mean) = if history.len() >= 10 {
            let n = history.len() as f64;
            let mean = history.iter().map(|(_, t)| *t as f64).sum::<f64>() / n;
            let variance = history
                .iter()
                .map(|(_, t)| {
                    let diff = *t as f64 - mean;
                    diff * diff
                })
                .sum::<f64>()
                / n;
            let stddev = variance.sqrt();
            ((*total_tokens as f64) > (mean + 3.0 * stddev), mean)
        } else {
            (false, 0.0)
        };
        out.insert(session_id.clone(), (anomalous, mean));
    }

    Ok(out)
}

fn parse_utc(s: &str, context: &str) -> Result<DateTime<Utc>> {
    s.parse::<DateTime<Utc>>().map_err(|err| CoreError::Parse {
        file: context.to_string(),
        line: 0,
        reason: format!("fecha UTC inválida '{s}': {err}"),
    })
}
