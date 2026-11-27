# Corrección de rendimiento de la fase 3 (`session_hour` y `< 200 ms`)

- **Estado:** completado (`bd8b6a5` y `82da3ec`, `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` en verde).
- **Tiempos de `panel_rapido` en release (3 ejecuciones):** `147 ms`, `153 ms`, `146 ms` (todas < 200 ms).
- **Filas en `panel_rapido`:** `hourly_agg = 12960`, `session_hour = 130560`.
- **Desglose por parte (`dashboard_profiled`, segunda llamada en caliente):**
  - Antes de la segunda pasada de optimización (`265 ms` en frío / `239.81 ms` en caliente): `hourly_agg=43.49 ms`, `prev_totals=0.06 ms`, `session_sql=186.91 ms`, `session_calc=4.52 ms`, `prev_active=0.09 ms`, `aggregate=3.54 ms`.
  - Tras optimizar (`147 / 153 / 146 ms` en frío):
    - Ejecución 1 (`128.41 ms`): `hourly_agg=17.56 ms`, `prev_totals=0.03 ms`, `session_sql=101.34 ms`, `session_calc=8.57 ms`, `prev_active=0.04 ms`, `aggregate=0.08 ms`.
    - Ejecución 2 (`112.14 ms`): `hourly_agg=13.51 ms`, `prev_totals=0.05 ms`, `session_sql=91.22 ms`, `session_calc=6.55 ms`, `prev_active=0.03 ms`, `aggregate=0.07 ms`.
    - Ejecución 3 (`114.71 ms`): `hourly_agg=11.64 ms`, `prev_totals=0.01 ms`, `session_sql=93.06 ms`, `session_calc=9.16 ms`, `prev_active=0.03 ms`, `aggregate=0.07 ms`.
- **Resumen de `cargo test`:** 91 pruebas aprobadas, 0 fallidas, 1 ignorada `panel_rapido` (6 `agy_usage`, 8 `alerts`, 12 `collector_antigravity`, 12 `collector_claude_code`, 10 `cost`, 9 `db`, 10 `ingest`, 10 `plans`, 10 `query`, 1 `smoke`, 3 unitarias de `gestor_core`).
- **Archivos tocados:**
  - `app/src-tauri/src/core/db/schema.sql` (`bd8b6a5`)
  - `app/src-tauri/src/core/ingest.rs` (`bd8b6a5`)
  - `app/src-tauri/src/core/query.rs` (`bd8b6a5`, `82da3ec`)
  - `app/src-tauri/tests/common/mod.rs` (`bd8b6a5`)
  - `app/src-tauri/tests/db.rs` (`bd8b6a5`)
  - `app/src-tauri/tests/ingest.rs` (`bd8b6a5`)
  - `app/src-tauri/tests/query.rs` (`bd8b6a5`, `82da3ec`)
