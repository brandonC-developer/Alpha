use chrono::{DateTime, NaiveDate, Utc};
use gestor_core::cost::{CostKind, RecalcResult, apply_cost, compute_cost, upsert_price};
use gestor_core::db::open_memory;
use gestor_core::model::{Provider, Tool, UsageEvent, plan_for};
use gestor_core::pricing::{Price, PriceInput, find_price, seed_prices};
use gestor_core::time::fmt_utc;
use rusqlite::{Connection, params};

fn fixed_now() -> DateTime<Utc> {
    "2026-11-27T12:00:00Z".parse().unwrap()
}

fn sample_event(
    dedup_key: &str,
    tool: Tool,
    provider: Provider,
    model: &str,
    ts: &str,
) -> UsageEvent {
    UsageEvent {
        dedup_key: dedup_key.to_string(),
        tool,
        provider,
        model: model.to_string(),
        ts_utc: ts.parse::<DateTime<Utc>>().unwrap(),
        session_id: "s-001".to_string(),
        project: Some("Alpha".to_string()),
        input: 1_000,
        cache_write: 500,
        cache_read: 2_000,
        output: 400,
        reasoning: 100,
    }
}

fn insert_event_and_hourly(conn: &Connection, ev: &UsageEvent) {
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

    let hour_str = format!("{}00:00Z", &ts_str[..14]);
    conn.execute(
        "INSERT INTO hourly_agg (
            hour_utc, tool, provider, plan_id, model, project,
            turns, input, cache_write, cache_read, output,
            cost_real, cost_equiv, unpriced
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8, ?9, ?10, 0.0, 0.0, 1)
        ON CONFLICT(hour_utc, tool, provider, plan_id, model, project) DO UPDATE SET
            turns = turns + 1,
            input = input + excluded.input,
            cache_write = cache_write + excluded.cache_write,
            cache_read = cache_read + excluded.cache_read,
            output = output + excluded.output,
            unpriced = unpriced + 1",
        params![
            hour_str,
            ev.tool.as_str(),
            ev.provider.as_str(),
            plan_id,
            ev.model,
            ev.project.as_deref().unwrap_or(""),
            ev.input as i64,
            ev.cache_write as i64,
            ev.cache_read as i64,
            ev.output as i64,
        ],
    )
    .unwrap();
}

#[test]
fn formula_cuatro_tipos() {
    let p = Price {
        id: 1,
        provider: Provider::Anthropic,
        model_pattern: "claude-opus-5-5*".to_string(),
        valid_from: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
        valid_to: None,
        input: 5.0,
        output: 25.0,
        cache_write: 6.25,
        cache_read: 0.5,
        source_url: "https://example.com".to_string(),
        approximate: false,
        user_edited: false,
    };
    let ev = UsageEvent {
        dedup_key: "cc:msg_1:req_1".to_string(),
        tool: Tool::ClaudeCode,
        provider: Provider::Anthropic,
        model: "claude-opus-5-5".to_string(),
        ts_utc: "2026-11-26T15:00:00Z".parse().unwrap(),
        session_id: "s-001".to_string(),
        project: Some("Alpha".to_string()),
        input: 2,
        cache_write: 22_158,
        cache_read: 29_587,
        output: 415,
        reasoning: 271,
    };
    // (2*5 + 415*25 + 22158*6.25 + 29587*0.5) / 1e6 = 163666.0 / 1e6 = 0.163666
    assert!((compute_cost(&ev, &p) - 0.163666).abs() < 1e-7);
}

#[test]
fn razonamiento_no_se_cobra_dos_veces() {
    let p = Price {
        id: 1,
        provider: Provider::Anthropic,
        model_pattern: "claude-opus-5-5*".to_string(),
        valid_from: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
        valid_to: None,
        input: 4.0,
        output: 20.0,
        cache_write: 5.0,
        cache_read: 0.2,
        source_url: "https://example.com".to_string(),
        approximate: false,
        user_edited: false,
    };
    let mut ev_no_reasoning = sample_event(
        "cc:r0:1",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        "2026-11-26T15:00:00Z",
    );
    ev_no_reasoning.reasoning = 0;

    let mut ev_with_reasoning = ev_no_reasoning.clone();
    ev_with_reasoning.reasoning = 271;

    assert_eq!(
        compute_cost(&ev_no_reasoning, &p),
        compute_cost(&ev_with_reasoning, &p)
    );
}

