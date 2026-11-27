# Correcciones de la fase 1

- **Estado:** hecho (`fix: apertura atómica de la BD, fechas con Z y busy_timeout`)
- **cargo test:** `tests/db.rs` 9 passed, `tests/smoke.rs` 1 passed, `gestor_core` (`mod.rs`) 1 passed (11 passed; 0 failed; 0 ignored).
- **Archivos tocados:**
  - `app/src-tauri/src/core/db/mod.rs`
  - `app/src-tauri/src/core/db/schema.sql`
  - `app/src-tauri/src/core/time.rs`
  - `app/src-tauri/src/core/Cargo.toml`
  - `app/src-tauri/tests/db.rs`
  - `app/src-tauri/Cargo.lock`
