# Informe de la fase 1

Fecha: 2026-11-27 · Lanzado por Claude desde la consola, una conversación por tarea · Modelos: `gemini-4-hydrogen-high` (T1, T2 y la corrección), `gemini-4-hydrogen-medium` (verificador), `claude-sonnet-5-5-high` (revisor) · Reserva DeepSeek: no se usó

## Puerta

| Casilla | Resultado | Evidencia |
| --- | --- | --- |
| `cargo test` y `npm test` en verde | Cumplida | 11 pruebas de Rust (`db.rs` 9, `smoke.rs` 1, otra 1) y 1 de Vitest. Las volvió a ejecutar Claude tras la corrección |
| `cargo clippy -- -D warnings` sin avisos | Cumplida | `--all-targets`, código de salida 0; `cargo fmt --check` correcto |
| `cargo tree -p gestor_core -e normal` sin `tauri` | Cumplida | Solo `chrono`, `rusqlite`, `serde`, `thiserror` y sus dependencias. La ruta contiene `src-tauri`, pero no es una dependencia |
| `npm run tauri dev` abre la ventana «Gestor de Tokens» | Cumplida | Confirmado por el usuario el 27/11/2026 |
| 2 commits con los mensajes del plan; informe | Cumplida | `5f973b4`, `6de180b` y la corrección `50c1494`; este informe |

## Rutas tocadas

Solo `app/`, `handoff/` y `.gitignore` en la raíz, que T1 tenía que crear. No cambió ningún archivo de configuración ni de documentación.

## Comandos bloqueados o que pidieron aprobación

- Verificador: `npm --prefix app test`. Es una forma válida de escribirlo, así que se añadió a `.agents/agy-permisos.json`.
- Revisor: tres comandos de git encadenados con `;` (denegado a propósito), y después `rg`. La conversación se retomó con `--conversation` para que terminara sin usarlos.

## Commits

- `50c1494` fix: apertura atómica de la BD, fechas con Z y busy_timeout
- `6de180b` feat: modelo de datos y esquema SQLite
- `5f973b4` chore: proyecto Tauri 2 con React y prueba de humo

## Revisión (Claude Sonnet 5.5 High, en Antigravity)

Sin hallazgos graves.

| Hallazgo | Qué se hizo |
| --- | --- |
| Medio: esquema y siembra fuera de la misma transacción | Corregido en `50c1494`: los PRAGMA van antes y el esquema, la siembra y `user_version` en una sola transacción. Tiene prueba |
| Medio: `to_rfc3339()` escribe `+00:00` | Corregido: `core::time::fmt_utc` con `Z`, usado en todo el código. Tiene prueba |
| Medio: TypeScript 6 en vez de 5 | Aceptado: es lo que instala la plantilla y la versión estable. El plan se actualizó |
| Menor: sin `busy_timeout` | Corregido: 5000 ms, con prueba |
| Menor: `user_version` mayor que 1 sin aviso | Deuda anotada para la primera migración |
| Menor: `plan_for(Opencode, Other)` devuelve `deepseek` | Intencionado: los modelos gratuitos cuentan 0 $ reales en ese plan (prueba de T14) |
| Menor: dependencias duplicadas, restos de la plantilla (`App.css`, svg, `authors`), `bundle.targets: "all"` | Se limpia en T11 (interfaz) y T16 (instalador NSIS) |
| Menor: faltan pruebas de los `CHECK` y de las claves externas | Se añaden con T5, cuando haya inserciones reales |

## Casillas que debe comprobar el usuario

- `cd "E:\Proyectos\Gestior de Tokens\Alpha\app"` y luego `npm run tauri dev`: tiene que abrirse la ventana «Gestor de Tokens».

## Decisiones tomadas sin estar en el plan

- `gestor_core` es un crate aparte del workspace (`03-arquitectura.md` §1).
- Se quitó `plugin-opener` de la plantilla y el usuario sincronizó `package-lock.json`.

## Preguntas abiertas para Claude

Ninguna.
