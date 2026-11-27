# Correcciones de la fase 3

- **Estado:** hecho (`fix: revisión de la fase 3, hallazgos graves (session_hour, bordes del rango, byProject)`, commit `5abe262`)
- **Tiempos de `panel_rapido` (`--release`):** 154 ms (primera ejecución; perfilado `total=121.67 ms`) y 143 ms (segunda ejecución; perfilado `total=187.92 ms`), con `hourly_agg=12960` y `session_hour=393120` (objetivo < 200 ms cumplido).
- **cargo test:** `agy_usage` 6 passed, `alerts` 8 passed, `collector_antigravity` 12 passed, `collector_claude_code` 12 passed, `cost` 10 passed, `db` 9 passed, `ingest` 10 passed, `plans` 10 passed, `query` 13 passed + 1 ignored (`panel_rapido`), `smoke` 1 passed, `gestor_core` 3 passed (94 passed; 0 failed; 1 ignored).
- **Archivos tocados:**
  - `app/src-tauri/src/core/ingest.rs`
  - `app/src-tauri/src/core/query.rs`
  - `app/src-tauri/tests/common/mod.rs`
  - `app/src-tauri/tests/ingest.rs`
  - `app/src-tauri/tests/query.rs`

## Hallazgos medios, menores y precios (n.º 4–10 + Claude)

- **Estado:** hecho (`fix: revisión de la fase 3, hallazgos medios y menores, precios de Haiku y Fable`, commit `5c55c36`)
- **Tiempo de `panel_rapido` (`--release`):** 153 ms (perfilado `total=138.23 ms`, `hourly_agg=12960`, `session_hour=393120`, objetivo < 200 ms cumplido).
- **cargo test:** `agy_usage` 6 passed, `alerts` 10 passed, `collector_antigravity` 12 passed, `collector_claude_code` 12 passed, `cost` 16 passed, `db` 9 passed, `ingest` 10 passed, `plans` 12 passed, `query` 13 passed + 1 ignored (`panel_rapido`), `smoke` 1 passed, `gestor_core` 4 passed (105 passed; 0 failed; 1 ignored); `clippy` y `fmt --check` en verde.
- **Archivos tocados:**
  - `app/src-tauri/resources/pricing.json`
  - `app/src-tauri/src/core/agy_usage.rs`
  - `app/src-tauri/src/core/alerts.rs`
  - `app/src-tauri/src/core/cost.rs`
  - `app/src-tauri/src/core/ingest.rs`
  - `app/src-tauri/src/core/plans.rs`
  - `app/src-tauri/src/core/pricing.rs`
  - `app/src-tauri/src/core/query.rs`
  - `app/src-tauri/tests/agy_usage.rs`
  - `app/src-tauri/tests/alerts.rs`
  - `app/src-tauri/tests/common/mod.rs`
  - `app/src-tauri/tests/cost.rs`
  - `app/src-tauri/tests/plans.rs`
