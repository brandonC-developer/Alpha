use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Timelike, Utc};
use gestor_core::cost::{CostKind, apply_cost};
use gestor_core::ingest::refresh_session_hour;
use gestor_core::model::{Provider, Tool, UsageEvent, plan_for};
use gestor_core::time::{fmt_utc, hour_utc_str};
use rusqlite::{Connection, params};

#[allow(dead_code)]
pub fn insert_event(conn: &Connection, ev: &UsageEvent) {
    insert_raw_event(conn, ev);
    apply_cost(conn, &ev.dedup_key).unwrap();

    let (cost_usd, cost_kind): (Option<f64>, String) = conn
        .query_row(
            "SELECT cost_usd, cost_kind FROM usage_event WHERE dedup_key = ?1",
            [&ev.dedup_key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();

    upsert_hourly_agg(conn, ev, cost_usd, &cost_kind);
    refresh_session_hour(conn, &ev.session_id, &hour_utc_str(ev.ts_utc)).unwrap();
}

#[allow(dead_code)]
pub fn insert_event_with_cost(
    conn: &Connection,
    ev: &UsageEvent,
    cost_usd: Option<f64>,
    cost_kind: CostKind,
) {
    let plan_id = plan_for(ev.tool, ev.provider);
    let ts_str = fmt_utc(ev.ts_utc);
    conn.execute(
        "INSERT INTO usage_event (
            dedup_key, tool, provider, plan_id, model, ts_utc, session_id, project,
            input, cache_write, cache_read, output, reasoning,
            cost_usd, cost_kind, cost_computed_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            ev.dedup_key,
            ev.tool.as_str(),
            ev.provider.as_str(),
            plan_id,
            ev.model,
            ts_str,
            ev.session_id,
            ev.project.as_deref(),
            ev.input as i64,
            ev.cache_write as i64,
            ev.cache_read as i64,
            ev.output as i64,
            ev.reasoning as i64,
            cost_usd,
            cost_kind.as_str(),
            ts_str,
        ],
    )
    .unwrap();

    upsert_hourly_agg(conn, ev, cost_usd, cost_kind.as_str());
    refresh_session_hour(conn, &ev.session_id, &hour_utc_str(ev.ts_utc)).unwrap();
}

fn insert_raw_event(conn: &Connection, ev: &UsageEvent) {
    let plan_id = plan_for(ev.tool, ev.provider);
    let ts_str = fmt_utc(ev.ts_utc);
    conn.execute(
        "INSERT INTO usage_event (
            dedup_key, tool, provider, plan_id, model, ts_utc, session_id, project,
            input, cache_write, cache_read, output, reasoning
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            ev.dedup_key,
            ev.tool.as_str(),
            ev.provider.as_str(),
            plan_id,
            ev.model,
            ts_str,
            ev.session_id,
            ev.project.as_deref(),
            ev.input as i64,
            ev.cache_write as i64,
            ev.cache_read as i64,
            ev.output as i64,
            ev.reasoning as i64,
        ],
    )
    .unwrap();
}

fn upsert_hourly_agg(conn: &Connection, ev: &UsageEvent, cost_usd: Option<f64>, cost_kind: &str) {
    let plan_id = plan_for(ev.tool, ev.provider);
    let hour_str = hour_utc_str(ev.ts_utc);
    let project_key = ev.project.as_deref().unwrap_or("");
    let (cost_real, cost_equiv, unpriced) = match (cost_kind, cost_usd) {
        ("real", Some(c)) => (c, 0.0, 0_i64),
        ("api_equivalent", Some(c)) => (0.0, c, 0_i64),
        _ => (0.0, 0.0, 1_i64),
    };

    conn.execute(
        "INSERT INTO hourly_agg (
            hour_utc, tool, provider, plan_id, model, project,
            turns, input, cache_write, cache_read, output,
            cost_real, cost_equiv, unpriced
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
        ON CONFLICT(hour_utc, tool, provider, plan_id, model, project) DO UPDATE SET
            turns = turns + 1,
            input = input + excluded.input,
            cache_write = cache_write + excluded.cache_write,
            cache_read = cache_read + excluded.cache_read,
            output = output + excluded.output,
            cost_real = cost_real + excluded.cost_real,
            cost_equiv = cost_equiv + excluded.cost_equiv,
            unpriced = unpriced + excluded.unpriced",
        params![
            hour_str,
            ev.tool.as_str(),
            ev.provider.as_str(),
            plan_id,
            ev.model,
            project_key,
            ev.input as i64,
            ev.cache_write as i64,
            ev.cache_read as i64,
            ev.output as i64,
            cost_real,
            cost_equiv,
            unpriced,
        ],
    )
    .unwrap();
}

#[allow(dead_code, clippy::too_many_arguments)]
pub fn make_event(
    dedup_key: &str,
    tool: Tool,
    provider: Provider,
    model: &str,
    ts_utc: DateTime<Utc>,
    session_id: &str,
    project: Option<&str>,
    input: u64,
    cache_write: u64,
    cache_read: u64,
    output: u64,
) -> UsageEvent {
    UsageEvent {
        dedup_key: dedup_key.to_string(),
        tool,
        provider,
        model: model.to_string(),
        ts_utc,
        session_id: session_id.to_string(),
        project: project.map(str::to_string),
        input,
        cache_write,
        cache_read,
        output,
        reasoning: 0,
    }
}

