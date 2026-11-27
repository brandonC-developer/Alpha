# Resultado Tarea 5: Ingestión incremental

- **Estado:** hecho
- **Commit:** `400e3b5` (`feat: ingestión incremental e idempotente`)
- **Resumen de `cargo test`:** 35 pruebas superadas (6 en `tests/ingest.rs`, 10 en `tests/collector_antigravity.rs`, 8 en `tests/collector_claude_code.rs`, 9 en `tests/db.rs`, 1 en `tests/smoke.rs`, 1 unitaria en `gestor_core`), 0 fallidas, 0 ignoradas; `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` en verde.

## Archivos creados

- `app/src-tauri/src/core/ingest.rs`
- `app/src-tauri/tests/ingest.rs`
- `app/src-tauri/examples/contar.rs`

*(Modificado: `app/src-tauri/src/core/mod.rs` para exportar `pub mod ingest;`.)*

## Decisiones

- **`CostHook` desacoplado para T6:** se definió el trait `CostHook` y `NoopCostHook` junto con `ingest_file_with_cost`, de modo que `ingest_file` invoca el gancho sin efecto hasta que T6 conecte `apply_cost`. Al insertar un evento nuevo en `usage_event`, `hourly_agg` lee `cost_usd` y `cost_kind` para sumar en `cost_real`, `cost_equiv` o `unpriced` automáticamente cuando T6 esté conectado.
- **Filtrado con `accepts_path` sin alterar el trait `Collector`:** `scan_all` recorre las raíces de cada colector usando `entry.file_type()` (sin seguir enlaces simbólicos) y filtra según `collector.id()` llamando a `ClaudeCodeCollector::accepts_path` (`"claude-code"`) y `AntigravityCollector::accepts_path` (`"antigravity"`), manteniendo intactas las firmas de T3 y T4.
- **Idempotencia, rotación y errores de línea:** cada archivo se procesa en su propia transacción (asegurando que una interrupción deje los archivos previos persistidos con su offset). Si `size < saved_size` se relee desde 0 y se limpian errores previos de ese `source_file_id`; los duplicados detectados por `INSERT OR IGNORE` no incrementan `hourly_agg`. Las fechas (`mtime`, `last_read`, `ts_utc`, `hour_utc`, `at`) se formatean exclusivamente con `core::time::fmt_utc`.
- **Ejemplo `examples/contar.rs`:** usa `tempfile::tempdir()` y `open_db` en una base temporal, lee en solo lectura las raíces por defecto de `ClaudeCodeCollector` y `AntigravityCollector`, e imprime únicamente contadores de ingesta y la tabla numérica de los últimos 7 días por herramienta y modelo (sin rutas ni contenido). No se ejecutó contra los logs reales.

## Dudas

- Ninguna.
