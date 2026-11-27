use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Timelike, Utc};
use rusqlite::{Connection, OptionalExtension, params};

use crate::collectors::Collector;
use crate::collectors::antigravity::AntigravityCollector;
use crate::collectors::claude_code::ClaudeCodeCollector;
use crate::error::{CoreError, Result};
use crate::model::plan_for;
use crate::time::{fmt_utc, hour_utc_str};

/// Resumen numérico devuelto por las operaciones de ingestión.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IngestStats {
    pub files_seen: u64,
    pub files_read: u64,
    pub files_failed: u64,
    pub new_events: u64,
    pub duplicates: u64,
    pub line_errors: u64,
}

impl IngestStats {
    fn accumulate(&mut self, other: Self) {
        self.files_seen += other.files_seen;
        self.files_read += other.files_read;
        self.files_failed += other.files_failed;
        self.new_events += other.new_events;
        self.duplicates += other.duplicates;
        self.line_errors += other.line_errors;
    }
}

/// Gancho para calcular el coste de un evento recién insertado (conectado en T6).
pub trait CostHook {
    fn apply(&self, conn: &Connection, dedup_key: &str) -> Result<()>;
}

/// Implementación por defecto que aplica el coste calculado desde la tabla de precios.
#[derive(Debug, Clone, Copy, Default)]
pub struct ApplyCostHook;

impl CostHook for ApplyCostHook {
    fn apply(&self, conn: &Connection, dedup_key: &str) -> Result<()> {
        crate::cost::apply_cost(conn, dedup_key)
    }
}

/// Implementación sin efecto útil en pruebas que aíslan la ingesta del cálculo de costes.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopCostHook;

impl CostHook for NoopCostHook {
    fn apply(&self, _conn: &Connection, _dedup_key: &str) -> Result<()> {
        Ok(())
    }
}

/// Ingiere de forma incremental e idempotente un archivo de registro usando `c`.
pub fn ingest_file(
    conn: &mut Connection,
    c: &dyn Collector,
    path: &Path,
    now: DateTime<Utc>,
) -> Result<IngestStats> {
    ingest_file_with_cost(conn, c, path, &ApplyCostHook, now)
}

/// Variante de [`ingest_file`] parametrizada con un [`CostHook`].
pub fn ingest_file_with_cost(
    conn: &mut Connection,
    c: &dyn Collector,
    path: &Path,
    cost_hook: &dyn CostHook,
    now: DateTime<Utc>,
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
            let saved_size = saved_size.max(0) as u64;
            let saved_offset = saved_offset.max(0) as u64;
            if size < saved_size
                || saved_offset > size
                || (saved_offset > 0 && !byte_before_offset_is_newline(path, saved_offset)?)
            {
                (0, true)
            } else {
                (saved_offset, false)
            }
        }
        None => (0, false),
    };

    let outcome = c.parse_file(path, from_offset)?;
    let now_str = fmt_utc(now);

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
    let mut touched_session_hours: HashSet<(String, String)> = HashSet::new();

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

            touched_session_hours.insert((ev.session_id.clone(), hour_str));
        }
    }

    if !touched_session_hours.is_empty() {
        refresh_session_hours(
            &tx,
            touched_session_hours
                .iter()
                .map(|(s, h)| (s.as_str(), h.as_str())),
        )?;
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
        files_failed: 0,
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
    now: DateTime<Utc>,
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
        match scan_one_file(conn, collector, &path, now) {
            Ok(Some(file_stats)) => stats.accumulate(file_stats),
            Ok(None) => stats.files_seen += 1,
            Err(CoreError::Io(_)) | Err(CoreError::Parse { .. }) => {
                stats.files_seen += 1;
                stats.files_failed += 1;
            }
            Err(err) => return Err(err),
        }
        progress((idx + 1) as u64, total);
    }

    Ok(stats)
}

