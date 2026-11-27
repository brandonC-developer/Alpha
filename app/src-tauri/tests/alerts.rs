mod common;

use chrono::{DateTime, Duration, FixedOffset, Utc};
use common::{insert_event, insert_event_with_cost, make_event};
use gestor_core::alerts::{Alert, evaluate_alerts, evaluate_alerts_in_tz};
use gestor_core::cost::CostKind;
use gestor_core::db::open_memory;
use gestor_core::model::{Provider, Tool};
use gestor_core::plans::{WindowName, calibrate, update_plan};
use gestor_core::time::fmt_utc;
use rusqlite::params;

fn utc(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn utc_minus_6() -> FixedOffset {
    FixedOffset::west_opt(6 * 3600).unwrap()
}

#[test]
fn ventana_80_salta_una_vez() {
    let conn = open_memory().unwrap();
    let block_start = utc("2026-11-26T10:00:00Z");
    let cal_at = utc("2026-11-26T10:30:00Z");
    let now = utc("2026-11-26T11:00:00Z");

    let e1 = make_event(
        "cc:a1:1",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        block_start,
        "s-a1",
        Some("Alpha"),
        100,
        0,
        0,
        100,
    );
    insert_event_with_cost(&conn, &e1, Some(5.0), CostKind::ApiEquivalent);
    // 5.0 = 50 % -> límite = 10.0
    calibrate(&conn, "claude-pro", WindowName::FiveHours, 50.0, cal_at).unwrap();

    // Llevamos el uso a 8.1 / 10.0 = 81 % (sin consumo en los últimos 30 min)
    let e2 = make_event(
        "cc:a1:2",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        utc("2026-11-26T10:20:00Z"),
        "s-a1",
        Some("Alpha"),
        100,
        0,
        0,
        100,
    );
    insert_event_with_cost(&conn, &e2, Some(3.1), CostKind::ApiEquivalent);

    let first = evaluate_alerts(&conn, now).unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].rule, "window5h.80");
    assert_eq!(first[0].key, format!("claude-pro:{}", fmt_utc(block_start)));

    // Evaluar otra vez en el mismo bloque no vuelve a disparar
    let second = evaluate_alerts(&conn, now + Duration::minutes(5)).unwrap();
    assert!(second.is_empty());
}

#[test]
fn ventana_nueva_vuelve_a_avisar() {
    let conn = open_memory().unwrap();
    let b1_start = utc("2026-11-26T08:00:00Z");
    let b1_eval = utc("2026-11-26T08:30:00Z");

    let e1 = make_event(
        "cc:b1:1",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        b1_start,
        "s-b1",
        Some("Alpha"),
        100,
        0,
        0,
        100,
    );
    insert_event_with_cost(&conn, &e1, Some(8.2), CostKind::ApiEquivalent);
    // 8.2 = 82 % -> límite = 10.0
    calibrate(&conn, "claude-pro", WindowName::FiveHours, 82.0, b1_eval).unwrap();

    let alerts1 = evaluate_alerts(&conn, b1_eval).unwrap();
    assert_eq!(alerts1.len(), 1);
    assert_eq!(alerts1[0].rule, "window5h.80");

    // Nuevo bloque > 5 h después (14:00Z), usado 8.2 / 10.0 = 82 %
    let b2_start = utc("2026-11-26T14:00:00Z");
    let b2_eval = utc("2026-11-26T14:20:00Z");
    let e2 = make_event(
        "cc:b2:1",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        b2_start,
        "s-b2",
        Some("Alpha"),
        100,
        0,
        0,
        100,
    );
    insert_event_with_cost(&conn, &e2, Some(8.2), CostKind::ApiEquivalent);

    let alerts2 = evaluate_alerts(&conn, b2_eval).unwrap();
    assert_eq!(alerts2.len(), 1);
    assert_eq!(alerts2[0].rule, "window5h.80");
    assert_eq!(alerts2[0].key, format!("claude-pro:{}", fmt_utc(b2_start)));
}

#[test]
fn sin_calibrar_no_avisa() {
    let conn = open_memory().unwrap();
    let now = utc("2026-11-26T16:00:00Z");

    let ev = make_event(
        "cc:nocal:1",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        now - Duration::minutes(10),
        "s-nocal",
        Some("Alpha"),
        100_000,
        0,
        0,
        50_000,
    );
    insert_event_with_cost(&conn, &ev, Some(150.0), CostKind::ApiEquivalent);

    let alerts = evaluate_alerts(&conn, now).unwrap();
    assert!(alerts.is_empty());
}

