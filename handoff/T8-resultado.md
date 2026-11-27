# Resultado T8: Planes, ventanas de uso y calibración

- **Estado:** hecho
- **Commit:** `3a0eed2` (`feat: planes, ventanas de uso y calibración`)
- **Resumen de `cargo test`:** 81 pruebas ejecutadas y aprobadas (0 fallidas, 1 ignorada `panel_rapido`: 6 `agy_usage`, 12 `collector_antigravity`, 12 `collector_claude_code`, 10 `cost`, 9 `db`, 9 `ingest`, 10 `plans`, 9 `query`, 1 `smoke`, 3 unitarias de `gestor_core`); `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` en verde.

## Archivos creados

- `app/src-tauri/src/core/plans.rs`
- `app/src-tauri/src/core/agy_usage.rs`
- `app/src-tauri/tests/fixtures/agy/usage.json`
- `app/src-tauri/tests/plans.rs`
- `app/src-tauri/tests/agy_usage.rs`

## Archivos modificados

- `app/src-tauri/src/core/mod.rs` (`pub mod agy_usage;` y `pub mod plans;`)

## Decisiones y dudas

1. **Rango de porcentaje en `calibrate` frente al comando Tauri:** `core::plans::calibrate` acepta `(0.0, 100.0]` para compartir la misma lógica con `agy_usage` (donde `(1 - remaining_fraction) * 100` puede quedar entre 0 y 1 % y el esquema SQLite exige `percent > 0 AND percent <= 100`); la validación `[1, 100]` específica de la interfaz queda para el comando `calibrate_plan` en T10 (`docs/04-comandos.md` §2).
2. **Media de calibraciones y mezcla `agy` / `user`:** cuando la última calibración de una ventana es `agy`, el límite estimado es la media de las 8 calibraciones `agy` más recientes (`ORDER BY at DESC, id DESC LIMIT 8`); si la última calibración es `user` (p. ej. `claude-pro` o calibración manual cuando no se usa `agy`), se promedian las calibraciones `user` de esa ventana.
3. **Reinicio oficial de `agy` (`< 20 min`):** en `window_status`, si existe una calibración `agy` con `0 <= now - at < 20 min` y `resets_at` no nulo, se devuelve ese `resets_at` oficial; en caso contrario se usa el fin del bloque de 5 h calculado por la app (`None` en la ventana semanal).
4. **Cálculo de `exhaustsAt`:** se usa el ritmo de los últimos 30 minutos (`recent_30m_used / 1800 s`) dentro de la ventana activa y solo se devuelve `Some(eta)` cuando existe límite calibrado, queda margen positivo y `eta < resets_at` (o sin cota si no hay `resets_at`).
5. **Ventana semanal y ventana de 5 h:** la ventana `5h` usa `current_block` sobre eventos cargados con un margen de 240 h para no cortar bloques encadenados; la ventana `weekly` suma los eventos en `(now - 168 h, now]`. Sin dudas bloqueantes.
