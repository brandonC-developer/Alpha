use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Timelike, Utc};
use rusqlite::{Connection, OptionalExtension, params};

use crate::collectors::Collector;
use crate::collectors::antigravity::AntigravityCollector;
use crate::collectors::claude_code::ClaudeCodeCollector;
use crate::error::Result;
use crate::model::plan_for;
use crate::time::fmt_utc;

/// Resumen numérico devuelto por las operaciones de ingestión.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IngestStats {
    pub files_seen: u64,
    pub files_read: u64,
    pub new_events: u64,
    pub duplicates: u64,
    pub line_errors: u64,
}

impl IngestStats {
    fn accumulate(&mut self, other: Self) {
        self.files_seen += other.files_seen;
        self.files_read += other.files_read;
        self.new_events += other.new_events;
        self.duplicates += other.duplicates;
        self.line_errors += other.line_errors;
    }
}

/// Gancho para calcular el coste de un evento recién insertado (conectado en T6).
pub trait CostHook {
    fn apply(&self, conn: &Connection, dedup_key: &str) -> Result<()>;
}

/// Implementación sin efecto mientras no exista el módulo de costes (T6).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopCostHook;

impl CostHook for NoopCostHook {
    fn apply(&self, _conn: &Connection, _dedup_key: &str) -> Result<()> {
        Ok(())
    }
}

/// Ingiere de forma incremental e idempotente un archivo de registro usando `c`.
pub fn ingest_file(conn: &mut Connection, c: &dyn Collector, path: &Path) -> Result<IngestStats> {
    ingest_file_with_cost(conn, c, path, &NoopCostHook)
}

