# Resultado T6: Precios versionados y coste por registro

- **Estado:** hecho
- **Commit:** `ad2db67` (`feat: precios versionados y coste por registro`)
- **Resumen de `cargo test`:** 56 pruebas ejecutadas y aprobadas (0 fallidas, 0 ignoradas: 12 `collector_antigravity`, 12 `collector_claude_code`, 10 `cost`, 9 `db`, 9 `ingest`, 1 `smoke`, 3 unitarias de `gestor_core`); `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` en verde.

## Archivos creados

- `app/src-tauri/resources/pricing.json`
- `app/src-tauri/src/core/pricing.rs`
- `app/src-tauri/src/core/cost.rs`
- `app/src-tauri/tests/cost.rs`

## Archivos modificados

- `app/src-tauri/src/core/Cargo.toml` (dependencia `globset = "0.4"`)
- `app/src-tauri/Cargo.lock`
- `app/src-tauri/src/core/mod.rs`
- `app/src-tauri/src/core/time.rs` (`hour_utc_str` compartido)
- `app/src-tauri/src/core/db/mod.rs` (`seed_prices` al inicializar la BD)
- `app/src-tauri/src/core/ingest.rs` (`ApplyCostHook` conectado al `CostHook`)
- `app/src-tauri/examples/contar.rs` (coste equivalente de los últimos 30 días)
- `app/src-tauri/tests/ingest.rs` (`agregado_horario_cuadra` actualizado para comprobar `unpriced = 0` y `cost_equiv = SUM(cost_usd)`)

## Decisiones

1. **Siembra automática de precios en `db::init_connection`:** se llama a `pricing::seed_prices` junto con `seed_plans` dentro de la inicialización de `open_db` y `open_memory`, manteniendo intactas las filas con `user_edited = 1`.
2. **`valid_to` exclusivo sincronizado:** tanto `seed_prices` como `upsert_price` recalculan `valid_to` por `model_pattern` ordenado por `valid_from ASC` (cada fila recibe como `valid_to` la fecha `valid_from` de la siguiente, y `NULL` la última).
3. **`sourceUrl` de `claude-sonnet-5-5*`:** `docs/06` §3.1 indica «DevelopersDigest»; se registró `https://developersdigest.org`.
4. **Precisión aritmética en `formula_cuatro_tipos`:** el comentario del plan (`2×5 + 415×25 + 22158×6.25 + 29587×0.5`) suma `163666.0`, que dividido entre `1e6` da exactamente `0.163666` (el plan escribía `0.1636710` por un desvío de `0.000005`); la prueba comprueba el valor exacto `0.163666` con tolerancia `1e-7`.
5. **Actualización de `hourly_agg` en `upsert_price`:** al recalcular eventos afectados dentro de la transacción, se ajustan también los acumulados `cost_real`, `cost_equiv` y `unpriced` de `hourly_agg` para mantener coherencia con `usage_event`.

## Dudas

- Ninguna bloqueante; solo la errata aritmética menor en el literal `0.1636710` del ejemplo de `formula_cuatro_tipos` del plan (`docs/superpowers/plans/2026-11-26-gestor-tokens.md`, línea 330), cuyo resultado exacto es `0.163666`.
