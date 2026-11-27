---
trigger: glob
description: Reglas para las pruebas del Gestor de Tokens.
globs: "app/src-tauri/tests/**/*, app/tests/**/*.ts, app/tests/**/*.tsx"
---

- Nombres de prueba en español, tal como los escribe el plan (`snake_case` en Rust, frase en Vitest).
- Cada prueba tiene al menos una aserción que falla si el comportamiento cambia.
- Sin red real: DeepSeek se prueba con `parse_balance` sobre JSON fijo; `fetch_balance` solo en pruebas `#[ignore]`.
- Base de datos de prueba con `open_memory()` o un `tempfile`; nunca `%APPDATA%`.
- Hora fija: `now = 2026-11-26T16:00:00Z`, `tz = UTC−6`, salvo que la prueba trate de la hora.
- Fixtures: solo los de `docs/06-colectores-y-costes.md` §4 y variantes sintéticas. **Nunca** copies líneas de los logs reales del usuario.
- No borres, ni marques `#[ignore]`/`skip`, una prueba existente para que la batería pase.
