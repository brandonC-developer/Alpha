mod common;

use std::time::Instant;

use chrono::{DateTime, Duration, FixedOffset, Utc};
use common::{generate_synthetic_events, insert_event, insert_event_with_cost, make_event};
use gestor_core::cost::CostKind;
use gestor_core::db::open_memory;
use gestor_core::model::{Provider, Tool};
use gestor_core::query::{Filter, costs, dashboard, session_detail, sessions};
use gestor_core::time::fmt_utc;

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

    let t0 = Instant::now();
    let d = dashboard(&conn, &f, &tz).unwrap();
    let elapsed = t0.elapsed();

    assert!((d.kpis.turns.value - 1_000_000.0).abs() < 1e-9);
    assert!(
        elapsed.as_millis() < 300,
        "dashboard tardó {} ms (objetivo < 300 ms)",
        elapsed.as_millis()
    );
}
