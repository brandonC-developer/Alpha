---
trigger: glob
description: Reglas para el código Rust del Gestor de Tokens.
globs: "app/src-tauri/src/**/*.rs, app/src-tauri/examples/**/*.rs"
---

- Rust estable, edición 2021. Sin `unsafe`. Sin `unwrap()` ni `expect()` fuera de pruebas y de `main.rs`: errores con `CoreError` (`thiserror`) y `?`.
- `core/` no importa `tauri`. La hora entra como `now: DateTime<Utc>`; nunca `Utc::now()` dentro de `core/` salvo en funciones de borde marcadas `_now`.
- SQLite: siempre `params![]` o `?1`; nunca `format!` con valores. Escrituras de varias filas en `conn.transaction()`.
- Archivos de las herramientas: solo `File::open`. Nunca `OpenOptions` con escritura, `rename` ni `remove` sobre ellos.
- Structs de `serde` para logs: solo los campos de `docs/06-colectores-y-costes.md` §1, con `#[serde(default)]` en los opcionales.
- `tracing`: nunca registres contenido de logs, la clave de DeepSeek ni rutas completas de `%USERPROFILE%` (usa rutas relativas).
- Tipos que cruzan a la interfaz: `#[derive(Serialize)]` con `#[serde(rename_all = "camelCase")]`, idénticos a `docs/04-comandos.md`.
