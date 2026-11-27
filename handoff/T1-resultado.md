# Resultado de la Tarea 1 (T1)

- **Estado:** hecho (pendiente que el usuario ejecute `npx tsc --noEmit` dentro de `app/`, sincronice `package-lock.json` y compruebe `npm run tauri dev`).
- **Commit:** `5f973b4` — `chore: proyecto Tauri 2 con React y prueba de humo`

## Pruebas y comprobaciones

- **`cargo test --manifest-path app/src-tauri/Cargo.toml`:** `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` (en `tests/smoke.rs`; los binarios de `lib.rs`, `main.rs`, `gestor_core` y doc-tests corrieron con 0 tests).
- **`npm test --prefix app`:** `Test Files  1 passed (1) | Tests  1 passed (1)` (Vitest v5.1.2 con jsdom).
- **`cargo clippy --manifest-path app/src-tauri/Cargo.toml --all-targets -- -D warnings`:** correcto (código de salida 0).
- **`cargo fmt --manifest-path app/src-tauri/Cargo.toml --check`:** correcto (código de salida 0).
- **`cargo tree --manifest-path app/src-tauri/Cargo.toml -p gestor_core -e normal`:** `gestor_core v0.1.0` (sin `tauri`).
- **`npx tsc --noEmit -p app/tsconfig.json`:** falló al ejecutarse desde la raíz porque `npx` busca `tsc` en el directorio actual (`node_modules` está en `app/`) y la herramienta no permite `cd` ni cambiar el directorio de trabajo.

## Archivos creados / modificados en T1

- Raíz: `.gitignore`
- Configuración web y pruebas: `app/package.json` (eliminado `@tauri-apps/plugin-opener`, añadido `"test": "vitest run"`), `app/vitest.config.ts`, `app/tsconfig.json`, `app/index.html`, `app/tests/web/setup.ts`, `app/tests/web/smoke.test.tsx`
- Interfaz esqueleto: `app/src/main.tsx`, `app/src/App.tsx`, `app/src/components/Nav.tsx`, `app/src/styles.css`
- Rust / Tauri: `app/src-tauri/Cargo.toml`, `app/src-tauri/Cargo.lock`, `app/src-tauri/tauri.conf.json`, `app/src-tauri/capabilities/default.json`, `app/src-tauri/src/lib.rs`, `app/src-tauri/src/app/mod.rs`, `app/src-tauri/src/core/Cargo.toml`, `app/src-tauri/src/core/mod.rs`, `app/src-tauri/tests/smoke.rs`

## Decisiones

1. **`gestor_core` como miembro de workspace Cargo (`app/src-tauri/src/core/Cargo.toml`):** se configuró `[workspace] members = [".", "src/core"]` y `default-members = [".", "src/core"]` en `app/src-tauri/Cargo.toml`, con `[lib] path = "mod.rs"` en `core/Cargo.toml` y `pub use gestor_core as core;` en `src/lib.rs`. Así se cumple que `cargo tree -p gestor_core -e normal` no contenga `tauri` (puerta 1), que `gestor_core::version()` funcione en `tests/smoke.rs` y que los archivos permanezcan en `app/src-tauri/src/core/`.
2. **Eliminación de `opener`:** se quitó `tauri-plugin-opener` y `@tauri-apps/plugin-opener` de `Cargo.toml`, `package.json`, `capabilities/default.json` y `src/lib.rs`.
3. **Dependencias Rust de T1:** se añadieron únicamente las de `docs/03-arquitectura.md` §2 usadas en T1 y sus capabilities (`tauri` con `tray-icon` e `image-png`, `tauri-plugin-single-instance`, `tauri-plugin-notification`, `tauri-plugin-autostart`, `tauri-plugin-dialog`, `serde` y `serde_json`).
4. **Esqueleto React mínimo:** `Nav.tsx` y `App.tsx` montan `<nav>` y las 6 rutas de `docs/05-ui.md` §2 con las variables CSS base de §1, sin adelantar componentes ni lógica de T11–T13.

## Dudas / notas

- Ninguna duda bloqueante sobre la arquitectura. Queda pendiente que el usuario ejecute `npx tsc --noEmit` y `npm install` (para actualizar `package-lock.json` sin `@tauri-apps/plugin-opener`) dentro de `app/`, y compruebe la ventana con `npm run tauri dev`.
