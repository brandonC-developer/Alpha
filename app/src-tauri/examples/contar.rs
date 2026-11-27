use chrono::{Duration, Utc};
use gestor_core::collectors::Collector;
use gestor_core::collectors::antigravity::AntigravityCollector;
use gestor_core::collectors::claude_code::ClaudeCodeCollector;
use gestor_core::db::open_db;
use gestor_core::ingest::scan_all;
use gestor_core::time::fmt_utc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = tempfile::tempdir()?;
    let db_path = tmp.path().join("contar.db");
    let mut conn = open_db(&db_path)?;

    let collectors: Vec<Box<dyn Collector>> = vec![
        Box::new(ClaudeCodeCollector::default()),
        Box::new(AntigravityCollector::default()),
    ];

    let now = Utc::now();
    let stats = scan_all(&mut conn, &collectors, &|_, _| {}, now)?;
    let since = fmt_utc(now - Duration::days(7));

    println!(
        "Ingesta: archivos_vistos={} archivos_leidos={} archivos_fallidos={} eventos_nuevos={} duplicados={} errores_linea={}",
        stats.files_seen,
        stats.files_read,
        stats.files_failed,
        stats.new_events,
        stats.duplicates,
        stats.line_errors
    );
    println!("Totales de los últimos 7 días (desde {since}):");
    println!(
        "{:<13} {:<32} {:>8} {:>12} {:>12} {:>12} {:>12}",
        "herramienta", "modelo", "turnos", "input", "cache_write", "cache_read", "output"
    );

    let mut stmt = conn.prepare(
        "SELECT tool, model, COUNT(*) AS turns,
                COALESCE(SUM(input), 0) AS input_tokens,
                COALESCE(SUM(cache_write), 0) AS cache_write_tokens,
                COALESCE(SUM(cache_read), 0) AS cache_read_tokens,
                COALESCE(SUM(output), 0) AS output_tokens
         FROM usage_event
         WHERE ts_utc >= ?1
         GROUP BY tool, model
         ORDER BY tool ASC, (input_tokens + output_tokens) DESC, model ASC",
    )?;

    let rows = stmt.query_map([&since], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, i64>(5)?,
            row.get::<_, i64>(6)?,
        ))
    })?;

    let mut total_turns: i64 = 0;
    let mut total_input: i64 = 0;
    let mut total_cache_write: i64 = 0;
    let mut total_cache_read: i64 = 0;
    let mut total_output: i64 = 0;

    for row in rows {
        let (tool, model, turns, input, cache_write, cache_read, output) = row?;
        total_turns += turns;
        total_input += input;
        total_cache_write += cache_write;
        total_cache_read += cache_read;
        total_output += output;
        println!(
            "{:<13} {:<32} {:>8} {:>12} {:>12} {:>12} {:>12}",
            tool, model, turns, input, cache_write, cache_read, output
        );
    }

    println!(
        "{:<13} {:<32} {:>8} {:>12} {:>12} {:>12} {:>12}",
        "TOTAL", "-", total_turns, total_input, total_cache_write, total_cache_read, total_output
    );

    let since_30 = fmt_utc(now - Duration::days(30));
    let (equiv_30, unpriced_30): (f64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(CASE WHEN cost_kind = 'api_equivalent' THEN cost_usd ELSE 0.0 END), 0.0),
                COALESCE(SUM(CASE WHEN cost_kind = 'unknown' THEN 1 ELSE 0 END), 0)
         FROM usage_event
         WHERE ts_utc >= ?1",
        [&since_30],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    println!(
        "Coste equivalente últimos 30 días (desde {since_30}): ${equiv_30:.4} USD (sin precio: {unpriced_30})"
    );

    Ok(())
}