fn scan_one_file(
    conn: &mut Connection,
    collector: &dyn Collector,
    path: &Path,
    now: DateTime<Utc>,
) -> Result<Option<IngestStats>> {
    if is_unchanged(conn, path)? {
        return Ok(None);
    }
    ingest_file(conn, collector, path, now).map(Some)
}

fn byte_before_offset_is_newline(path: &Path, offset: u64) -> Result<bool> {
    let mut file = fs::File::open(path)?;
    file.seek(SeekFrom::Start(offset - 1))?;
    let mut byte = [0u8; 1];
    let n = file.read(&mut byte)?;
    Ok(n == 1 && byte[0] == b'\n')
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

/// Recalcula en `session_hour` la pareja `(session_id, hour_utc)` desde `usage_event` (vía `ix_event_session`).
pub fn refresh_session_hour(conn: &Connection, session_id: &str, hour_utc: &str) -> Result<()> {
    refresh_session_hours(conn, std::iter::once((session_id, hour_utc)))
}

fn refresh_session_hours<'a, I>(conn: &Connection, pairs: I) -> Result<()>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    struct GroupAcc {
        events: i64,
        first_ts: String,
        last_ts: String,
        last_ep: i64,
        active_secs: i64,
        minute_mask: i64,
    }

    let mut delete_stmt =
        conn.prepare("DELETE FROM session_hour WHERE session_id = ?1 AND hour_utc = ?2")?;
    let mut select_stmt = conn.prepare(
        "SELECT tool, provider, COALESCE(project, ''), ts_utc
         FROM usage_event
         WHERE session_id = ?1 AND ts_utc >= ?2 AND ts_utc < ?3
         ORDER BY ts_utc ASC",
    )?;
    let mut insert_stmt = conn.prepare(
        "INSERT INTO session_hour (
            session_id, tool, provider, project, hour_utc,
            events, first_ts, last_ts, active_secs, minute_mask
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
    )?;

    for (session_id, hour_utc) in pairs {
        let hour_dt = hour_utc
            .parse::<DateTime<Utc>>()
            .map_err(|err| CoreError::Parse {
                file: "session_hour".to_string(),
                line: 0,
                reason: format!("hora UTC inválida '{hour_utc}': {err}"),
            })?;
        let hour_end = fmt_utc(hour_dt + Duration::hours(1));

        delete_stmt.execute(params![session_id, hour_utc])?;

        let mut groups: BTreeMap<(String, String, String), GroupAcc> = BTreeMap::new();
        let mut rows = select_stmt.query(params![session_id, hour_utc, &hour_end])?;
        while let Some(r) = rows.next()? {
            let tool: String = r.get(0)?;
            let provider: String = r.get(1)?;
            let project: String = r.get(2)?;
            let ts_str: String = r.get(3)?;
            let ts = ts_str
                .parse::<DateTime<Utc>>()
                .map_err(|err| CoreError::Parse {
                    file: "usage_event".to_string(),
                    line: 0,
                    reason: format!("fecha UTC inválida '{ts_str}': {err}"),
                })?;
            let ep = ts.timestamp();
            let bit = 1_i64 << ts.minute();

            let key = (tool, provider, project);
            if let Some(acc) = groups.get_mut(&key) {
                acc.events += 1;
                acc.last_ts = ts_str;
                acc.active_secs += (ep - acc.last_ep).clamp(0, 300);
                acc.last_ep = ep;
                acc.minute_mask |= bit;
            } else {
                groups.insert(
                    key,
                    GroupAcc {
                        events: 1,
                        first_ts: ts_str.clone(),
                        last_ts: ts_str,
                        last_ep: ep,
                        active_secs: 0,
                        minute_mask: bit,
                    },
                );
            }
        }

        for ((tool, provider, project), acc) in groups {
            insert_stmt.execute(params![
                session_id,
                &tool,
                &provider,
                &project,
                hour_utc,
                acc.events,
                &acc.first_ts,
                &acc.last_ts,
                acc.active_secs,
                acc.minute_mask,
            ])?;
        }
    }

    Ok(())
}
