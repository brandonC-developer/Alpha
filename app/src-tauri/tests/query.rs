mod common;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::time::Instant;

use chrono::{DateTime, Duration, FixedOffset, NaiveDate, TimeZone, Utc};
use common::{generate_synthetic_events, insert_event, insert_event_with_cost, make_event};
use gestor_core::collectors::Collector;
use gestor_core::collectors::antigravity::AntigravityCollector;
use gestor_core::collectors::claude_code::ClaudeCodeCollector;
use gestor_core::cost::CostKind;
use gestor_core::db::open_memory;
use gestor_core::ingest::scan_all;
use gestor_core::model::{Provider, Tool};
use gestor_core::query::{Filter, costs, dashboard, dashboard_profiled, session_detail, sessions};
use gestor_core::time::{fmt_utc, local_date};
use rusqlite::{Connection, params_from_iter};

fn utc(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn utc_minus_6() -> FixedOffset {
    FixedOffset::west_opt(6 * 3600).unwrap()
}

#[test]
fn kpis_basicos() {
    let conn = open_memory().unwrap();
    let tz = Utc;

    let e1 = make_event(
        "cc:1:1",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        utc("2026-11-26T10:00:00Z"),
        "s-1",
        Some("Alpha"),
        100,
        200,
        700,
        50,
    );
    let e2 = make_event(
        "cc:1:2",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        utc("2026-11-26T10:02:00Z"),
        "s-1",
        Some("Alpha"),
        50,
        0,
        950,
        100,
    );
    let e3 = make_event(
        "ag:1:1",
        Tool::Antigravity,
        Provider::Google,
        "gemini-4-neon-xhigh",
        utc("2026-11-26T11:00:00Z"),
        "s-2",
        None,
        300,
        0,
        700,
        150,
    );

    insert_event(&conn, &e1);
    insert_event(&conn, &e2);
    insert_event(&conn, &e3);

    let f = Filter {
        from: utc("2026-11-26T00:00:00Z"),
        to: utc("2026-11-27T00:00:00Z"),
        tool: None,
        provider: None,
        project: None,
    };

    let d = dashboard(&conn, &f, &tz).unwrap();

    // inputTotal = (100+200+700) + (50+0+950) + (300+0+700) = 3000
    assert!((d.kpis.input_total.value - 3000.0).abs() < 1e-9);
    // cacheRead = 700 + 950 + 700 = 2350 -> 2350 / 3000 * 100 = 78.3333...
    assert!((d.kpis.cache_pct - (2350.0 / 3000.0 * 100.0)).abs() < 1e-6);
    // output = 50 + 100 + 150 = 300
    assert!((d.kpis.output.value - 300.0).abs() < 1e-9);
    // turns = 3 -> outputPerTurn = 100.0
    assert!((d.kpis.turns.value - 3.0).abs() < 1e-9);
    assert!((d.kpis.output_per_turn - 100.0).abs() < 1e-9);
    assert_eq!(d.kpis.sessions, 2);
    // s-1 tiene un hueco de 120 s; s-2 tiene 1 evento (0 s)
    assert!((d.kpis.active_secs.value - 120.0).abs() < 1e-9);
    assert_eq!(d.kpis.max_concurrency, 1);
    assert_eq!(d.days_with_use, 1);
}

#[test]
fn variacion_periodo_anterior() {
    let conn = open_memory().unwrap();
    let tz = Utc;

    // Periodo anterior [Nov 25, Nov 26): 100 inputTotal, 20 output, 1 turno
    let prev = make_event(
        "cc:prev:1",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        utc("2026-11-25T12:00:00Z"),
        "s-prev",
        Some("Alpha"),
        40,
        10,
        50,
        20,
    );
    // Periodo actual [Nov 26, Nov 27): 250 inputTotal, 50 output, 2 turnos
    let cur1 = make_event(
        "cc:cur:1",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        utc("2026-11-26T12:00:00Z"),
        "s-cur",
        Some("Alpha"),
        100,
        0,
        50,
        25,
    );
    let cur2 = make_event(
        "cc:cur:2",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        utc("2026-11-26T12:01:00Z"),
        "s-cur",
        Some("Alpha"),
        50,
        0,
        50,
        25,
    );

    insert_event(&conn, &prev);
    insert_event(&conn, &cur1);
    insert_event(&conn, &cur2);

    let f_with_prev = Filter {
        from: utc("2026-11-26T00:00:00Z"),
        to: utc("2026-11-27T00:00:00Z"),
        tool: None,
        provider: None,
        project: None,
    };
    let d1 = dashboard(&conn, &f_with_prev, &tz).unwrap();
    // (250 - 100) / 100 * 100 = 150.0 %
    assert!((d1.kpis.input_total.delta_pct.unwrap() - 150.0).abs() < 1e-9);
    assert!((d1.kpis.output.delta_pct.unwrap() - 150.0).abs() < 1e-9);
    assert!((d1.kpis.turns.delta_pct.unwrap() - 100.0).abs() < 1e-9);

    // Rango donde el periodo anterior [Nov 24, Nov 25) no tiene datos -> None
    let f_no_prev = Filter {
        from: utc("2026-11-25T00:00:00Z"),
        to: utc("2026-11-26T00:00:00Z"),
        tool: None,
        provider: None,
        project: None,
    };
    let d2 = dashboard(&conn, &f_no_prev, &tz).unwrap();
    assert_eq!(d2.kpis.input_total.delta_pct, None);
    assert_eq!(d2.kpis.output.delta_pct, None);
    assert_eq!(d2.kpis.turns.delta_pct, None);
}

#[test]
fn dia_local_no_utc() {
    let conn = open_memory().unwrap();
    let tz = utc_minus_6();

    // 2026-11-27T03:30:00Z en UTC-6 = jueves 2026-11-26 21:30
    let ev = make_event(
        "cc:tz:1",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        utc("2026-11-27T03:30:00Z"),
        "s-tz",
        Some("Alpha"),
        100,
        0,
        400,
        50,
    );
    insert_event(&conn, &ev);

    let f = Filter {
        from: utc("2026-11-26T06:00:00Z"), // 2026-11-26 00:00 local
        to: utc("2026-11-28T06:00:00Z"),   // 2026-11-28 00:00 local
        tool: None,
        provider: None,
        project: None,
    };
    let d = dashboard(&conn, &f, &tz).unwrap();

    let day_26 = d
        .by_day
        .iter()
        .find(|day| day.date == "2026-11-26")
        .unwrap();
    let day_27 = d
        .by_day
        .iter()
        .find(|day| day.date == "2026-11-27")
        .unwrap();
    assert_eq!(day_26.input_total, 500);
    assert_eq!(day_26.output, 50);
    assert_eq!(day_27.input_total, 0);
    assert_eq!(day_27.output, 0);

    // Fila 3 = jueves (0=lunes, 1=martes, 2=miércoles, 3=jueves), hora 21
    assert_eq!(d.heatmap[3][21], 550);
    let peak = d.peak.unwrap();
    assert_eq!(peak.weekday, 3);
    assert_eq!(peak.hour, 21);
}

#[test]
fn tiempo_activo_con_tope() {
    let conn = open_memory().unwrap();
    let tz = Utc;

    // Huecos: 60 s y 3600 s (tope 300 s) -> 60 + 300 = 360 s
    let t0 = utc("2026-11-26T10:00:00Z");
    let t1 = t0 + Duration::seconds(60);
    let t2 = t1 + Duration::seconds(3600);

    for (idx, ts) in [t0, t1, t2].into_iter().enumerate() {
        let ev = make_event(
            &format!("cc:act:{idx}"),
            Tool::ClaudeCode,
            Provider::Anthropic,
            "claude-opus-5-5",
            ts,
            "s-act",
            Some("Alpha"),
            10,
            0,
            0,
            5,
        );
        insert_event(&conn, &ev);
    }

    let f = Filter {
        from: utc("2026-11-26T00:00:00Z"),
        to: utc("2026-11-27T00:00:00Z"),
        tool: None,
        provider: None,
        project: None,
    };

    let d = dashboard(&conn, &f, &tz).unwrap();
    assert!((d.kpis.active_secs.value - 360.0).abs() < 1e-9);

    let sess = sessions(&conn, &f).unwrap();
    assert_eq!(sess.len(), 1);
    assert_eq!(sess[0].active_secs, 360);
}

#[test]
fn concurrencia_maxima() {
    let conn = open_memory().unwrap();
    let tz = Utc;

    // 3 sesiones distintas con eventos dentro del mismo minuto 15:02
    for (idx, sid) in ["s-a", "s-b", "s-c"].into_iter().enumerate() {
        let ev = make_event(
            &format!("cc:conc:{idx}"),
            Tool::ClaudeCode,
            Provider::Anthropic,
            "claude-opus-5-5",
            utc("2026-11-26T15:02:00Z") + Duration::seconds((idx as i64) * 15),
            sid,
            Some("Alpha"),
            10,
            0,
            0,
            5,
        );
        insert_event(&conn, &ev);
    }
    // Otro minuto con solo 1 sesión repetida 2 veces
    for idx in 0..2 {
        let ev = make_event(
            &format!("cc:conc:single:{idx}"),
            Tool::ClaudeCode,
            Provider::Anthropic,
            "claude-opus-5-5",
            utc("2026-11-26T15:05:00Z") + Duration::seconds(idx * 10),
            "s-a",
            Some("Alpha"),
            10,
            0,
            0,
            5,
        );
        insert_event(&conn, &ev);
    }

    let f = Filter {
        from: utc("2026-11-26T00:00:00Z"),
        to: utc("2026-11-27T00:00:00Z"),
        tool: None,
        provider: None,
        project: None,
    };

    let d = dashboard(&conn, &f, &tz).unwrap();
    assert_eq!(d.kpis.max_concurrency, 3);
}

#[test]
fn filtro_por_herramienta() {
    let conn = open_memory().unwrap();
    let tz = Utc;

    insert_event(
        &conn,
        &make_event(
            "cc:f:1",
            Tool::ClaudeCode,
            Provider::Anthropic,
            "claude-opus-5-5",
            utc("2026-11-26T10:00:00Z"),
            "s-cc",
            Some("Alpha"),
            1000,
            0,
            0,
            100,
        ),
    );
    insert_event(
        &conn,
        &make_event(
            "ag:f:1",
            Tool::Antigravity,
            Provider::Google,
            "gemini-4-neon-xhigh",
            utc("2026-11-26T11:00:00Z"),
            "s-ag",
            None,
            200,
            0,
            300,
            40,
        ),
    );

    let f = Filter {
        from: utc("2026-11-26T00:00:00Z"),
        to: utc("2026-11-27T00:00:00Z"),
        tool: Some(Tool::Antigravity),
        provider: None,
        project: None,
    };

    let d = dashboard(&conn, &f, &tz).unwrap();
    assert!((d.kpis.input_total.value - 500.0).abs() < 1e-9);
    assert!((d.kpis.output.value - 40.0).abs() < 1e-9);
    assert!((d.kpis.turns.value - 1.0).abs() < 1e-9);
    assert_eq!(d.kpis.sessions, 1);
    assert_eq!(d.by_model.len(), 1);
    assert_eq!(d.by_model[0].model, "gemini-4-neon-xhigh");
    assert_eq!(d.by_provider.len(), 1);
    assert_eq!(d.by_provider[0].provider, Provider::Google);
    assert_eq!(d.by_tool.len(), 1);
    assert_eq!(d.by_tool[0].tool, Tool::Antigravity);

    let c = costs(&conn, &f, &tz, utc("2026-11-26T12:00:00Z")).unwrap();
    assert_eq!(c.by_tool.len(), 1);
    assert_eq!(c.by_tool[0].tool, Tool::Antigravity);
    assert_eq!(c.by_model.len(), 1);
    assert_eq!(c.by_model[0].model, "gemini-4-neon-xhigh");

    let sess = sessions(&conn, &f).unwrap();
    assert_eq!(sess.len(), 1);
    assert_eq!(sess[0].tool, Tool::Antigravity);
}

#[test]
fn rendimiento_suscripcion() {
    let conn = open_memory().unwrap();
    let tz = Utc;
    let now = utc("2026-11-27T00:00:00Z");
    let from = now - Duration::days(30);

    // 30 días, cuota Claude Pro = 20 $, equivalente API = 312 $ -> ratio = 15.6
    let ev = make_event(
        "cc:yield:1",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        utc("2026-11-20T12:00:00Z"),
        "s-yield",
        Some("Alpha"),
        10_000_000,
        0,
        0,
        1_000_000,
    );
    insert_event_with_cost(&conn, &ev, Some(312.0), CostKind::ApiEquivalent);

    let f = Filter {
        from,
        to: now,
        tool: None,
        provider: None,
        project: None,
    };

    let c = costs(&conn, &f, &tz, now).unwrap();
    assert!((c.api_equivalent_usd - 312.0).abs() < 1e-9);
    assert!((c.fees_usd - 40.0).abs() < 1e-9);

    let claude = c
        .r#yield
        .iter()
        .find(|y| y.plan_id == "claude-pro")
        .unwrap();
    assert!((claude.fee_usd - 20.0).abs() < 1e-9);
    assert!((claude.equivalent_usd - 312.0).abs() < 1e-9);
    assert!((claude.ratio.unwrap() - 15.6).abs() < 1e-9);
}

#[test]
fn coste_unknown_no_suma_cero() {
    let conn = open_memory().unwrap();
    let tz = Utc;
    let now = utc("2026-11-27T00:00:00Z");

    // 2 eventos sin precio (gemini-flash-worker-agy) y 1 con precio (claude-opus-5-5, 10 $)
    for idx in 0..2 {
        let ev = make_event(
            &format!("ag:unk:{idx}"),
            Tool::Antigravity,
            Provider::Google,
            "gemini-flash-worker-agy",
            utc("2026-11-26T10:00:00Z") + Duration::minutes(idx),
            "s-unk",
            None,
            1_000,
            0,
            0,
            200,
        );
        insert_event(&conn, &ev);
    }
    let priced = make_event(
        "cc:priced:1",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        utc("2026-11-26T12:00:00Z"),
        "s-priced",
        Some("Alpha"),
        1_000,
        0,
        0,
        100,
    );
    insert_event_with_cost(&conn, &priced, Some(10.0), CostKind::ApiEquivalent);

    let f = Filter {
        from: utc("2026-11-26T00:00:00Z"),
        to: now,
        tool: None,
        provider: None,
        project: None,
    };

    let c = costs(&conn, &f, &tz, now).unwrap();
    assert!((c.real_usd - 0.0).abs() < 1e-9);
    assert!((c.api_equivalent_usd - 10.0).abs() < 1e-9);
    assert_eq!(c.unpriced.len(), 1);
    assert_eq!(c.unpriced[0].model, "gemini-flash-worker-agy");
    assert_eq!(c.unpriced[0].events, 2);

    // En byDay no deben aparecer entradas unknown sumando 0
    assert!(c.by_day.iter().all(|d| d.kind != CostKind::Unknown));

    let d = dashboard(&conn, &f, &tz).unwrap();
    let unk_model = d
        .by_model
        .iter()
        .find(|m| m.model == "gemini-flash-worker-agy")
        .unwrap();
    assert_eq!(unk_model.cost_usd, None);

    // Comprobar también session_detail
    let detail = session_detail(&conn, "s-unk").unwrap();
    assert_eq!(detail.len(), 2);
    assert_eq!(detail[0].cost_usd, None);
    assert_eq!(detail[0].kind, CostKind::Unknown);
}

#[test]
fn sesion_anomala() {
    // Caso 1: con solo 9 sesiones en el historial (< 10) -> anomalous = false
    let conn_short = open_memory().unwrap();
    let base = utc("2026-11-01T10:00:00Z");
    for i in 0..9 {
        let ev = make_event(
            &format!("ag:h9:{i}"),
            Tool::Antigravity,
            Provider::Google,
            "gemini-4-neon-xhigh",
            base + Duration::hours(i),
            &format!("s-h9-{i}"),
            None,
            900_000,
            0,
            0,
            100_000,
        );
        insert_event(&conn_short, &ev);
    }
    let spike_short = make_event(
        "ag:h9:spike",
        Tool::Antigravity,
        Provider::Google,
        "gemini-4-neon-xhigh",
        base + Duration::hours(20),
        "s-h9-spike",
        None,
        39_000_000,
        0,
        0,
        1_000_000,
    );
    insert_event(&conn_short, &spike_short);

    let f_short = Filter {
        from: base - Duration::days(1),
        to: base + Duration::days(5),
        tool: None,
        provider: None,
        project: None,
    };
    let rows_short = sessions(&conn_short, &f_short).unwrap();
    let spike_row_short = rows_short
        .iter()
        .find(|s| s.session_id == "s-h9-spike")
        .unwrap();
    assert!(!spike_row_short.anomalous);

    // Caso 2: con 30 sesiones ~1 M y una de 40 M -> anomalous = true
    let conn_full = open_memory().unwrap();
    for i in 0..30 {
        let delta = ((i % 5) as u64) * 10_000;
        let ev = make_event(
            &format!("ag:h30:{i}"),
            Tool::Antigravity,
            Provider::Google,
            "gemini-4-neon-xhigh",
            base + Duration::hours(i),
            &format!("s-h30-{i}"),
            None,
            900_000 + delta,
            0,
            0,
            100_000,
        );
        insert_event(&conn_full, &ev);
    }
    let spike_full = make_event(
        "ag:h30:spike",
        Tool::Antigravity,
        Provider::Google,
        "gemini-4-neon-xhigh",
        base + Duration::hours(40),
        "s-h30-spike",
        None,
        39_000_000,
        0,
        0,
        1_000_000,
    );
    insert_event(&conn_full, &spike_full);

    let rows_full = sessions(&conn_full, &f_short).unwrap();
    let spike_row_full = rows_full
        .iter()
        .find(|s| s.session_id == "s-h30-spike")
        .unwrap();
    assert!(spike_row_full.anomalous);

    let normal_row = rows_full
        .iter()
        .find(|s| s.session_id == "s-h30-29")
        .unwrap();
    assert!(!normal_row.anomalous);

    // Formato de fechas de SessionRow y CallRow usa ISO 8601 con Z
    let serialized = serde_json::to_value(spike_row_full).unwrap();
    assert!(serialized["start"].as_str().unwrap().ends_with('Z'));
    assert_eq!(
        serialized["start"].as_str().unwrap(),
        fmt_utc(spike_row_full.start)
    );
}

#[test]
#[ignore]
fn panel_rapido() {
    let mut conn = open_memory().unwrap();
    let tz = utc_minus_6();
    let to = utc("2026-11-27T06:00:00Z");
    let from = to - Duration::days(90);

    generate_synthetic_events(&mut conn, 1_000_000, from, 90);

    let f = Filter {
        from,
        to,
        tool: None,
        provider: None,
        project: None,
    };

    let hourly_rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM hourly_agg", [], |r| r.get(0))
        .unwrap();
    let session_hour_rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM session_hour", [], |r| r.get(0))
        .unwrap();

    let t0 = Instant::now();
    let d = dashboard(&conn, &f, &tz).unwrap();
    let elapsed = t0.elapsed();

    let (_, prof) = dashboard_profiled(&conn, &f, &tz).unwrap();
    println!("panel_rapido rows: hourly_agg={hourly_rows}, session_hour={session_hour_rows}");
    println!(
        "panel_rapido profile: total={:.2} ms | hourly_agg={:.2} ms | prev_totals={:.2} ms | session_sql={:.2} ms | session_calc={:.2} ms | prev_active={:.2} ms | aggregate={:.2} ms",
        prof.total_ms,
        prof.hourly_agg_ms,
        prof.prev_totals_ms,
        prof.session_sql_ms,
        prof.session_calc_ms,
        prof.prev_active_ms,
        prof.aggregate_ms,
    );
    println!("panel_rapido dashboard: {} ms", elapsed.as_millis());

    assert!((d.kpis.turns.value - 1_000_000.0).abs() < 1e-9);
    assert!(
        elapsed.as_millis() < 300,
        "dashboard tardó {} ms (objetivo < 300 ms)",
        elapsed.as_millis()
    );
}

#[derive(Debug, PartialEq)]
struct RefMetrics {
    sessions: u64,
    active_secs: u64,
    prev_active_secs: u64,
    max_concurrency: u64,
    active_by_day: HashMap<NaiveDate, u64>,
    active_by_tool: HashMap<Tool, u64>,
}

fn reference_metrics_from_events<Tz: TimeZone>(
    conn: &Connection,
    f: &Filter,
    tz: &Tz,
) -> RefMetrics {
    fn scan_range<Tz: TimeZone>(
        conn: &Connection,
        f: &Filter,
        tz: &Tz,
    ) -> (u64, u64, u64, HashMap<NaiveDate, u64>, HashMap<Tool, u64>) {
        if f.from >= f.to {
            return (0, 0, 0, HashMap::new(), HashMap::new());
        }
        let mut clauses = vec!["ts_utc >= ?1".to_string(), "ts_utc < ?2".to_string()];
        let mut params = vec![
            rusqlite::types::Value::Text(fmt_utc(f.from)),
            rusqlite::types::Value::Text(fmt_utc(f.to)),
        ];
        if let Some(tool) = f.tool {
            let idx = params.len() + 1;
            clauses.push(format!("tool = ?{idx}"));
            params.push(rusqlite::types::Value::Text(tool.as_str().to_string()));
        }
        if let Some(provider) = f.provider {
            let idx = params.len() + 1;
            clauses.push(format!("provider = ?{idx}"));
            params.push(rusqlite::types::Value::Text(provider.as_str().to_string()));
        }
        if let Some(ref project) = f.project {
            let idx = params.len() + 1;
            if project.is_empty() {
                clauses.push(format!("COALESCE(project, '') = ?{idx}"));
            } else {
                clauses.push(format!("project = ?{idx}"));
            }
            params.push(rusqlite::types::Value::Text(project.clone()));
        }

        let sql = format!(
            "SELECT ts_utc, session_id, tool
             FROM usage_event
             WHERE {}
             ORDER BY ts_utc ASC, dedup_key ASC",
            clauses.join(" AND ")
        );
        let mut stmt = conn.prepare(&sql).unwrap();
        let mut rows = stmt.query(params_from_iter(params)).unwrap();

        let mut last_by_session: HashMap<String, i64> = HashMap::new();
        let mut minute_sessions: HashMap<i64, HashSet<String>> = HashMap::new();
        let mut active_by_day: HashMap<NaiveDate, u64> = HashMap::new();
        let mut active_by_tool: HashMap<Tool, u64> = HashMap::new();
        let mut total_active = 0_u64;

        while let Some(r) = rows.next().unwrap() {
            let ts_str: String = r.get(0).unwrap();
            let session_id: String = r.get(1).unwrap();
            let tool: Tool = r.get::<_, String>(2).unwrap().parse().unwrap();
            let ts: DateTime<Utc> = ts_str.parse().unwrap();
            let ep = ts.timestamp();

            minute_sessions
                .entry(ep.div_euclid(60))
                .or_default()
                .insert(session_id.clone());

            if let Some(prev_ep) = last_by_session.get_mut(&session_id) {
                let gap = (ep - *prev_ep).clamp(0, 300) as u64;
                *prev_ep = ep;
                total_active += gap;
                *active_by_tool.entry(tool).or_insert(0) += gap;
                *active_by_day.entry(local_date(ts, tz)).or_insert(0) += gap;
            } else {
                last_by_session.insert(session_id, ep);
            }
        }

        let max_conc = minute_sessions
            .values()
            .map(|s| s.len() as u64)
            .max()
            .unwrap_or(0);

        (
            last_by_session.len() as u64,
            total_active,
            max_conc,
            active_by_day,
            active_by_tool,
        )
    }

    let (sessions, active_secs, max_concurrency, active_by_day, active_by_tool) =
        scan_range(conn, f, tz);
    let dur = f.to - f.from;
    let prev = Filter {
        from: f.from - dur,
        to: f.from,
        tool: f.tool,
        provider: f.provider,
        project: f.project.clone(),
    };
    let (_, prev_active_secs, _, _, _) = scan_range(conn, &prev, tz);

    RefMetrics {
        sessions,
        active_secs,
        prev_active_secs,
        max_concurrency,
        active_by_day,
        active_by_tool,
    }
}

#[test]
fn session_hour_equivale_a_eventos() {
    let tmp = tempfile::tempdir().unwrap();
    let cc_root = tmp.path().join("claude");
    let ag_root = tmp.path().join("antigravity");
    fs::create_dir_all(&cc_root).unwrap();

    // Generamos ~2.040 eventos en archivos reales de Claude Code y Antigravity:
    // - Varias sesiones (24)
    // - Huecos < 5 min (15 s, 90 s, 299 s, 300 s) y > 5 min (301 s, 450 s, 1200 s)
    // - Eventos fuera de orden dentro de un archivo y repartidos entre 2 archivos de la misma sesión
    // - Sesiones que cruzan cambios de hora UTC y cambio de día local
    // - Varias sesiones coincidiendo en el mismo minuto (concurrencia > 1)
    let base = utc("2026-11-25T08:40:00Z");

    let mut cc_lines_a: Vec<String> = Vec::new();
    let mut cc_lines_b: Vec<String> = Vec::new();
    let mut seq = 0_usize;

    // 16 sesiones de Claude Code × 90 eventos = 1.440 eventos
    // Las 4 primeras van en el periodo anterior (Nov 25), las 12 siguientes en Nov 26 (cruzando horas y días)
    for s in 0..16_usize {
        let session_id = format!("cc-eq-{s:02}");
        let cwd = match s % 3 {
            0 => Some(r"E:\\Proyectos\\Alpha"),
            1 => Some(r"E:\\Proyectos\\Beta"),
            _ => None,
        };
        let mut ts = if s < 4 {
            base + Duration::hours((s as i64) * 2) + Duration::minutes(45)
        } else {
            // Arrancan cerca del cambio de hora (:52:00) para cruzar horas, y algunas coinciden en el mismo minuto
            let day_offset = if s < 10 { 24 } else { 30 };
            let group_slot = ((s - 4) / 3) as i64; // grupos de 3 sesiones comparten minutos exactos
            base + Duration::hours(day_offset + group_slot * 2) + Duration::minutes(52)
        };

        let mut session_lines: Vec<String> = Vec::new();
        for k in 0..90_usize {
            seq += 1;
            let gap_secs = match k % 7 {
                0 => 15,
                1 => 90,
                2 => 299,
                3 => 300,
                4 => 301,
                5 => 450,
                _ => 45,
            };
            if k > 0 {
                ts += Duration::seconds(gap_secs);
            }
            let ts_str = fmt_utc(ts);
            let cwd_field = match cwd {
                Some(p) => format!(r#","cwd":"{p}""#),
                None => String::new(),
            };
            session_lines.push(format!(
                r#"{{"type":"assistant","sessionId":"{session_id}"{cwd_field},"timestamp":"{ts_str}","requestId":"req_{seq}","message":{{"id":"msg_{seq}","model":"claude-opus-5-5","usage":{{"input_tokens":100,"cache_creation_input_tokens":20,"cache_read_input_tokens":300,"output_tokens":50}}}}}}"#
            ));
        }

        // Desordenar deliberadamente dentro de la sesión y repartir entre dos archivos para probar recálculo
        let mut shuffled = Vec::with_capacity(session_lines.len());
        for chunk in session_lines.chunks(5) {
            for line in chunk.iter().rev() {
                shuffled.push(line.clone());
            }
        }
        for (idx, line) in shuffled.into_iter().enumerate() {
            if idx % 4 == 0 {
                cc_lines_b.push(line);
            } else {
                cc_lines_a.push(line);
            }
        }
    }

    // Mezclar líneas de distintas sesiones en cc_lines_a (orden inverso para forzar aún más desorden)
    cc_lines_a.reverse();
    fs::write(cc_root.join("parte_a.jsonl"), cc_lines_a.join("\n") + "\n").unwrap();
    fs::write(cc_root.join("parte_b.jsonl"), cc_lines_b.join("\n") + "\n").unwrap();

    // 8 sesiones de Antigravity × 75 eventos = 600 eventos (total = 2.040 eventos)
    for s in 0..8_usize {
        let uuid = format!("7f0c2a10-0000-4000-8000-{s:012}");
        let log_dir = ag_root.join(&uuid).join(".system_generated").join("logs");
        fs::create_dir_all(&log_dir).unwrap();
        let mut ts = base + Duration::hours(24 + ((s / 2) as i64) * 2) + Duration::minutes(52);
        let mut ag_lines: Vec<String> = Vec::new();
        for step in 0..75_usize {
            if step > 0 {
                let gap_secs = match step % 5 {
                    0 => 600,
                    1 => 25,
                    2 => 180,
                    3 => 305,
                    _ => 60,
                };
                ts += Duration::seconds(gap_secs);
            }
            let ts_str = fmt_utc(ts);
            ag_lines.push(format!(
                r#"{{"step_index":{step},"source":"MODEL","created_at":"{ts_str}","model":"gemini-4-neon-xhigh","input_tokens":200,"cache_read_tokens":400,"output_tokens":80}}"#
            ));
        }
        // Invertir por bloques para insertar eventos fuera de orden
        let mut out_of_order = Vec::with_capacity(ag_lines.len());
        for chunk in ag_lines.chunks(4) {
            for line in chunk.iter().rev() {
                out_of_order.push(line.clone());
            }
        }
        fs::write(
            log_dir.join("transcript.jsonl"),
            out_of_order.join("\n") + "\n",
        )
        .unwrap();
    }

    let mut conn = open_memory().unwrap();
    let collectors: Vec<Box<dyn Collector>> = vec![
        Box::new(ClaudeCodeCollector::new(cc_root)),
        Box::new(AntigravityCollector::new(vec![ag_root])),
    ];
    let stats = scan_all(
        &mut conn,
        &collectors,
        &|_, _| {},
        utc("2026-11-27T12:00:00Z"),
    )
    .unwrap();
    assert_eq!(stats.new_events, 2040);

    let session_hour_rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM session_hour", [], |r| r.get(0))
        .unwrap();
    assert!(session_hour_rows > 0);

    let filters = [
        Filter {
            from: utc("2026-11-26T00:00:00Z"),
            to: utc("2026-11-27T06:00:00Z"),
            tool: None,
            provider: None,
            project: None,
        },
        Filter {
            from: utc("2026-11-26T06:00:00Z"),
            to: utc("2026-11-27T06:00:00Z"),
            tool: Some(Tool::ClaudeCode),
            provider: None,
            project: None,
        },
        Filter {
            from: utc("2026-11-26T06:00:00Z"),
            to: utc("2026-11-27T06:00:00Z"),
            tool: Some(Tool::Antigravity),
            provider: Some(Provider::Google),
            project: None,
        },
        Filter {
            from: utc("2026-11-26T06:00:00Z"),
            to: utc("2026-11-27T06:00:00Z"),
            tool: None,
            provider: None,
            project: Some("Alpha".to_string()),
        },
        Filter {
            from: utc("2026-11-26T06:00:00Z"),
            to: utc("2026-11-27T06:00:00Z"),
            tool: Some(Tool::ClaudeCode),
            provider: None,
            project: Some(String::new()),
        },
    ];

    for (idx, f) in filters.iter().enumerate() {
        let tz = utc_minus_6();
        let d = dashboard(&conn, f, &tz).unwrap();
        let r = reference_metrics_from_events(&conn, f, &tz);

        assert_eq!(d.kpis.sessions, r.sessions, "filtro {idx}: sessions");
        assert!(
            (d.kpis.active_secs.value - r.active_secs as f64).abs() < 1e-9,
            "filtro {idx}: active_secs {} != {}",
            d.kpis.active_secs.value,
            r.active_secs
        );
        assert_eq!(
            d.kpis.max_concurrency, r.max_concurrency,
            "filtro {idx}: max_concurrency"
        );

        let expected_delta = if r.prev_active_secs > 0 {
            Some(
                (r.active_secs as f64 - r.prev_active_secs as f64) / r.prev_active_secs as f64
                    * 100.0,
            )
        } else {
            None
        };
        match (d.kpis.active_secs.delta_pct, expected_delta) {
            (Some(a), Some(b)) => {
                assert!((a - b).abs() < 1e-9, "filtro {idx}: delta_pct {a} != {b}")
            }
            (None, None) => {}
            other => panic!("filtro {idx}: delta_pct mismatch {other:?}"),
        }

        // Verificar spark diario de active_secs
        assert_eq!(d.kpis.active_secs.spark.len(), d.by_day.len());
        for (day, spark_val) in d.by_day.iter().zip(d.kpis.active_secs.spark.iter()) {
            let nd: NaiveDate = day.date.parse().unwrap();
            let expected = *r.active_by_day.get(&nd).unwrap_or(&0) as f64;
            assert!(
                (*spark_val - expected).abs() < 1e-9,
                "filtro {idx} día {}: spark {spark_val} != {expected}",
                day.date
            );
        }

        // Verificar active_secs por herramienta
        for t in &d.by_tool {
            let expected = *r.active_by_tool.get(&t.tool).unwrap_or(&0);
            assert_eq!(
                t.active_secs, expected,
                "filtro {idx} tool {:?}: {} != {}",
                t.tool, t.active_secs, expected
            );
        }
    }
}

#[test]
fn session_hour_mezcla_proveedores_y_proyectos() {
    let conn = open_memory().unwrap();
    let tz = Utc;
    let base = utc("2026-11-26T10:00:00Z");

    // Caso 1 de Carbon: eventos cada minuto alternando google y anthropic (0..5 min) -> 300 s total, 240 s por proveedor
    for (idx, provider) in [
        Provider::Google,
        Provider::Anthropic,
        Provider::Google,
        Provider::Anthropic,
        Provider::Google,
        Provider::Anthropic,
    ]
    .into_iter()
    .enumerate()
    {
        let model = match provider {
            Provider::Google => "gemini-4-neon-xhigh",
            _ => "claude-opus-5-5",
        };
        let ev = make_event(
            &format!("ag:mix1:{idx}"),
            Tool::Antigravity,
            provider,
            model,
            base + Duration::minutes(idx as i64),
            "conv-1",
            None,
            100,
            0,
            0,
            50,
        );
        insert_event(&conn, &ev);
    }

    let f_all = Filter {
        from: utc("2026-11-26T10:00:00Z"),
        to: utc("2026-11-26T11:00:00Z"),
        tool: None,
        provider: None,
        project: None,
    };
    let d_all = dashboard(&conn, &f_all, &tz).unwrap();
    let s_all = sessions(&conn, &f_all).unwrap();
    assert_eq!(s_all.len(), 1);
    assert_eq!(s_all[0].active_secs, 300);
    assert!((d_all.kpis.active_secs.value - 300.0).abs() < 1e-9);

    let f_google = Filter {
        provider: Some(Provider::Google),
        ..f_all.clone()
    };
    let d_google = dashboard(&conn, &f_google, &tz).unwrap();
    let s_google = sessions(&conn, &f_google).unwrap();
    assert_eq!(s_google[0].active_secs, 240);
    assert!((d_google.kpis.active_secs.value - 240.0).abs() < 1e-9);

    let f_anthropic = Filter {
        provider: Some(Provider::Anthropic),
        ..f_all.clone()
    };
    let d_anthropic = dashboard(&conn, &f_anthropic, &tz).unwrap();
    let s_anthropic = sessions(&conn, &f_anthropic).unwrap();
    assert_eq!(s_anthropic[0].active_secs, 240);
    assert!((d_anthropic.kpis.active_secs.value - 240.0).abs() < 1e-9);

    // Caso 2 de Carbon: eventos cada 4 min alternando google y anthropic -> 720 s sin filtro, 300 s con google
    let base2 = utc("2026-11-26T12:00:00Z");
    for (idx, provider) in [
        Provider::Google,
        Provider::Anthropic,
        Provider::Google,
        Provider::Anthropic,
    ]
    .into_iter()
    .enumerate()
    {
        let model = match provider {
            Provider::Google => "gemini-4-neon-xhigh",
            _ => "claude-opus-5-5",
        };
        let ev = make_event(
            &format!("ag:mix2:{idx}"),
            Tool::Antigravity,
            provider,
            model,
            base2 + Duration::minutes((idx as i64) * 4),
            "conv-2",
            None,
            100,
            0,
            0,
            50,
        );
        insert_event(&conn, &ev);
    }

    let f2_all = Filter {
        from: utc("2026-11-26T12:00:00Z"),
        to: utc("2026-11-26T13:00:00Z"),
        tool: None,
        provider: None,
        project: None,
    };
    let d2_all = dashboard(&conn, &f2_all, &tz).unwrap();
    let s2_all = sessions(&conn, &f2_all).unwrap();
    assert_eq!(s2_all[0].active_secs, 720);
    assert!((d2_all.kpis.active_secs.value - 720.0).abs() < 1e-9);

    let f2_google = Filter {
        provider: Some(Provider::Google),
        ..f2_all.clone()
    };
    let d2_google = dashboard(&conn, &f2_google, &tz).unwrap();
    let s2_google = sessions(&conn, &f2_google).unwrap();
    assert_eq!(s2_google[0].active_secs, 300);
    assert!((d2_google.kpis.active_secs.value - 300.0).abs() < 1e-9);

    // Caso 3: mezcla de proyectos y cruce de hora (el hueco entre horas parte del max last_ts previo)
    for (idx, (ts, proj)) in [
        (utc("2026-11-26T14:00:00Z"), "Alpha"),
        (utc("2026-11-26T14:02:00Z"), "Beta"),
        (utc("2026-11-26T14:59:00Z"), "Alpha"),
        (utc("2026-11-26T15:01:00Z"), "Beta"),
    ]
    .into_iter()
    .enumerate()
    {
        let ev = make_event(
            &format!("cc:proj:{idx}"),
            Tool::ClaudeCode,
            Provider::Anthropic,
            "claude-opus-5-5",
            ts,
            "conv-3",
            Some(proj),
            100,
            0,
            0,
            50,
        );
        insert_event(&conn, &ev);
    }

    let f3_all = Filter {
        from: utc("2026-11-26T14:00:00Z"),
        to: utc("2026-11-26T16:00:00Z"),
        tool: None,
        provider: None,
        project: None,
    };
    let d3_all = dashboard(&conn, &f3_all, &tz).unwrap();
    let s3_all = sessions(&conn, &f3_all).unwrap();
    assert_eq!(s3_all[0].active_secs, 540);
    assert!((d3_all.kpis.active_secs.value - 540.0).abs() < 1e-9);

    let f3_beta = Filter {
        project: Some("Beta".to_string()),
        ..f3_all
    };
    let d3_beta = dashboard(&conn, &f3_beta, &tz).unwrap();
    let s3_beta = sessions(&conn, &f3_beta).unwrap();
    assert_eq!(s3_beta[0].active_secs, 300);
    assert!((d3_beta.kpis.active_secs.value - 300.0).abs() < 1e-9);
}

#[test]
fn filtro_redondea_bordes_a_hora_utc() {
    let conn = open_memory().unwrap();
    let tz = Utc;

    // Caso 1 de Carbon: now = 2026-11-27T19:52:10Z, from = 20T19:52:10Z, to = 27T19:52:10Z, evento en 20T19:55:00Z (5,00 $)
    let ev1 = make_event(
        "cc:edge1:1",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        utc("2026-11-20T19:55:00Z"),
        "s-edge-1",
        Some("Alpha"),
        100,
        0,
        0,
        50,
    );
    insert_event_with_cost(&conn, &ev1, Some(5.0), CostKind::ApiEquivalent);

    let now1 = utc("2026-11-27T19:52:10Z");
    let f1 = Filter {
        from: utc("2026-11-20T19:52:10Z"),
        to: now1,
        tool: None,
        provider: None,
        project: None,
    };

    let d1 = dashboard(&conn, &f1, &tz).unwrap();
    assert!((d1.kpis.turns.value - 1.0).abs() < 1e-9);
    assert_eq!(d1.kpis.sessions, 1);
    // El periodo anterior termina en 2026-11-20T19:00:00Z, así que no debe robar el evento de 19:55:00Z
    assert_eq!(d1.kpis.turns.delta_pct, None);

    let c1 = costs(&conn, &f1, &tz, now1).unwrap();
    assert!((c1.api_equivalent_usd - 5.0).abs() < 1e-9);
    assert_eq!(c1.by_project.len(), 1);
    assert_eq!(c1.by_project[0].sessions, 1);
    let p1_json = serde_json::to_value(&c1.by_project[0]).unwrap();
    assert!((p1_json["usd"].as_f64().unwrap() - 5.0).abs() < 1e-9);

    let s1 = sessions(&conn, &f1).unwrap();
    assert_eq!(s1.len(), 1);

    // Caso 2 de Carbon: from = 19:10:00Z, to = 19:52:10Z, evento en 19:40:00Z
    let ev2 = make_event(
        "cc:edge2:1",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        utc("2026-11-27T19:40:00Z"),
        "s-edge-2",
        Some("Beta"),
        100,
        0,
        0,
        50,
    );
    insert_event_with_cost(&conn, &ev2, Some(2.0), CostKind::ApiEquivalent);

    let f2 = Filter {
        from: utc("2026-11-27T19:10:00Z"),
        to: utc("2026-11-27T19:52:10Z"),
        tool: None,
        provider: None,
        project: None,
    };
    let d2 = dashboard(&conn, &f2, &tz).unwrap();
    assert!((d2.kpis.turns.value - 1.0).abs() < 1e-9);
    assert_eq!(d2.kpis.sessions, 1);

    let c2 = costs(&conn, &f2, &tz, now1).unwrap();
    assert!((c2.api_equivalent_usd - 2.0).abs() < 1e-9);

    let s2 = sessions(&conn, &f2).unwrap();
    assert_eq!(s2.len(), 1);

    // Consistencia en el borde superior `to`: to = 19:30:00Z redondea arriba a 20:00:00Z en las 3 tablas
    let f3 = Filter {
        from: utc("2026-11-27T18:00:00Z"),
        to: utc("2026-11-27T19:30:00Z"),
        tool: None,
        provider: None,
        project: None,
    };
    let d3 = dashboard(&conn, &f3, &tz).unwrap();
    let c3 = costs(&conn, &f3, &tz, now1).unwrap();
    let s3 = sessions(&conn, &f3).unwrap();
    assert!((d3.kpis.turns.value - 1.0).abs() < 1e-9);
    assert_eq!(d3.kpis.sessions, 1);
    assert!((c3.api_equivalent_usd - 2.0).abs() < 1e-9);
    assert_eq!(s3.len(), 1);
}

#[test]
fn by_project_sin_precio_es_null_y_media_solo_con_precio() {
    let conn = open_memory().unwrap();
    let tz = Utc;
    let now = utc("2026-11-27T00:00:00Z");

    // Proyecto "Alpha": 1 sesión con precio (10 $) + 9 sesiones unknown (NULL)
    let alpha_priced = make_event(
        "cc:alpha:priced",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        utc("2026-11-26T10:00:00Z"),
        "s-alpha-priced",
        Some("Alpha"),
        1_000,
        0,
        0,
        100,
    );
    insert_event_with_cost(&conn, &alpha_priced, Some(10.0), CostKind::ApiEquivalent);
    for i in 0..9 {
        let ev = make_event(
            &format!("ag:alpha:unk:{i}"),
            Tool::Antigravity,
            Provider::Google,
            "gemini-flash-worker-agy",
            utc("2026-11-26T11:00:00Z") + Duration::minutes(i),
            &format!("s-alpha-unk-{i}"),
            Some("Alpha"),
            500,
            0,
            0,
            50,
        );
        insert_event_with_cost(&conn, &ev, None, CostKind::Unknown);
    }

    // Proyecto "SinPrecio": 10 sesiones todas unknown
    for i in 0..10 {
        let ev = make_event(
            &format!("ag:sinprecio:{i}"),
            Tool::Antigravity,
            Provider::Google,
            "gemini-flash-worker-agy",
            utc("2026-11-26T12:00:00Z") + Duration::minutes(i),
            &format!("s-sinprecio-{i}"),
            Some("SinPrecio"),
            500,
            0,
            0,
            50,
        );
        insert_event_with_cost(&conn, &ev, None, CostKind::Unknown);
    }

    // Proyecto "Gratis": 2 sesiones con tarifa real 0,00 $
    for i in 0..2 {
        let ev = make_event(
            &format!("oc:gratis:{i}"),
            Tool::Opencode,
            Provider::Other,
            "deepseek-flash-free",
            utc("2026-11-26T13:00:00Z") + Duration::minutes(i),
            &format!("s-gratis-{i}"),
            Some("Gratis"),
            500,
            0,
            0,
            50,
        );
        insert_event_with_cost(&conn, &ev, Some(0.0), CostKind::Real);
    }

    let f = Filter {
        from: utc("2026-11-26T00:00:00Z"),
        to: now,
        tool: None,
        provider: None,
        project: None,
    };
    let c = costs(&conn, &f, &tz, now).unwrap();

    let alpha_row = c.by_project.iter().find(|p| p.project == "Alpha").unwrap();
    assert_eq!(alpha_row.sessions, 10);
    assert!((alpha_row.usd.unwrap() - 10.0).abs() < 1e-9);
    assert!((alpha_row.avg_per_session.unwrap() - 10.0).abs() < 1e-9);

    let sin_precio_row = c
        .by_project
        .iter()
        .find(|p| p.project == "SinPrecio")
        .unwrap();
    assert_eq!(sin_precio_row.sessions, 10);
    assert_eq!(sin_precio_row.usd, None);
    assert_eq!(sin_precio_row.avg_per_session, None);

    let gratis_row = c.by_project.iter().find(|p| p.project == "Gratis").unwrap();
    assert_eq!(gratis_row.sessions, 2);
    assert!((gratis_row.usd.unwrap() - 0.0).abs() < 1e-9);
    assert!((gratis_row.avg_per_session.unwrap() - 0.0).abs() < 1e-9);

    let by_proj_json = serde_json::to_value(&c.by_project).unwrap();
    let arr = by_proj_json.as_array().unwrap();
    assert_eq!(arr.len(), 3);

    let alpha = arr.iter().find(|p| p["project"] == "Alpha").unwrap();
    assert_eq!(alpha["sessions"], 10);
    assert!((alpha["usd"].as_f64().unwrap() - 10.0).abs() < 1e-9);
    assert!((alpha["avgPerSession"].as_f64().unwrap() - 10.0).abs() < 1e-9);

    let sin_precio = arr.iter().find(|p| p["project"] == "SinPrecio").unwrap();
    assert_eq!(sin_precio["sessions"], 10);
    assert!(sin_precio["usd"].is_null());
    assert!(sin_precio["avgPerSession"].is_null());

    let gratis = arr.iter().find(|p| p["project"] == "Gratis").unwrap();
    assert_eq!(gratis["sessions"], 2);
    assert!((gratis["usd"].as_f64().unwrap() - 0.0).abs() < 1e-9);
    assert!((gratis["avgPerSession"].as_f64().unwrap() - 0.0).abs() < 1e-9);
}