#[test]
fn sin_precio_es_unknown_y_null() {
    let conn = open_memory().unwrap();
    for (idx, model) in ["modelo-raro", "gemini-flash-worker-agy"]
        .iter()
        .enumerate()
    {
        let key = format!("ag:s1:{idx}");
        let ev = sample_event(
            &key,
            Tool::Antigravity,
            Provider::Google,
            model,
            "2026-11-26T15:00:00Z",
        );
        insert_event_and_hourly(&conn, &ev);
        apply_cost(&conn, &key, fixed_now()).unwrap();

        let (cost_usd, cost_kind, price_id, computed_at): (
            Option<f64>,
            String,
            Option<i64>,
            Option<String>,
        ) = conn
            .query_row(
                "SELECT cost_usd, cost_kind, price_id, cost_computed_at FROM usage_event WHERE dedup_key = ?1",
                [&key],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(cost_usd, None);
        assert_eq!(cost_kind, CostKind::Unknown.as_str());
        assert_eq!(price_id, None);
        assert!(computed_at.unwrap().ends_with('Z'));
    }
}

#[test]
fn suscripcion_es_api_equivalent() {
    let conn = open_memory().unwrap();

    let cc = sample_event(
        "cc:1:1",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        "2026-11-26T15:00:00Z",
    );
    insert_event_and_hourly(&conn, &cc);
    apply_cost(&conn, &cc.dedup_key, fixed_now()).unwrap();

    let (cc_cost, cc_kind): (Option<f64>, String) = conn
        .query_row(
            "SELECT cost_usd, cost_kind FROM usage_event WHERE dedup_key = ?1",
            [&cc.dedup_key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(cc_cost.unwrap() > 0.0);
    assert_eq!(cc_kind, CostKind::ApiEquivalent.as_str());

    let ag_claude = sample_event(
        "ag:1:1",
        Tool::Antigravity,
        Provider::Anthropic,
        "claude-opus-5-5-high",
        "2026-11-26T15:00:00Z",
    );
    insert_event_and_hourly(&conn, &ag_claude);
    apply_cost(&conn, &ag_claude.dedup_key, fixed_now()).unwrap();
    let ag_kind: String = conn
        .query_row(
            "SELECT cost_kind FROM usage_event WHERE dedup_key = ?1",
            [&ag_claude.dedup_key],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(ag_kind, CostKind::ApiEquivalent.as_str());

    let oc_ds = sample_event(
        "oc:1",
        Tool::Opencode,
        Provider::DeepSeek,
        "deepseek-flash",
        "2026-11-26T15:00:00Z",
    );
    insert_event_and_hourly(&conn, &oc_ds);
    apply_cost(&conn, &oc_ds.dedup_key, fixed_now()).unwrap();
    let (ds_cost, ds_kind): (Option<f64>, String) = conn
        .query_row(
            "SELECT cost_usd, cost_kind FROM usage_event WHERE dedup_key = ?1",
            [&oc_ds.dedup_key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(ds_cost.unwrap() > 0.0);
    assert_eq!(ds_kind, CostKind::Real.as_str());

    let oc_free = sample_event(
        "oc:2",
        Tool::Opencode,
        Provider::Other,
        "deepseek-v4.1-flash-free",
        "2026-11-26T15:00:00Z",
    );
    insert_event_and_hourly(&conn, &oc_free);
    apply_cost(&conn, &oc_free.dedup_key, fixed_now()).unwrap();
    let (free_cost, free_kind): (Option<f64>, String) = conn
        .query_row(
            "SELECT cost_usd, cost_kind FROM usage_event WHERE dedup_key = ?1",
            [&oc_free.dedup_key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(free_cost, Some(0.0));
    assert_eq!(free_kind, CostKind::Real.as_str());

    let oc_pickle = sample_event(
        "oc:3",
        Tool::Opencode,
        Provider::Other,
        "big-pickle",
        "2026-11-26T15:00:00Z",
    );
    insert_event_and_hourly(&conn, &oc_pickle);
    apply_cost(&conn, &oc_pickle.dedup_key, fixed_now()).unwrap();
    let (pickle_cost, pickle_kind): (Option<f64>, String) = conn
        .query_row(
            "SELECT cost_usd, cost_kind FROM usage_event WHERE dedup_key = ?1",
            [&oc_pickle.dedup_key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(pickle_cost, Some(0.0));
    assert_eq!(pickle_kind, CostKind::Real.as_str());
}

#[test]
fn patron_mas_largo_gana() {
    let mut conn = open_memory().unwrap();
    let d = NaiveDate::from_ymd_opt(2026, 11, 26).unwrap();

    upsert_price(
        &mut conn,
        PriceInput {
            provider: Provider::Google,
            model_pattern: "gemini-4-*".to_string(),
            valid_from: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            input: 1.0,
            output: 2.0,
            cache_write: 0.0,
            cache_read: 0.1,
            source_url: "https://example.com".to_string(),
            approximate: false,
        },
        fixed_now(),
    )
    .unwrap();
    upsert_price(
        &mut conn,
        PriceInput {
            provider: Provider::Google,
            model_pattern: "gemini-4-neon-*".to_string(),
            valid_from: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            input: 9.0,
            output: 18.0,
            cache_write: 0.0,
            cache_read: 0.9,
            source_url: "https://example.com".to_string(),
            approximate: false,
        },
        fixed_now(),
    )
    .unwrap();

    let matched = find_price(&conn, Provider::Google, "gemini-4-neon-high", d)
        .unwrap()
        .unwrap();
    assert_eq!(matched.model_pattern, "gemini-4-neon-*");
    assert!((matched.input - 9.0).abs() < 1e-9);
}

#[test]
fn precio_por_fecha() {
    let conn = open_memory().unwrap();
    let before_bump = NaiveDate::from_ymd_opt(2026, 11, 30).unwrap();
    let on_bump = NaiveDate::from_ymd_opt(2026, 12, 1).unwrap();

    let p_old = find_price(&conn, Provider::Google, "gemini-4-carbon-high", before_bump)
        .unwrap()
        .unwrap();
    assert!((p_old.input - 2.0).abs() < 1e-9);
    assert!((p_old.output - 10.0).abs() < 1e-9);
    assert_eq!(p_old.valid_to, Some(on_bump));

    let p_new = find_price(&conn, Provider::Google, "gemini-4-carbon-high", on_bump)
        .unwrap()
        .unwrap();
    assert!((p_new.input - 4.0).abs() < 1e-9);
    assert!((p_new.output - 20.0).abs() < 1e-9);
    assert_eq!(p_new.valid_to, None);

    let f_2026 = find_price(
        &conn,
        Provider::Google,
        "gemini-3.8-flash-medium",
        NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
    )
    .unwrap()
    .unwrap();
    assert!((f_2026.input - 0.75).abs() < 1e-9);

    let f_2027 = find_price(
        &conn,
        Provider::Google,
        "gemini-3.8-flash-medium",
        NaiveDate::from_ymd_opt(2027, 1, 1).unwrap(),
    )
    .unwrap()
    .unwrap();
    assert!((f_2027.input - 1.5).abs() < 1e-9);
}

#[test]
fn recalculo_solo_toca_eventos_afectados() {
    let mut conn = open_memory().unwrap();

    let ev_before = sample_event(
        "cc:before",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        "2026-11-15T10:00:00Z",
    );
    let ev_on = sample_event(
        "cc:on",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        "2026-11-20T00:00:00Z",
    );
    let ev_after = sample_event(
        "cc:after",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        "2026-11-26T18:00:00Z",
    );
    let ev_other = sample_event(
        "ag:other",
        Tool::Antigravity,
        Provider::Google,
        "gemini-4-neon-xhigh",
        "2026-11-25T12:00:00Z",
    );

    for ev in [&ev_before, &ev_on, &ev_after, &ev_other] {
        insert_event_and_hourly(&conn, ev);
        apply_cost(&conn, &ev.dedup_key, fixed_now()).unwrap();
    }

    // Sincronizar hourly_agg con costes iniciales para comprobar que upsert_price también lo actualiza
    conn.execute("DELETE FROM hourly_agg", []).unwrap();
    conn.execute(
        "INSERT INTO hourly_agg (
            hour_utc, tool, provider, plan_id, model, project,
            turns, input, cache_write, cache_read, output,
            cost_real, cost_equiv, unpriced
         )
         SELECT substr(ts_utc, 1, 13) || ':00:00Z', tool, provider, plan_id, model, COALESCE(project, ''),
                COUNT(*), SUM(input), SUM(cache_write), SUM(cache_read), SUM(output),
                0.0, COALESCE(SUM(cost_usd), 0.0), 0
         FROM usage_event
         GROUP BY 1, 2, 3, 4, 5, 6",
        [],
    )
    .unwrap();

    let before_cost_initial: f64 = conn
        .query_row(
            "SELECT cost_usd FROM usage_event WHERE dedup_key = 'cc:before'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let before_price_id: i64 = conn
        .query_row(
            "SELECT price_id FROM usage_event WHERE dedup_key = 'cc:before'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let other_cost_initial: f64 = conn
        .query_row(
            "SELECT cost_usd FROM usage_event WHERE dedup_key = 'ag:other'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let affected_old_sum: f64 = conn
        .query_row(
            "SELECT SUM(cost_usd) FROM usage_event WHERE dedup_key IN ('cc:on', 'cc:after')",
            [],
            |r| r.get(0),
        )
        .unwrap();

    let res: RecalcResult = upsert_price(
        &mut conn,
        PriceInput {
            provider: Provider::Anthropic,
            model_pattern: "claude-opus-5-5*".to_string(),
            valid_from: NaiveDate::from_ymd_opt(2026, 11, 20).unwrap(),
            input: 8.0,
            output: 40.0,
            cache_write: 10.0,
            cache_read: 0.40,
            source_url: "https://platform.claude.com/docs/en/about-claude/pricing".to_string(),
            approximate: false,
        },
        fixed_now(),
    )
    .unwrap();

    assert_eq!(res.events, 2);
    assert!((res.old_total - affected_old_sum).abs() < 1e-12);

    let affected_new_sum: f64 = conn
        .query_row(
            "SELECT SUM(cost_usd) FROM usage_event WHERE dedup_key IN ('cc:on', 'cc:after')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!((res.new_total - affected_new_sum).abs() < 1e-12);
    assert!((res.new_total - res.old_total * 2.0).abs() < 1e-12);

    // Eventos fuera de rango o de otro modelo permanecen intactos
    let (before_cost_after, before_pid_after): (f64, i64) = conn
        .query_row(
            "SELECT cost_usd, price_id FROM usage_event WHERE dedup_key = 'cc:before'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!((before_cost_after - before_cost_initial).abs() < 1e-12);
    assert_eq!(before_pid_after, before_price_id);

    let other_cost_after: f64 = conn
        .query_row(
            "SELECT cost_usd FROM usage_event WHERE dedup_key = 'ag:other'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!((other_cost_after - other_cost_initial).abs() < 1e-12);

    // 1 fila en cost_recalc_log
    let log_row: (String, i64, f64, f64) = conn
        .query_row(
            "SELECT at, events, old_total, new_total FROM cost_recalc_log",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert!(log_row.0.ends_with('Z'));
    assert_eq!(log_row.1, 2);
    assert!((log_row.2 - res.old_total).abs() < 1e-12);
    assert!((log_row.3 - res.new_total).abs() < 1e-12);

    // hourly_agg cuadra con usage_event
    let total_event_cost: f64 = conn
        .query_row("SELECT SUM(cost_usd) FROM usage_event", [], |r| r.get(0))
        .unwrap();
    let total_agg_equiv: f64 = conn
        .query_row("SELECT SUM(cost_equiv) FROM hourly_agg", [], |r| r.get(0))
        .unwrap();
    assert!((total_event_cost - total_agg_equiv).abs() < 1e-12);
}

#[test]
fn seed_no_pisa_precios_editados() {
    let mut conn = open_memory().unwrap();
    upsert_price(
        &mut conn,
        PriceInput {
            provider: Provider::Anthropic,
            model_pattern: "claude-opus-5-5*".to_string(),
            valid_from: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            input: 99.0,
            output: 199.0,
            cache_write: 10.0,
            cache_read: 1.0,
            source_url: "https://manual.example".to_string(),
            approximate: false,
        },
        fixed_now(),
    )
    .unwrap();

    seed_prices(&conn).unwrap();

    let p = find_price(
        &conn,
        Provider::Anthropic,
        "claude-opus-5-5",
        NaiveDate::from_ymd_opt(2026, 11, 26).unwrap(),
    )
    .unwrap()
    .unwrap();
    assert!(p.user_edited);
    assert!((p.input - 99.0).abs() < 1e-9);
    assert!((p.output - 199.0).abs() < 1e-9);
}

#[test]
fn orchestrators_llevan_precios_aproximados() {
    let conn = open_memory().unwrap();
    let nov = NaiveDate::from_ymd_opt(2026, 11, 27).unwrap();
    let dec = NaiveDate::from_ymd_opt(2026, 12, 1).unwrap();

    let neon = find_price(&conn, Provider::Google, "gemini-pro-orchestrator-neon", nov)
        .unwrap()
        .unwrap();
    assert!(neon.approximate);
    assert!((neon.input - 3.0).abs() < 1e-9);
    assert!((neon.output - 15.0).abs() < 1e-9);

    let carbon_nov = find_price(&conn, Provider::Google, "gemini-pro-orchestrator", nov)
        .unwrap()
        .unwrap();
    assert!(carbon_nov.approximate);
    assert!((carbon_nov.input - 2.0).abs() < 1e-9);

    let carbon_dec = find_price(&conn, Provider::Google, "gemini-pro-orchestrator", dec)
        .unwrap()
        .unwrap();
    assert!(carbon_dec.approximate);
    assert!((carbon_dec.input - 4.0).abs() < 1e-9);

    let argon_nov = find_price(
        &conn,
        Provider::Google,
        "gemini-pro-orchestrator-argon",
        nov,
    )
    .unwrap()
    .unwrap();
    assert!(argon_nov.approximate);
    assert!((argon_nov.input - 2.0).abs() < 1e-9);

    let argon_dec = find_price(
        &conn,
        Provider::Google,
        "gemini-pro-orchestrator-argon",
        dec,
    )
    .unwrap()
    .unwrap();
    assert!(argon_dec.approximate);
    assert!((argon_dec.input - 4.0).abs() < 1e-9);
}

#[test]
fn alta_de_precio_para_evento_sin_precio_actualiza_hourly_agg_y_respeta_precedencia() {
    let mut conn = open_memory().unwrap();

    // Evento sin precio inicial (gemini-flash-worker-agy) y evento con patrón más largo (gemini-4-neon-xhigh)
    let ev_unknown = sample_event(
        "ag:worker:1",
        Tool::Antigravity,
        Provider::Google,
        "gemini-flash-worker-agy",
        "2026-11-26T15:00:00Z",
    );
    let ev_neon = sample_event(
        "ag:neon:1",
        Tool::Antigravity,
        Provider::Google,
        "gemini-4-neon-xhigh",
        "2026-11-26T15:00:00Z",
    );
    insert_event_and_hourly(&conn, &ev_unknown);
    apply_cost(&conn, &ev_unknown.dedup_key, fixed_now()).unwrap();
    insert_event_and_hourly(&conn, &ev_neon);
    apply_cost(&conn, &ev_neon.dedup_key, fixed_now()).unwrap();

    // Sincronizar hourly_agg con el estado inicial tras apply_cost
    let neon_cost_before: f64 = conn
        .query_row(
            "SELECT cost_usd FROM usage_event WHERE dedup_key = 'ag:neon:1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    conn.execute("DELETE FROM hourly_agg", []).unwrap();
    conn.execute(
        "INSERT INTO hourly_agg (
            hour_utc, tool, provider, plan_id, model, project,
            turns, input, cache_write, cache_read, output,
            cost_real, cost_equiv, unpriced
         )
         SELECT substr(ts_utc, 1, 13) || ':00:00Z', tool, provider, plan_id, model, COALESCE(project, ''),
                COUNT(*), SUM(input), SUM(cache_write), SUM(cache_read), SUM(output),
                0.0,
                COALESCE(SUM(CASE WHEN cost_kind = 'api_equivalent' THEN cost_usd ELSE 0.0 END), 0.0),
                COALESCE(SUM(CASE WHEN cost_kind = 'unknown' THEN 1 ELSE 0 END), 0)
         FROM usage_event
         GROUP BY 1, 2, 3, 4, 5, 6",
        [],
    )
    .unwrap();

    // Un patrón más corto ("gemini-*") no debe tocar "gemini-4-neon-xhigh" (ya cubierto por "gemini-4-neon*"),
    // pero sí debe ponerle precio a "gemini-flash-worker-agy" que antes estaba unknown.
    let res = upsert_price(
        &mut conn,
        PriceInput {
            provider: Provider::Google,
            model_pattern: "gemini-*".to_string(),
            valid_from: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            input: 1.0,
            output: 2.0,
            cache_write: 0.0,
            cache_read: 0.1,
            source_url: "https://example.com".to_string(),
            approximate: true,
        },
        fixed_now(),
    )
    .unwrap();

    assert_eq!(res.events, 1);
    assert!((res.old_total - 0.0).abs() < 1e-12);
    assert!(res.new_total > 0.0);

    let (unpriced_sum, cost_equiv_sum): (i64, f64) = conn
        .query_row(
            "SELECT SUM(unpriced), SUM(cost_equiv) FROM hourly_agg",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(unpriced_sum, 0);
    assert!((cost_equiv_sum - (neon_cost_before + res.new_total)).abs() < 1e-12);
}

#[test]
fn precios_haiku_fable_y_sonnet() {
    let conn = open_memory().unwrap();
    let d = NaiveDate::from_ymd_opt(2026, 11, 27).unwrap();

    let haiku = find_price(&conn, Provider::Anthropic, "claude-haiku-5-5-20261001", d)
        .unwrap()
        .unwrap();
    assert_eq!(haiku.provider, Provider::Anthropic);
    assert!((haiku.input - 0.10).abs() < 1e-9);
    assert!((haiku.output - 0.50).abs() < 1e-9);
    assert!((haiku.cache_write - 0.125).abs() < 1e-9);
    assert!((haiku.cache_read - 0.01).abs() < 1e-9);
    assert!(haiku.approximate);

    let fable = find_price(&conn, Provider::Anthropic, "claude-fable-5-2", d)
        .unwrap()
        .unwrap();
    assert_eq!(fable.provider, Provider::Anthropic);
    assert!((fable.input - 8.0).abs() < 1e-9);
    assert!((fable.output - 40.0).abs() < 1e-9);
    assert!((fable.cache_write - 10.0).abs() < 1e-9);
    assert!((fable.cache_read - 0.20).abs() < 1e-9);
    assert!(!fable.approximate);

    let sonnet = find_price(&conn, Provider::Anthropic, "claude-sonnet-5-5", d)
        .unwrap()
        .unwrap();
    assert_eq!(sonnet.provider, Provider::Anthropic);
    assert!((sonnet.input - 2.0).abs() < 1e-9);
    assert!((sonnet.output - 10.0).abs() < 1e-9);
    assert!((sonnet.cache_write - 2.5).abs() < 1e-9);
    assert!((sonnet.cache_read - 0.10).abs() < 1e-9);
    assert!(!sonnet.approximate);
}

#[test]
fn cambio_precio_carbon_en_borde_utc() {
    let conn = open_memory().unwrap();

    let mut ev_nov = sample_event(
        "ag:carbon:nov",
        Tool::Antigravity,
        Provider::Google,
        "gemini-4-carbon-high",
        "2026-11-30T23:30:00Z",
    );
    ev_nov.input = 1_000_000;
    ev_nov.cache_write = 0;
    ev_nov.cache_read = 0;
    ev_nov.output = 1_000_000;
    ev_nov.reasoning = 0;

    let mut ev_dec = ev_nov.clone();
    ev_dec.dedup_key = "ag:carbon:dec".to_string();
    ev_dec.ts_utc = "2026-12-01T00:30:00Z".parse().unwrap();

    insert_event_and_hourly(&conn, &ev_nov);
    insert_event_and_hourly(&conn, &ev_dec);
    apply_cost(&conn, &ev_nov.dedup_key, fixed_now()).unwrap();
    apply_cost(&conn, &ev_dec.dedup_key, fixed_now()).unwrap();

    let nov_cost: f64 = conn
        .query_row(
            "SELECT cost_usd FROM usage_event WHERE dedup_key = 'ag:carbon:nov'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let dec_cost: f64 = conn
        .query_row(
            "SELECT cost_usd FROM usage_event WHERE dedup_key = 'ag:carbon:dec'",
            [],
            |r| r.get(0),
        )
        .unwrap();

    // Nov: 1M*2 + 1M*10 = 12 $; Dic: 1M*4 + 1M*20 = 24 $
    assert!((nov_cost - 12.0).abs() < 1e-9);
    assert!((dec_cost - 24.0).abs() < 1e-9);
}

#[test]
fn deepseek_flash_free_de_other_vale_cero_real() {
    let conn = open_memory().unwrap();
    let ev = sample_event(
        "oc:free-ds",
        Tool::Opencode,
        Provider::Other,
        "deepseek-flash-free",
        "2026-11-26T15:00:00Z",
    );
    insert_event_and_hourly(&conn, &ev);
    apply_cost(&conn, &ev.dedup_key, fixed_now()).unwrap();

    let (cost_usd, cost_kind): (Option<f64>, String) = conn
        .query_row(
            "SELECT cost_usd, cost_kind FROM usage_event WHERE dedup_key = ?1",
            [&ev.dedup_key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(cost_usd, Some(0.0));
    assert_eq!(cost_kind, CostKind::Real.as_str());
}

#[test]
fn upsert_price_comodin_deepseek_no_toca_google() {
    let mut conn = open_memory().unwrap();
    let ev_google = sample_event(
        "ag:worker:prov",
        Tool::Antigravity,
        Provider::Google,
        "gemini-flash-worker-agy",
        "2026-11-26T15:00:00Z",
    );
    insert_event_and_hourly(&conn, &ev_google);
    apply_cost(&conn, &ev_google.dedup_key, fixed_now()).unwrap();

    let res = upsert_price(
        &mut conn,
        PriceInput {
            provider: Provider::DeepSeek,
            model_pattern: "*".to_string(),
            valid_from: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            input: 1.0,
            output: 2.0,
            cache_write: 0.0,
            cache_read: 0.0,
            source_url: "https://example.com".to_string(),
            approximate: true,
        },
        fixed_now(),
    )
    .unwrap();

    assert_eq!(res.events, 0);
    let (cost_usd, cost_kind): (Option<f64>, String) = conn
        .query_row(
            "SELECT cost_usd, cost_kind FROM usage_event WHERE dedup_key = ?1",
            [&ev_google.dedup_key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(cost_usd, None);
    assert_eq!(cost_kind, CostKind::Unknown.as_str());
}

#[test]
fn apply_cost_y_upsert_price_usan_now_inyectado() {
    let mut conn = open_memory().unwrap();
    let ev = sample_event(
        "cc:now:1",
        Tool::ClaudeCode,
        Provider::Anthropic,
        "claude-opus-5-5",
        "2026-11-26T15:00:00Z",
    );
    insert_event_and_hourly(&conn, &ev);

    let t_apply: DateTime<Utc> = "2026-11-20T08:09:10Z".parse().unwrap();
    apply_cost(&conn, &ev.dedup_key, t_apply).unwrap();

    let computed_at: String = conn
        .query_row(
            "SELECT cost_computed_at FROM usage_event WHERE dedup_key = ?1",
            [&ev.dedup_key],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(computed_at, "2026-11-20T08:09:10Z");

    let t_upsert: DateTime<Utc> = "2026-11-21T09:10:11Z".parse().unwrap();
    upsert_price(
        &mut conn,
        PriceInput {
            provider: Provider::Anthropic,
            model_pattern: "claude-opus-5-5*".to_string(),
            valid_from: NaiveDate::from_ymd_opt(2026, 11, 20).unwrap(),
            input: 8.0,
            output: 40.0,
            cache_write: 10.0,
            cache_read: 0.4,
            source_url: "https://example.com".to_string(),
            approximate: false,
        },
        t_upsert,
    )
    .unwrap();

    let computed_after: String = conn
        .query_row(
            "SELECT cost_computed_at FROM usage_event WHERE dedup_key = ?1",
            [&ev.dedup_key],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(computed_after, "2026-11-21T09:10:11Z");

    let log_at: String = conn
        .query_row("SELECT at FROM cost_recalc_log", [], |r| r.get(0))
        .unwrap();
    assert_eq!(log_at, "2026-11-21T09:10:11Z");
}

#[test]
fn upsert_price_usa_ix_event_ts() {
    let conn = open_memory().unwrap();
    let mut stmt = conn
        .prepare(
            "EXPLAIN QUERY PLAN
             SELECT e.dedup_key
             FROM usage_event e
             JOIN plan pl ON pl.id = e.plan_id
             WHERE e.ts_utc >= '2026-12-01T00:00:00Z' AND e.provider = 'google'",
        )
        .unwrap();
    let details: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(3))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(
        details.iter().any(|d| d.contains("ix_event_ts")),
        "plan inesperado: {details:?}"
    );
}
