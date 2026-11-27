use chrono::{FixedOffset, NaiveDate};
use gestor_core::db::{open_db, open_memory};
use gestor_core::model::{Provider, Tool, UsageEvent, plan_for, provider_for};
use gestor_core::time::{hours_between, local_date};

#[test]
fn esquema_se_aplica_una_vez() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("gestor.db");

    {
        let conn = open_db(&db_path).unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);

        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(journal_mode.to_lowercase(), "wal");

        let foreign_keys: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(foreign_keys, 1);
    }

    let conn2 = open_db(&db_path).unwrap();
    let version2: i64 = conn2
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version2, 1);
}

#[test]
fn tablas_existen() {
    let conn = open_memory().unwrap();
    let expected_tables = [
        "usage_event",
        "hourly_agg",
        "source_file",
        "ingest_error",
        "price",
        "cost_recalc_log",
        "plan",
        "plan_calibration",
        "alert_fired",
        "setting",
    ];
    for table in expected_tables {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "falta la tabla {table}");
    }

    let expected_indexes = [
        "ix_event_ts",
        "ix_event_session",
        "ix_event_model",
        "ix_event_plan",
    ];
    for index in expected_indexes {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
                [index],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "falta el índice {index}");
    }
}

#[test]
fn planes_sembrados() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("gestor.db");

    {
        let conn = open_db(&db_path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM plan", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 4);

        let expected = [
            (
                "claude-pro",
                "Claude Pro",
                "anthropic",
                "subscription",
                20.0_f64,
                None::<f64>,
                r#"[{"name":"5h","hours":5},{"name":"weekly","hours":168}]"#,
            ),
            (
                "google-ai-pro",
                "Google AI Pro · Gemini",
                "google",
                "subscription",
                20.0_f64,
                None::<f64>,
                r#"[{"name":"5h","hours":5},{"name":"weekly","hours":168}]"#,
            ),
            (
                "google-ai-pro-claude",
                "Google AI Pro · Claude y GPT",
                "anthropic",
                "subscription",
                0.0_f64,
                None::<f64>,
                r#"[{"name":"5h","hours":5},{"name":"weekly","hours":168}]"#,
            ),
            (
                "deepseek",
                "DeepSeek",
                "deepseek",
                "payg",
                0.0_f64,
                Some(5.0_f64),
                "[]",
            ),
        ];

        for (id, name, provider, kind, fee, budget, windows) in expected {
            let row: (String, String, String, f64, Option<f64>, String) = conn
                .query_row(
                    "SELECT name, provider, kind, monthly_fee_usd, budget_usd, windows FROM plan WHERE id = ?1",
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
                )
                .unwrap();
            assert_eq!(row.0, name);
            assert_eq!(row.1, provider);
            assert_eq!(row.2, kind);
            assert!((row.3 - fee).abs() < 1e-9);
            assert_eq!(row.4, budget);
            assert_eq!(row.5, windows);
        }

        conn.execute(
            "UPDATE plan SET monthly_fee_usd = 25.0 WHERE id = 'claude-pro'",
            [],
        )
        .unwrap();
    }

    let conn2 = open_db(&db_path).unwrap();
    let fee: f64 = conn2
        .query_row(
            "SELECT monthly_fee_usd FROM plan WHERE id = 'claude-pro'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!((fee - 25.0).abs() < 1e-9);
}

#[test]
fn plan_por_herramienta() {
    assert_eq!(
        plan_for(Tool::ClaudeCode, Provider::Anthropic),
        "claude-pro"
    );
    assert_eq!(
        plan_for(Tool::Antigravity, Provider::Google),
        "google-ai-pro"
    );
    assert_eq!(
        plan_for(Tool::Antigravity, Provider::Anthropic),
        "google-ai-pro-claude"
    );
    assert_eq!(plan_for(Tool::Opencode, Provider::DeepSeek), "deepseek");
    assert_eq!(plan_for(Tool::Opencode, Provider::Other), "deepseek");
    assert_eq!(
        plan_for(Tool::Antigravity, Provider::Other),
        "google-ai-pro"
    );

    assert_eq!(Tool::ClaudeCode.as_str(), "claude-code");
    assert_eq!(Tool::Antigravity.as_str(), "antigravity");
    assert_eq!(Tool::Opencode.as_str(), "opencode");
    assert_eq!("claude-code".parse::<Tool>().unwrap(), Tool::ClaudeCode);
    assert_eq!("antigravity".parse::<Tool>().unwrap(), Tool::Antigravity);
    assert_eq!("opencode".parse::<Tool>().unwrap(), Tool::Opencode);
    assert!("otro".parse::<Tool>().is_err());
}

#[test]
fn dedup_key_es_unica() {
    let conn = open_memory().unwrap();
    let ev = UsageEvent {
        dedup_key: "cc:msg_1:req_1".to_string(),
        tool: Tool::ClaudeCode,
        provider: Provider::Anthropic,
        model: "claude-opus-5-5".to_string(),
        ts_utc: "2026-11-26T15:00:00Z".parse().unwrap(),
        session_id: "s-001".to_string(),
        project: Some("Alpha".to_string()),
        input: 10,
        cache_write: 20,
        cache_read: 30,
        output: 40,
        reasoning: 5,
    };

    for _ in 0..2 {
        conn.execute(
            "INSERT OR IGNORE INTO usage_event (
                dedup_key, tool, provider, plan_id, model, ts_utc, session_id, project,
                input, cache_write, cache_read, output, reasoning
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            rusqlite::params![
                ev.dedup_key,
                ev.tool.as_str(),
                ev.provider.as_str(),
                plan_for(ev.tool, ev.provider),
                ev.model,
                ev.ts_utc.to_rfc3339(),
                ev.session_id,
                ev.project,
                ev.input,
                ev.cache_write,
                ev.cache_read,
                ev.output,
                ev.reasoning,
            ],
        )
        .unwrap();
    }

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM usage_event", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn provider_por_modelo() {
    assert_eq!(
        provider_for(Tool::ClaudeCode, "claude-opus-5-5", None),
        Provider::Anthropic
    );
    assert_eq!(
        provider_for(Tool::Antigravity, "gemini-4-neon-xhigh", None),
        Provider::Google
    );
    assert_eq!(
        provider_for(Tool::Opencode, "deepseek-v4-flash", Some("deepseek")),
        Provider::DeepSeek
    );
    assert_eq!(
        provider_for(Tool::Opencode, "big-pickle", Some("opencode")),
        Provider::Other
    );

    assert_eq!(Provider::Anthropic.as_str(), "anthropic");
    assert_eq!(Provider::Google.as_str(), "google");
    assert_eq!(Provider::DeepSeek.as_str(), "deepseek");
    assert_eq!(Provider::Other.as_str(), "other");
}

#[test]
fn dia_local_de_un_instante_utc() {
    let tz = FixedOffset::west_opt(6 * 3600).unwrap();
    assert_eq!(
        local_date("2026-11-27T03:30:00Z".parse().unwrap(), &tz),
        NaiveDate::from_ymd_opt(2026, 11, 26).unwrap()
    );

    let a = "2026-11-27T00:00:00Z".parse().unwrap();
    let b = "2026-11-27T05:30:00Z".parse().unwrap();
    assert!((hours_between(a, b) - 5.5).abs() < 1e-9);
}
