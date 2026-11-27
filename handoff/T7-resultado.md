# Resultado T7: Consultas del Panel, Costes y Sesiones

- **Estado:** hecho
- **Commit:** `3268972` (`feat: consultas del panel, costes y sesiones`)
- **Resumen de `cargo test`:** 65 pruebas ejecutadas y aprobadas (0 fallidas, 1 ignorada `panel_rapido`: 12 `collector_antigravity`, 12 `collector_claude_code`, 10 `cost`, 9 `db`, 9 `ingest`, 9 `query`, 1 `smoke`, 3 unitarias de `gestor_core`); `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` en verde.

## Archivos creados

- `app/src-tauri/src/core/query.rs`
- `app/src-tauri/tests/common/mod.rs`
- `app/src-tauri/tests/query.rs`

## Archivos modificados

- `app/src-tauri/src/core/mod.rs` (`pub mod query;`)

## Decisiones y dudas

1. **Identificador `r#yield` en `CostsView`:** `yield` es palabra reservada en Rust, por lo que el campo se declara `#[serde(rename = "yield")] pub r#yield: Vec<PlanYield>`, preservando el nombre JSON exacto de `docs/04-comandos.md`.
2. **Rendimiento conjunto de Google AI Pro (`yield`):** según `docs/06-colectores-y-costes.md` §3.2, `google-ai-pro` y `google-ai-pro-claude` comparten una única cuota mensual (20 $), por lo que en `CostsView.yield` se consolidan bajo `planId: "google-ai-pro"` (`"Google AI Pro"`) sumando las cuotas y el equivalente API de ambos planes. Si `fee_usd == 0` o `equivalent_usd == 0`, `ratio` devuelve `None`.
3. **Campo `unpriced` en `CostsView`:** el comentario de prueba del plan T7 menciona `unpricedEvents`, pero el contrato de `docs/04-comandos.md` §2 fija `unpriced: { model: string; events: number }[]`; se implementó `unpriced` siguiendo el contrato exacto y `docs/mockups/Costes.html`.
4. **Serialización de fechas en `Filter`, `SessionRow` y `CallRow`:** se usa `DateTime<Utc>` en Rust y un serializador `serde` basado en `core::time::fmt_utc` para emitir siempre ISO 8601 con sufijo `Z` en JSON y SQLite.
5. **Proyectos nulos en `byProject`:** los eventos sin proyecto (`project IS NULL`, almacenados como `''` en `hourly_agg`) se agrupan bajo `project: ""`.
6. **Observación sobre `panel_rapido` (1 M de eventos, marcado `#[ignore]` para fase 7):** al ejecutarlo manualmente en perfil `debug` (`-O0`), `dashboard` bajó de 17 847 ms (3 consultas `LAG`/`COUNT(DISTINCT)` en SQLite) a 4 685 ms (lectura en una sola pasada ordenada por `ix_event_ts` con `FastMap`), aún por encima del objetivo de 300 ms en `debug`. El cuello de botella no está en `hourly_agg` (~milisegundos), sino en que `sessions`, `activeSecs` y `maxConcurrency` requieren recorrer las 1 M filas de `usage_event` porque `hourly_agg` no almacena `session_id` ni granularidad de minuto. Duda no bloqueante para fase 7 / revisión: si `panel_rapido` se validará compilado con optimizaciones (`--release`) o si conviene añadir en una futura tarea una tabla/resumen precalculado por sesión o minuto (p. ej. `session_agg` / `minute_agg`) para bajar también en `debug` de 300 ms.