/// Variante de [`ingest_file`] parametrizada con un [`CostHook`].
pub fn ingest_file_with_cost(
    conn: &mut Connection,
    c: &dyn Collector,
    path: &Path,
    cost_hook: &dyn CostHook,
) -> Result<IngestStats> {
    let meta = fs::metadata(path)?;
    let size = meta.len();
    let modified: DateTime<Utc> = meta.modified()?.into();
    let mtime_str = fmt_utc(modified);
    let path_str = path.to_string_lossy().into_owned();

    let existing: Option<(i64, i64)> = conn
        .query_row(
            "SELECT size, read_offset FROM source_file WHERE path = ?1",
            [&path_str],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;

    let (from_offset, rotated) = match existing {
        Some((saved_size, saved_offset)) => {
            if size < saved_size.max(0) as u64 {
                (0, true)
            } else {
                (saved_offset.max(0) as u64, false)
            }
        }
        None => (0, false),
    };

    let outcome = c.parse_file(path, from_offset)?;
    let now_str = fmt_utc(Utc::now());

    let tx = conn.transaction()?;

    tx.execute(
        "INSERT INTO source_file (collector, path, size, mtime, read_offset, last_read)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(path) DO UPDATE SET
           collector = excluded.collector,
           size = excluded.size,
           mtime = excluded.mtime,
           read_offset = excluded.read_offset,
           last_read = excluded.last_read",
        params![
            c.id(),
            &path_str,
            size as i64,
            &mtime_str,
            outcome.new_offset as i64,
            &now_str,
        ],
    )?;

    let source_file_id: i64 = tx.query_row(
        "SELECT id FROM source_file WHERE path = ?1",
        [&path_str],
        |r| r.get(0),
    )?;

    if rotated {
        tx.execute(
            "DELETE FROM ingest_error WHERE source_file_id = ?1",
            [source_file_id],
        )?;
    }

    let mut new_events: u64 = 0;
    let mut duplicates: u64 = 0;

    {
        let mut insert_event_stmt = tx.prepare(
            "INSERT OR IGNORE INTO usage_event (
                dedup_key, tool, provider, plan_id, model, ts_utc, session_id, project,
                input, cache_write, cache_read, output, reasoning, source_file_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        )?;
        let mut cost_lookup_stmt =
            tx.prepare("SELECT cost_usd, cost_kind FROM usage_event WHERE dedup_key = ?1")?;
        let mut upsert_hourly_stmt = tx.prepare(
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
        )?;

        for ev in &outcome.events {
            let plan_id = plan_for(ev.tool, ev.provider);
            let ts_str = fmt_utc(ev.ts_utc);
            let inserted = insert_event_stmt.execute(params![
                &ev.dedup_key,
                ev.tool.as_str(),
                ev.provider.as_str(),
                plan_id,
                &ev.model,
                &ts_str,
                &ev.session_id,
                ev.project.as_deref(),
                ev.input as i64,
                ev.cache_write as i64,
                ev.cache_read as i64,
                ev.output as i64,
                ev.reasoning as i64,
                source_file_id,
            ])?;

            if inserted == 0 {
                duplicates += 1;
                continue;
            }

            new_events += 1;
            cost_hook.apply(&tx, &ev.dedup_key)?;

            let (cost_usd, cost_kind): (Option<f64>, String) =
                cost_lookup_stmt.query_row([&ev.dedup_key], |r| Ok((r.get(0)?, r.get(1)?)))?;
            let (cost_real, cost_equiv, unpriced) = match (cost_kind.as_str(), cost_usd) {
                ("real", Some(c)) => (c, 0.0, 0_i64),
                ("api_equivalent", Some(c)) => (0.0, c, 0_i64),
                _ => (0.0, 0.0, 1_i64),
            };

            let hour_str = hour_utc_str(ev.ts_utc);
            let project_key = ev.project.as_deref().unwrap_or("");
            upsert_hourly_stmt.execute(params![
                &hour_str,
                ev.tool.as_str(),
                ev.provider.as_str(),
                plan_id,
                &ev.model,
                project_key,
                ev.input as i64,
                ev.cache_write as i64,
                ev.cache_read as i64,
                ev.output as i64,
                cost_real,
                cost_equiv,
                unpriced,
            ])?;
        }
    }

    if !outcome.errors.is_empty() {
        let mut insert_err_stmt = tx.prepare(
            "INSERT INTO ingest_error (source_file_id, line, reason, at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(source_file_id, line) DO UPDATE SET
               reason = excluded.reason,
               at = excluded.at",
        )?;
        for err in &outcome.errors {
            insert_err_stmt.execute(params![
                source_file_id,
                err.line as i64,
                &err.reason,
                &now_str,
            ])?;
        }
    }

    tx.commit()?;

    Ok(IngestStats {
        files_seen: 1,
        files_read: 1,
        new_events,
        duplicates,
        line_errors: outcome.errors.len() as u64,
    })
}

/// Recorre las raíces de todos los colectores, saltando archivos sin cambios.
pub fn scan_all(
    conn: &mut Connection,
    cs: &[Box<dyn Collector>],
    progress: &dyn Fn(u64, u64),
) -> Result<IngestStats> {
    let mut work: Vec<(&dyn Collector, PathBuf)> = Vec::new();
    for collector in cs {
        let mut paths = Vec::new();
        for root in collector.roots() {
            if !root.exists() {
                continue;
            }
            collect_files(collector.id(), &root, &mut paths);
        }
        paths.sort();
        paths.dedup();
        for path in paths {
            work.push((collector.as_ref(), path));
        }
    }

    let total = work.len() as u64;
    progress(0, total);

    let mut stats = IngestStats::default();
    for (idx, (collector, path)) in work.into_iter().enumerate() {
        if is_unchanged(conn, &path)? {
            stats.files_seen += 1;
            progress((idx + 1) as u64, total);
            continue;
        }

        let file_stats = ingest_file(conn, collector, &path)?;
        stats.accumulate(file_stats);
        progress((idx + 1) as u64, total);
    }

    Ok(stats)
}

fn is_unchanged(conn: &Connection, path: &Path) -> Result<bool> {
    let meta = fs::metadata(path)?;
    let size = meta.len() as i64;
    let modified: DateTime<Utc> = meta.modified()?.into();
    let mtime_str = fmt_utc(modified);
    let path_str = path.to_string_lossy();

    let unchanged = conn
        .query_row(
            "SELECT size = ?1 AND mtime = ?2 FROM source_file WHERE path = ?3",
            params![size, &mtime_str, path_str.as_ref()],
            |row| row.get::<_, bool>(0),
        )
        .optional()?
        .unwrap_or(false);

    Ok(unchanged)
}

fn collect_files(collector_id: &str, dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if file_type.is_dir() {
            collect_files(collector_id, &path, out);
        } else if file_type.is_file() && accepts_collector_path(collector_id, &path) {
            out.push(path);
        }
    }
}

fn accepts_collector_path(collector_id: &str, path: &Path) -> bool {
    match collector_id {
        "claude-code" => ClaudeCodeCollector::accepts_path(path),
        "antigravity" => AntigravityCollector::accepts_path(path),
        _ => true,
    }
}

fn hour_utc_str(ts: DateTime<Utc>) -> String {
    let truncated = ts
        .with_minute(0)
        .and_then(|d| d.with_second(0))
        .and_then(|d| d.with_nanosecond(0))
        .unwrap_or(ts);
    fmt_utc(truncated)
}
