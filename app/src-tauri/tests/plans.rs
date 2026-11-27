mod common;

use chrono::{DateTime, Duration, Utc};
use common::{insert_event_with_cost, make_event};
use gestor_core::cost::CostKind;
use gestor_core::db::open_memory;
use gestor_core::model::{Provider, Tool};
use gestor_core::plans::{
    CalibrationSource, PlanKind, WindowName, calibrate, current_block, list_plans, update_plan,
    window_status,
};

fn utc(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

#[test]
fn planes_se_leen_y_actualizan() {
    let conn = open_memory().unwrap();
    let plans = list_plans(&conn).unwrap();
    assert_eq!(plans.len(), 4);

    let claude = plans.iter().find(|p| p.id == "claude-pro").unwrap();
    assert_eq!(claude.kind, PlanKind::Subscription);
    assert!((claude.monthly_fee_usd - 20.0).abs() < 1e-9);
    assert_eq!(claude.windows.len(), 2);

    let updated = update_plan(&conn, "deepseek", 0.0, Some(12.5)).unwrap();
    assert_eq!(updated.kind, PlanKind::Payg);
    assert_eq!(updated.budget_usd, Some(12.5));
    assert!(
        window_status(&conn, "deepseek", utc("2026-11-27T12:00:00Z"))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn bloque_empieza_con_primer_evento() {
    let points = vec![
        (utc("2026-11-27T09:10:00Z"), 1.0),
        (utc("2026-11-27T10:00:00Z"), 2.0),
        (utc("2026-11-27T13:00:00Z"), 0.5),
    ];
    let block = current_block(&points, utc("2026-11-27T13:30:00Z"), 5).unwrap();
    assert_eq!(block.start, utc("2026-11-27T09:10:00Z"));
    assert_eq!(block.end, utc("2026-11-27T14:10:00Z"));
    assert!((block.used - 3.5).abs() < 1e-9);
}

#[test]
fn hueco_largo_abre_bloque_nuevo() {
    let points = vec![
        (utc("2026-11-27T09:10:00Z"), 4.0),
        (utc("2026-11-27T15:00:00Z"), 1.5),
    ];
    let block = current_block(&points, utc("2026-11-27T15:30:00Z"), 5).unwrap();
    assert_eq!(block.start, utc("2026-11-27T15:00:00Z"));
    assert_eq!(block.end, utc("2026-11-27T20:00:00Z"));
    assert!((block.used - 1.5).abs() < 1e-9);
}

#[test]
fn sin_eventos_recientes_no_hay_bloque() {
    let now = utc("2026-11-27T18:00:00Z");
    let points = vec![(now - Duration::hours(6), 2.0)];
    assert!(current_block(&points, now, 5).is_none());
}

#[test]
fn semanal_son_168_horas() {
    let conn = open_memory().unwrap();
    let now = utc("2026-11-27T18:00:00Z");

    let old_ev = make_event(
        "cc:old",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        now - Duration::hours(169),
        "s-old",
        None,
        10,
        0,
        0,
        10,
    );
    let recent_ev = make_event(
        "cc:recent",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        now - Duration::hours(10),
        "s-recent",
        None,
        10,
        0,
        0,
        10,
    );
    let unpriced_ev = make_event(
        "cc:unpriced",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-nuevo",
        now - Duration::hours(2),
        "s-recent",
        None,
        10,
        0,
        0,
        10,
    );

    insert_event_with_cost(&conn, &old_ev, Some(9.0), CostKind::ApiEquivalent);
    insert_event_with_cost(&conn, &recent_ev, Some(2.5), CostKind::ApiEquivalent);
    insert_event_with_cost(&conn, &unpriced_ev, None, CostKind::Unknown);

    let statuses = window_status(&conn, "claude-pro", now).unwrap();
    let weekly = statuses
        .iter()
        .find(|w| w.name == WindowName::Weekly)
        .unwrap();
    assert!((weekly.used - 2.5).abs() < 1e-9);
    assert_eq!(weekly.unpriced_events, 1);
    assert_eq!(weekly.resets_at, None);
}

#[test]
fn calibracion_calcula_limite() {
    let conn = open_memory().unwrap();
    let t1 = utc("2026-11-27T10:00:00Z");

    let ev1 = make_event(
        "cc:c1",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        t1 - Duration::minutes(10),
        "s-1",
        None,
        100,
        0,
        0,
        100,
    );
    insert_event_with_cost(&conn, &ev1, Some(3.0), CostKind::ApiEquivalent);

    let lim1 = calibrate(&conn, "claude-pro", WindowName::FiveHours, 40.0, t1).unwrap();
    assert!((lim1 - 7.5).abs() < 1e-9);

    // Nuevo bloque de 5 h más tarde con usado 4.25 al 50 % -> límite 8.5; media de (7.5, 8.5) = 8.0
    let t2 = t1 + Duration::hours(6);
    let ev2 = make_event(
        "cc:c2",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        t2 - Duration::minutes(5),
        "s-2",
        None,
        100,
        0,
        0,
        100,
    );
    insert_event_with_cost(&conn, &ev2, Some(4.25), CostKind::ApiEquivalent);

    let lim2 = calibrate(&conn, "claude-pro", WindowName::FiveHours, 50.0, t2).unwrap();
    assert!((lim2 - 8.0).abs() < 1e-9);

    let statuses = window_status(&conn, "claude-pro", t2).unwrap();
    let w5 = statuses
        .iter()
        .find(|w| w.name == WindowName::FiveHours)
        .unwrap();
    assert!((w5.limit.unwrap() - 8.0).abs() < 1e-9);
    assert!((w5.percent.unwrap() - (4.25 / 8.0 * 100.0)).abs() < 1e-6);
    assert_eq!(w5.calibrated_by, Some(CalibrationSource::User));
    assert_eq!(w5.calibrated_at, Some(t2));
}

#[test]
fn calibracion_rechaza_porcentaje_invalido() {
    let conn = open_memory().unwrap();
    let now = utc("2026-11-27T12:00:00Z");
    let ev = make_event(
        "cc:inv",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        now - Duration::minutes(10),
        "s-1",
        None,
        100,
        0,
        0,
        100,
    );
    insert_event_with_cost(&conn, &ev, Some(3.0), CostKind::ApiEquivalent);

    assert!(calibrate(&conn, "claude-pro", WindowName::FiveHours, 0.0, now).is_err());
    assert!(calibrate(&conn, "claude-pro", WindowName::FiveHours, -5.0, now).is_err());
    assert!(calibrate(&conn, "claude-pro", WindowName::FiveHours, 120.0, now).is_err());

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM plan_calibration", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn sin_calibrar_no_hay_porcentaje() {
    let conn = open_memory().unwrap();
    let now = utc("2026-11-27T12:00:00Z");
    let ev = make_event(
        "cc:nocal",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        now - Duration::minutes(10),
        "s-1",
        None,
        100,
        0,
        0,
        100,
    );
    insert_event_with_cost(&conn, &ev, Some(2.0), CostKind::ApiEquivalent);

    let statuses = window_status(&conn, "claude-pro", now).unwrap();
    for w in statuses {
        assert_eq!(w.limit, None);
        assert_eq!(w.percent, None);
        assert_eq!(w.calibrated_by, None);
        assert_eq!(w.calibrated_at, None);
    }
}

#[test]
fn ritmo_estima_agotamiento() {
    let conn = open_memory().unwrap();
    let block_start = utc("2026-11-27T10:00:00Z");
    let now = utc("2026-11-27T12:00:00Z");

    // 5.0 $ al principio del bloque y 1.0 $ en los últimos 30 min -> total 6.0 $
    let e1 = make_event(
        "cc:r1",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        block_start,
        "s-1",
        None,
        100,
        0,
        0,
        100,
    );
    let e2 = make_event(
        "cc:r2",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        now - Duration::minutes(15),
        "s-1",
        None,
        100,
        0,
        0,
        100,
    );
    insert_event_with_cost(&conn, &e1, Some(5.0), CostKind::ApiEquivalent);
    insert_event_with_cost(&conn, &e2, Some(1.0), CostKind::ApiEquivalent);

    // Calibramos con usado 6.0 al 75 % -> límite = 8.0; restante = 2.0; ritmo = 1.0 / 30 min = 2.0 $/h -> exhaustsAt = now + 1 h (13:00 < 15:00)
    let lim = calibrate(&conn, "claude-pro", WindowName::FiveHours, 75.0, now).unwrap();
    assert!((lim - 8.0).abs() < 1e-9);

    let statuses = window_status(&conn, "claude-pro", now).unwrap();
    let w5 = statuses
        .iter()
        .find(|w| w.name == WindowName::FiveHours)
        .unwrap();
    assert_eq!(w5.resets_at, Some(block_start + Duration::hours(5)));
    assert_eq!(w5.exhausts_at, Some(now + Duration::hours(1)));

    // Si ahora quedan solo 20 min de bloque (now = 14:40, fin = 15:00), exhaustsAt (15:40) > fin -> None
    let late_now = utc("2026-11-27T14:40:00Z");
    let e3 = make_event(
        "cc:r3",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        late_now - Duration::minutes(5),
        "s-1",
        None,
        100,
        0,
        0,
        100,
    );
    insert_event_with_cost(&conn, &e3, Some(0.5), CostKind::ApiEquivalent);
    // usado = 6.5, restante = 1.5, últimos 30 min = 0.5 -> tasa 1.0 $/h -> tardaría 1.5 h (16:10 > 15:00) -> None
    let late_statuses = window_status(&conn, "claude-pro", late_now).unwrap();
    let late_w5 = late_statuses
        .iter()
        .find(|w| w.name == WindowName::FiveHours)
        .unwrap();
    assert_eq!(late_w5.exhausts_at, None);

    let json = serde_json::to_value(late_w5).unwrap();
    assert!(json.get("unpricedEvents").is_some());
    assert!(json.get("calibratedBy").is_some());
    assert!(json.get("calibratedAt").is_some());
    assert!(
        json.get("resetsAt")
            .and_then(|v| v.as_str())
            .is_some_and(|s| s.ends_with('Z'))
    );
}

#[test]
fn calibracion_sin_uso_falla() {
    let conn = open_memory().unwrap();
    let now = utc("2026-11-27T12:00:00Z");
    assert!(calibrate(&conn, "claude-pro", WindowName::FiveHours, 40.0, now).is_err());
}