#[test]
fn presupuesto_deepseek() {
    let conn = open_memory().unwrap();
    let tz = utc_minus_6();
    let now = utc("2026-11-26T16:00:00Z");

    update_plan(&conn, "deepseek", 0.0, Some(10.0)).unwrap();

    let ev = make_event(
        "oc:ds:1",
        Tool::Opencode,
        Provider::DeepSeek,
        "deepseek-flash",
        now - Duration::hours(1),
        "s-ds",
        Some("Alpha"),
        1000,
        0,
        0,
        1000,
    );
    insert_event_with_cost(&conn, &ev, Some(8.1), CostKind::Real);

    let alerts = evaluate_alerts_in_tz(&conn, now, &tz).unwrap();
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].rule, "budget.80");
    assert_eq!(alerts[0].key, "deepseek:2026-11");
    assert_eq!(alerts[0].title, "DeepSeek: 8,10 $ de 10 $ este mes");
    assert_eq!(alerts[0].body, "Saldo: 1,90 $.");

    // No se repite en el mismo mes
    let again = evaluate_alerts_in_tz(&conn, now + Duration::hours(2), &tz).unwrap();
    assert!(again.is_empty());
}

#[test]
fn sesion_anomala_avisa() {
    let conn = open_memory().unwrap();
    let base = utc("2026-11-01T10:00:00Z");

    // 15 sesiones normales (~1 M tokens cada una)
    for i in 0..15 {
        let delta = ((i % 3) as u64) * 50_000;
        let ev = make_event(
            &format!("ag:norm:{i}"),
            Tool::Antigravity,
            Provider::Google,
            "gemini-4-neon-xhigh",
            base + Duration::hours(i),
            &format!("s-norm-{i}"),
            None,
            900_000 + delta,
            0,
            0,
            100_000,
        );
        insert_event(&conn, &ev);
    }

    // Sesión anómala de 41,2 M tokens
    let spike = make_event(
        "ag:spike:1",
        Tool::Antigravity,
        Provider::Google,
        "gemini-4-neon-xhigh",
        base + Duration::hours(20),
        "s-spike-41m",
        None,
        40_000_000,
        0,
        0,
        1_200_000,
    );
    insert_event(&conn, &spike);

    let now = base + Duration::hours(21);
    let alerts = evaluate_alerts(&conn, now).unwrap();
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].rule, "anomaly");
    assert_eq!(alerts[0].key, "s-spike-41m");
    assert_eq!(alerts[0].title, "Sesión fuera de lo normal en Antigravity");
    assert!(alerts[0].body.starts_with("41,2 M tokens, "));
    assert!(alerts[0].body.contains("× tu media. Ábrela en Sesiones."));

    // No vuelve a avisar por la misma sesión
    let again = evaluate_alerts(&conn, now + Duration::minutes(10)).unwrap();
    assert!(again.is_empty());
}

#[test]
fn textos_en_espanol() {
    let conn = open_memory().unwrap();
    let tz = utc_minus_6();
    // 10:10 UTC = 04:10 local, fin de bloque 15:10 UTC = 09:10 local
    // Para que los textos coincidan en UTC: probamos en tz = Utc
    let block_start = utc("2026-11-26T10:10:00Z");
    let now = utc("2026-11-26T13:50:00Z");

    // 7.0 al inicio + 1.0 hace 15 min = 8.0 usado; calibrado al 80 % -> límite = 10.0
    // restante = 2.0, ritmo últimos 30 min = 1.0 / 30 min = 2.0 $/h -> 1 h?
    // Para que exhaustsAt = 14:20 (en 30 min): restante = 2.0, últimos 30 min = 2.0 -> 6.0 + 2.0 = 8.0
    let e1 = make_event(
        "cc:txt:1",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        block_start,
        "s-txt",
        Some("Alpha"),
        100,
        0,
        0,
        100,
    );
    let e2 = make_event(
        "cc:txt:2",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        now - Duration::minutes(10),
        "s-txt",
        Some("Alpha"),
        100,
        0,
        0,
        100,
    );
    insert_event_with_cost(&conn, &e1, Some(6.0), CostKind::ApiEquivalent);
    insert_event_with_cost(&conn, &e2, Some(2.0), CostKind::ApiEquivalent);
    calibrate(&conn, "claude-pro", WindowName::FiveHours, 80.0, now).unwrap();

    let alerts: Vec<Alert> = evaluate_alerts_in_tz(&conn, now, &Utc).unwrap();
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].title, "Claude Pro: 80 % de la ventana de 5 h");
    assert_eq!(
        alerts[0].body,
        "Se reinicia a las 15:10. A este ritmo se agota a las 14:20."
    );

    // También comprobamos que evaluate_alerts_in_tz respeta la zona horaria local
    let conn2 = open_memory().unwrap();
    insert_event_with_cost(&conn2, &e1, Some(6.0), CostKind::ApiEquivalent);
    insert_event_with_cost(&conn2, &e2, Some(2.0), CostKind::ApiEquivalent);
    calibrate(&conn2, "claude-pro", WindowName::FiveHours, 80.0, now).unwrap();
    let alerts_local = evaluate_alerts_in_tz(&conn2, now, &tz).unwrap();
    assert_eq!(
        alerts_local[0].body,
        "Se reinicia a las 09:10. A este ritmo se agota a las 08:20."
    );
}

