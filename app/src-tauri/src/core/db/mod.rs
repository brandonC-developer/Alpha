use std::path::Path;

use rusqlite::{Connection, params};

use crate::error::Result;

const SCHEMA_SQL: &str = include_str!("schema.sql");
const SUBSCRIPTION_WINDOWS: &str = r#"[{"name":"5h","hours":5},{"name":"weekly","hours":168}]"#;

/// Abre o crea la base de datos SQLite en `path`, aplicando el esquema si `user_version == 0`.
pub fn open_db(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    init_connection(conn)
}

/// Abre una base de datos SQLite en memoria con el esquema y los planes iniciales aplicados.
pub fn open_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    init_connection(conn)
}

fn init_connection(mut conn: Connection) -> Result<Connection> {
    conn.execute_batch("PRAGMA journal_mode = WAL;\nPRAGMA foreign_keys = ON;")?;

    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version == 0 {
        conn.execute_batch(SCHEMA_SQL)?;
        let tx = conn.transaction()?;
        seed_plans(&tx)?;
        tx.pragma_update(None, "user_version", 1)?;
        tx.commit()?;
    } else {
        seed_plans(&conn)?;
    }

    Ok(conn)
}

struct SeedPlan {
    id: &'static str,
    name: &'static str,
    provider: &'static str,
    kind: &'static str,
    monthly_fee_usd: f64,
    budget_usd: Option<f64>,
    windows: &'static str,
}

fn seed_plans(conn: &Connection) -> Result<()> {
    let plans = [
        SeedPlan {
            id: "claude-pro",
            name: "Claude Pro",
            provider: "anthropic",
            kind: "subscription",
            monthly_fee_usd: 20.0,
            budget_usd: None,
            windows: SUBSCRIPTION_WINDOWS,
        },
        SeedPlan {
            id: "google-ai-pro",
            name: "Google AI Pro · Gemini",
            provider: "google",
            kind: "subscription",
            monthly_fee_usd: 20.0,
            budget_usd: None,
            windows: SUBSCRIPTION_WINDOWS,
        },
        SeedPlan {
            id: "google-ai-pro-claude",
            name: "Google AI Pro · Claude y GPT",
            provider: "anthropic",
            kind: "subscription",
            monthly_fee_usd: 0.0,
            budget_usd: None,
            windows: SUBSCRIPTION_WINDOWS,
        },
        SeedPlan {
            id: "deepseek",
            name: "DeepSeek",
            provider: "deepseek",
            kind: "payg",
            monthly_fee_usd: 0.0,
            budget_usd: Some(5.0),
            windows: "[]",
        },
    ];

    let mut stmt = conn.prepare(
        "INSERT OR IGNORE INTO plan (id, name, provider, kind, monthly_fee_usd, budget_usd, windows)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    for plan in plans {
        stmt.execute(params![
            plan.id,
            plan.name,
            plan.provider,
            plan.kind,
            plan.monthly_fee_usd,
            plan.budget_usd,
            plan.windows,
        ])?;
    }
    Ok(())
}
