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

fn init_connection(conn: Connection) -> Result<Connection> {
    init_connection_with(conn, seed_plans)
}

fn init_connection_with<F>(mut conn: Connection, seed: F) -> Result<Connection>
where
    F: FnOnce(&Connection) -> Result<()>,
{
    conn.execute_batch(
        "PRAGMA busy_timeout = 5000;\nPRAGMA journal_mode = WAL;\nPRAGMA foreign_keys = ON;",
    )?;

    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version == 0 {
        let tx = conn.transaction()?;
        tx.execute_batch(SCHEMA_SQL)?;
        seed(&tx)?;
        tx.pragma_update(None, "user_version", 1)?;
        tx.commit()?;
    } else {
        seed(&conn)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CoreError;

    #[test]
    fn fallo_en_siembra_deja_bd_vacia_y_reapertura_funciona() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("gestor.db");

        {
            let conn = Connection::open(&db_path).unwrap();
            let err = init_connection_with(conn, |_tx| {
                Err(CoreError::Secret("fallo simulado en siembra".to_string()))
            })
            .unwrap_err();
            assert!(matches!(err, CoreError::Secret(_)));
        }

        {
            let check = Connection::open(&db_path).unwrap();
            let version: i64 = check
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .unwrap();
            assert_eq!(version, 0);

            let table_count: i64 = check
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(table_count, 0);
        }

        let reopened = open_db(&db_path).unwrap();
        let version: i64 = reopened
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);

        let plan_count: i64 = reopened
            .query_row("SELECT COUNT(*) FROM plan", [], |row| row.get(0))
            .unwrap();
        assert_eq!(plan_count, 4);
    }
}