#[allow(dead_code)]
pub fn generate_synthetic_events(
    conn: &mut Connection,
    count: usize,
    start: DateTime<Utc>,
    span_days: i64,
) {
    let total_secs = (span_days * 86_400).max(1);
    let tx = conn.transaction().unwrap();
    {
        let mut stmt = tx
            .prepare(
                "INSERT INTO usage_event (
                    dedup_key, tool, provider, plan_id, model, ts_utc, session_id, project,
                    input, cache_write, cache_read, output, reasoning,
                    cost_usd, cost_kind, cost_computed_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 0, ?13, ?14, ?15)",
            )
            .unwrap();

        let tools = [
            (
                "claude-code",
                "anthropic",
                "claude-pro",
                "claude-opus-5-5",
                "api_equivalent",
            ),
            (
                "antigravity",
                "google",
                "google-ai-pro",
                "gemini-4-neon-xhigh",
                "api_equivalent",
            ),
            ("opencode", "deepseek", "deepseek", "deepseek-flash", "real"),
        ];

        for i in 0..count {
            let (tool, provider, plan_id, model, kind) = tools[i % tools.len()];
            let offset_secs = ((i as i64) * total_secs) / (count as i64).max(1);
            let ts = start + Duration::seconds(offset_secs);
            let ts_str = fmt_utc(ts);
            let session_id = format!("syn-{}", i / 50);
            let project = if i % 4 == 0 { None } else { Some("Alpha") };
            let dedup_key = format!("syn:{i}");

            stmt.execute(params![
                dedup_key, tool, provider, plan_id, model, &ts_str, session_id, project, 100_i64,
                50_i64, 850_i64, 40_i64, 0.01_f64, kind, &ts_str,
            ])
            .unwrap();
        }
    }

    {
        struct SynAcc {
            events: i64,
            first_ts: String,
            last_ts: String,
            last_ep: i64,
            active_secs: i64,
            minute_mask: i64,
        }
        let mut sh_stmt = tx
            .prepare(
                "INSERT INTO session_hour (
                    session_id, tool, provider, project, hour_utc,
                    events, first_ts, last_ts, active_secs, minute_mask
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )
            .unwrap();
        let tools = [
            ("claude-code", "anthropic"),
            ("antigravity", "google"),
            ("opencode", "deepseek"),
        ];
        let mut current_session_idx = 0_usize;
        let mut current_session_id = "syn-0".to_string();
        let mut batch: BTreeMap<(&'static str, &'static str, &'static str, String), SynAcc> =
            BTreeMap::new();

        for i in 0..count {
            let s_idx = i / 50;
            if s_idx != current_session_idx {
                for ((tool, provider, project, hour_utc), acc) in std::mem::take(&mut batch) {
                    sh_stmt
                        .execute(params![
                            &current_session_id,
                            tool,
                            provider,
                            project,
                            &hour_utc,
                            acc.events,
                            &acc.first_ts,
                            &acc.last_ts,
                            acc.active_secs,
                            acc.minute_mask,
                        ])
                        .unwrap();
                }
                current_session_idx = s_idx;
                current_session_id = format!("syn-{s_idx}");
            }

            let (tool, provider) = tools[i % tools.len()];
            let project = if i % 4 == 0 { "" } else { "Alpha" };
            let offset_secs = ((i as i64) * total_secs) / (count as i64).max(1);
            let ts = start + Duration::seconds(offset_secs);
            let ts_str = fmt_utc(ts);
            let hour_utc = hour_utc_str(ts);
            let ep = ts.timestamp();
            let bit = 1_i64 << ts.minute();

            for (prov, proj) in [
                (provider, project),
                (provider, "*"),
                ("*", project),
                ("*", "*"),
            ] {
                let key = (tool, prov, proj, hour_utc.clone());
                if let Some(acc) = batch.get_mut(&key) {
                    acc.events += 1;
                    acc.last_ts = ts_str.clone();
                    acc.active_secs += (ep - acc.last_ep).clamp(0, 300);
                    acc.last_ep = ep;
                    acc.minute_mask |= bit;
                } else {
                    batch.insert(
                        key,
                        SynAcc {
                            events: 1,
                            first_ts: ts_str.clone(),
                            last_ts: ts_str.clone(),
                            last_ep: ep,
                            active_secs: 0,
                            minute_mask: bit,
                        },
                    );
                }
            }
        }
        for ((tool, provider, project, hour_utc), acc) in batch {
            sh_stmt
                .execute(params![
                    &current_session_id,
                    tool,
                    provider,
                    project,
                    &hour_utc,
                    acc.events,
                    &acc.first_ts,
                    &acc.last_ts,
                    acc.active_secs,
                    acc.minute_mask,
                ])
                .unwrap();
        }
    }

    tx.execute(
        "INSERT INTO hourly_agg (
            hour_utc, tool, provider, plan_id, model, project,
            turns, input, cache_write, cache_read, output,
            cost_real, cost_equiv, unpriced
         )
         SELECT substr(ts_utc, 1, 13) || ':00:00Z', tool, provider, plan_id, model, COALESCE(project, ''),
                COUNT(*), SUM(input), SUM(cache_write), SUM(cache_read), SUM(output),
                COALESCE(SUM(CASE WHEN cost_kind = 'real' THEN cost_usd ELSE 0.0 END), 0.0),
                COALESCE(SUM(CASE WHEN cost_kind = 'api_equivalent' THEN cost_usd ELSE 0.0 END), 0.0),
                COALESCE(SUM(CASE WHEN cost_kind = 'unknown' THEN 1 ELSE 0 END), 0)
         FROM usage_event
         GROUP BY 1, 2, 3, 4, 5, 6",
        [],
    )
    .unwrap();

    tx.commit().unwrap();
}
