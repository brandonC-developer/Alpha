mod common;

use chrono::{DateTime, Duration, Utc};
use common::{insert_event_with_cost, make_event};
use gestor_core::agy_usage::{AgyBucket, apply_agy_usage, parse_agy_usage};
use gestor_core::cost::CostKind;
use gestor_core::db::open_memory;
use gestor_core::error::CoreError;
use gestor_core::model::{Provider, Tool};
use gestor_core::plans::{CalibrationSource, WindowName, window_status};
use rusqlite::params;

const FIXTURE_USAGE: &str = include_str!("fixtures/agy/usage.json");

fn utc(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

#[test]
fn agy_usage_se_interpreta() {
    let buckets = parse_agy_usage(FIXTURE_USAGE).unwrap();
    assert_eq!(buckets.len(), 4);

    let debug_str = format!("{buckets:?}");
    assert!(!debug_str.contains("secreto-del-prompt"));

    assert_eq!(
        buckets[0],
        AgyBucket {
            plan_id: "google-ai-pro".to_string(),
            window: WindowName::Weekly,
            remaining_fraction: 0.75,
            resets_at: Some(utc("2026-12-03T14:25:59Z")),
        }
    );
    assert_eq!(
        buckets[1],
        AgyBucket {
            plan_id: "google-ai-pro".to_string(),
            window: WindowName::FiveHours,
            remaining_fraction: 0.6,
            resets_at: Some(utc("2026-11-27T20:02:00Z")),
        }
    );
    assert_eq!(
        buckets[2],
        AgyBucket {
            plan_id: "google-ai-pro-claude".to_string(),
            window: WindowName::Weekly,
            remaining_fraction: 0.5,
            resets_at: Some(utc("2026-12-03T22:40:28Z")),
        }
    );
    assert_eq!(
        buckets[3],
        AgyBucket {
            plan_id: "google-ai-pro-claude".to_string(),
            window: WindowName::FiveHours,
            remaining_fraction: 1.0,
            resets_at: None,
        }
    );
}

#[test]
fn agy_usage_calibra() {
    let conn = open_memory().unwrap();
    let now = utc("2026-11-27T16:00:00Z");

    let ev = make_event(
        "ag:1:1",
        Tool::Antigravity,
        Provider::Google,
        "gemini-4-neon-xhigh",
        now - Duration::minutes(30),
        "conv-1",
        None,
        1000,
        0,
        0,
        500,
    );
    insert_event_with_cost(&conn, &ev, Some(3.0), CostKind::ApiEquivalent);

    let buckets = parse_agy_usage(FIXTURE_USAGE).unwrap();
    let inserted = apply_agy_usage(&conn, &buckets, now).unwrap();
    // google-ai-pro tiene uso en 5h y weekly; google-ai-pro-claude no tiene uso -> 2 calibraciones
    assert_eq!(inserted, 2);

    let (percent, limit_est, source, resets_at): (f64, f64, String, Option<String>) = conn
        .query_row(
            "SELECT percent, limit_est, source, resets_at
             FROM plan_calibration
             WHERE plan_id = 'google-ai-pro' AND win_name = '5h'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert!((percent - 40.0).abs() < 1e-9);
    assert!((limit_est - 7.5).abs() < 1e-9);
    assert_eq!(source, "agy");
    assert_eq!(resets_at.as_deref(), Some("2026-11-27T20:02:00Z"));

    // Menos de 20 min después: resets_at sigue siendo el oficial de agy
    let statuses = window_status(&conn, "google-ai-pro", now + Duration::minutes(10)).unwrap();
    let w5 = statuses
        .iter()
        .find(|w| w.name == WindowName::FiveHours)
        .unwrap();
    assert_eq!(w5.resets_at, Some(utc("2026-11-27T20:02:00Z")));
    assert_eq!(w5.calibrated_by, Some(CalibrationSource::Agy));
    assert_eq!(w5.calibrated_at, Some(now));

    // Más de 20 min después: resets_at pasa al fin del bloque de 5 h calculado por la app
    let statuses_old = window_status(&conn, "google-ai-pro", now + Duration::minutes(25)).unwrap();
    let w5_old = statuses_old
        .iter()
        .find(|w| w.name == WindowName::FiveHours)
        .unwrap();
    assert_eq!(w5_old.resets_at, Some(ev.ts_utc + Duration::hours(5)));
}

#[test]
fn agy_usage_sin_uso_no_calibra() {
    let conn = open_memory().unwrap();
    let now = utc("2026-11-27T16:00:00Z");

    // Evento para google-ai-pro-claude (así used > 0), pero el bucket 3p-5h tiene remaining_fraction = 1
    let ev_claude = make_event(
        "ag:claude:1",
        Tool::Antigravity,
        Provider::Anthropic,
        "claude-opus-5-5-high",
        now - Duration::minutes(15),
        "conv-2",
        None,
        100,
        0,
        0,
        50,
    );
    insert_event_with_cost(&conn, &ev_claude, Some(2.0), CostKind::ApiEquivalent);

    // Solo pasamos los buckets 5h: gemini-5h (sin uso en BD -> used 0) y 3p-5h (remaining_fraction = 1)
    let buckets = vec![
        AgyBucket {
            plan_id: "google-ai-pro".to_string(),
            window: WindowName::FiveHours,
            remaining_fraction: 0.6,
            resets_at: Some(utc("2026-11-27T20:02:00Z")),
        },
        AgyBucket {
            plan_id: "google-ai-pro-claude".to_string(),
            window: WindowName::FiveHours,
            remaining_fraction: 1.0,
            resets_at: None,
        },
    ];

    let inserted = apply_agy_usage(&conn, &buckets, now).unwrap();
    assert_eq!(inserted, 0);

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM plan_calibration", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn agy_usa_las_ultimas_8() {
    let conn = open_memory().unwrap();
    let base = utc("2026-11-27T12:00:00Z");

    let ev = make_event(
        "ag:8:1",
        Tool::Antigravity,
        Provider::Google,
        "gemini-4-neon-xhigh",
        base - Duration::minutes(30),
        "conv-8",
        None,
        100,
        0,
        0,
        50,
    );
    insert_event_with_cost(&conn, &ev, Some(2.0), CostKind::ApiEquivalent);

    // 2 calibraciones antiguas de 100.0 y 8 recientes de 10.0
    for i in 0..10_i64 {
        let limit_est = if i < 2 { 100.0 } else { 10.0 };
        let at = gestor_core::time::fmt_utc(base + Duration::minutes(i));
        conn.execute(
            "INSERT INTO plan_calibration (plan_id, win_name, at, percent, used, limit_est, source, resets_at)
             VALUES ('google-ai-pro', '5h', ?1, 20.0, 2.0, ?2, 'agy', NULL)",
            params![at, limit_est],
        )
        .unwrap();
    }

    let statuses = window_status(&conn, "google-ai-pro", base + Duration::minutes(15)).unwrap();
    let w5 = statuses
        .iter()
        .find(|w| w.name == WindowName::FiveHours)
        .unwrap();
    assert!((w5.limit.unwrap() - 10.0).abs() < 1e-9);
}

#[test]
fn agy_usage_malformado() {
    let broken = r#"{"status":"SUCCESS","command":{"name":"usage","data":{}}}"#;
    let err = parse_agy_usage(broken).unwrap_err();
    assert!(matches!(err, CoreError::Parse { .. }));

    let invalid_json = "esto no es json";
    let err2 = parse_agy_usage(invalid_json).unwrap_err();
    assert!(matches!(err2, CoreError::Parse { .. }));
}

#[test]
fn agy_usage_ignora_bucket_desconocido() {
    let json = r#"{"command":{"name":"usage","data":{"groups":[{"buckets":[{"id":"otro-bucket","window":"5h","remaining_fraction":0.5,"reset_time":"2026-11-27T20:00:00Z"},{"id":"gemini-5h","window":"5h","remaining_fraction":0.6,"reset_time":"2026-11-27T20:02:00Z"}]}]}}}"#;
    let buckets = parse_agy_usage(json).unwrap();
    assert_eq!(buckets.len(), 1);
    assert_eq!(buckets[0].plan_id, "google-ai-pro");
    assert_eq!(buckets[0].window, WindowName::FiveHours);
}