#[test]
fn pausa_1h_registra_pero_no_notifica() {
    let conn = open_memory().unwrap();
    let now = utc("2026-11-26T16:00:00Z");

    // Pausado hasta 17:00Z
    conn.execute(
        "INSERT INTO setting (key, value) VALUES ('alerts.paused_until', ?1)",
        [fmt_utc(now + Duration::hours(1))],
    )
    .unwrap();

    let ev = make_event(
        "cc:p:1",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        now - Duration::minutes(10),
        "s-p",
        Some("Alpha"),
        100,
        0,
        0,
        100,
    );
    insert_event_with_cost(&conn, &ev, Some(8.5), CostKind::ApiEquivalent);
    calibrate(&conn, "claude-pro", WindowName::FiveHours, 85.0, now).unwrap();

    // Durante la pausa: devuelve 0 alertas para notificar, pero SÍ registra en alert_fired
    let during = evaluate_alerts(&conn, now).unwrap();
    assert!(during.is_empty());

    let fired_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM alert_fired WHERE rule = 'window5h.80'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(fired_count, 1);

    // Tras expirar la pausa, la alerta ya registrada no se repite
    let after = evaluate_alerts(&conn, now + Duration::hours(1)).unwrap();
    assert!(after.is_empty());
}

#[test]
fn semanal_y_95_y_umbrales_desde_setting() {
    let conn = open_memory().unwrap();
    let now = utc("2026-11-27T12:00:00Z");

    let ev = make_event(
        "ag:w:1",
        Tool::Antigravity,
        Provider::Google,
        "gemini-4-neon-xhigh",
        now - Duration::hours(2),
        "s-w1",
        None,
        100,
        0,
        0,
        100,
    );
    insert_event_with_cost(&conn, &ev, Some(9.6), CostKind::ApiEquivalent);
    // Calibramos 5h al 96 % y semanal al 80 %
    calibrate(&conn, "google-ai-pro", WindowName::FiveHours, 96.0, now).unwrap();
    calibrate(&conn, "google-ai-pro", WindowName::Weekly, 80.0, now).unwrap();

    let alerts = evaluate_alerts_in_tz(&conn, now, &Utc).unwrap();
    let rules: Vec<&str> = alerts.iter().map(|a| a.rule.as_str()).collect();
    assert!(rules.contains(&"window5h.80"));
    assert!(rules.contains(&"window5h.95"));
    assert!(rules.contains(&"weekly.80"));

    let weekly = alerts.iter().find(|a| a.rule == "weekly.80").unwrap();
    assert_eq!(weekly.key, "google-ai-pro:2026-W48");
    assert_eq!(
        weekly.title,
        "Google AI Pro · Gemini: 80 % del límite semanal"
    );
    assert_eq!(
        weekly.body,
        "Quedan 7 días para que se renueve por completo."
    );

    // Ahora probamos umbrales personalizados en setting y saldo explícito de DeepSeek
    let conn2 = open_memory().unwrap();
    conn2
        .execute_batch(
            "INSERT INTO setting (key, value) VALUES
               ('alert.window5h.levels', '[70]'),
               ('alert.weekly.level', '90'),
               ('alert.budget.level', '50'),
               ('alert.anomaly', 'false'),
               ('deepseek.balance_usd', '2.50');",
        )
        .unwrap();
    let ev2 = make_event(
        "oc:custom:1",
        Tool::Opencode,
        Provider::DeepSeek,
        "deepseek-flash",
        now - Duration::minutes(30),
        "s-c1",
        None,
        100,
        0,
        0,
        100,
    );
    insert_event_with_cost(&conn2, &ev2, Some(2.6), CostKind::Real); // 2.6 de 5 = 52 % >= 50 %

    let alerts2 = evaluate_alerts_in_tz(&conn2, now, &Utc).unwrap();
    assert_eq!(alerts2.len(), 1);
    assert_eq!(alerts2[0].rule, "budget.50");
    assert_eq!(alerts2[0].title, "DeepSeek: 2,60 $ de 5 $ este mes");
    assert_eq!(alerts2[0].body, "Saldo: 2,50 $.");

    // Guardado en alert_fired con timestamp ISO 8601 UTC con Z
    let at: String = conn2
        .query_row(
            "SELECT at FROM alert_fired WHERE rule = 'budget.50'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(at, fmt_utc(now));
    let _ = params![""];
}
