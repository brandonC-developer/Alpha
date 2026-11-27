# Resultado de la Tarea 2 (T2)

- **Estado:** hecho
- **Commit:** `6de180b` — `feat: modelo de datos y esquema SQLite`

## Pruebas y comprobaciones

- **`cargo test --manifest-path app/src-tauri/Cargo.toml`:**
  - `tests/db.rs`: `test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s`
  - `tests/smoke.rs`: `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`
- **`npm test --prefix app`:** `Test Files  1 passed (1) | Tests  1 passed (1)`

## Archivos creados y modificados

- **Creados:**
  - `app/src-tauri/src/core/model.rs`
  - `app/src-tauri/src/core/time.rs`
  - `app/src-tauri/src/core/error.rs`
  - `app/src-tauri/src/core/db/mod.rs`
  - `app/src-tauri/src/core/db/schema.sql`
  - `app/src-tauri/tests/db.rs`
- **Modificados:**
  - `app/src-tauri/src/core/Cargo.toml`
  - `app/src-tauri/src/core/mod.rs`
  - `app/src-tauri/Cargo.toml`
  - `app/src-tauri/Cargo.lock`

## Decisiones

1. **Proveedor de `google-ai-pro-claude`:** se sembró con `provider = 'anthropic'`, en línea con `docs/06-colectores-y-costes.md` §2 para los modelos `claude-*` ejecutados dentro de Antigravity.
2. **`FromStr` y `Display` en `Tool` y `Provider`:** `FromStr` devuelve `CoreError::Parse` ante valores desconocidos y `Display` reutiliza `as_str()`.
3. **`hours_between(a, b)`:** calcula `b.signed_duration_since(a).num_milliseconds() as f64 / 3_600_000.0`, manteniendo el signo.
4. **Dependencias de prueba:** se añadieron `tempfile`, `chrono` y `rusqlite` en `[dev-dependencies]` de `app/src-tauri/Cargo.toml` (todas listadas en `docs/03-arquitectura.md` §2) para compilar `tests/db.rs` dentro de la estructura del workspace de T1; `gestor_core` mantiene únicamente `rusqlite` (`bundled`), `chrono` (`serde`), `thiserror` y `serde` (`derive`).
5. **Creación de carpeta padre en `open_db`:** `open_db(path)` crea los directorios padre si no existen antes de abrir la conexión, activa `WAL` y `foreign_keys = ON`, aplica `schema.sql` cuando `user_version == 0` y siembra los 4 planes con `INSERT OR IGNORE`.

## Dudas

- Ninguna.
