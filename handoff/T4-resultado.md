# Resultado Tarea 4: Colector de Antigravity

- **Estado:** hecho
- **Commit:** `a79b38a` y `84c1b1a` (`feat: colector de Antigravity`)
- **Resumen de `cargo test`:** 29 pruebas superadas (10 en `tests/collector_antigravity.rs`, 8 en `tests/collector_claude_code.rs`, 9 en `tests/db.rs`, 1 en `tests/smoke.rs`, 1 unitaria en `gestor_core`); `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` en verde.

## Archivos creados

- `app/src-tauri/src/core/collectors/antigravity.rs`
- `app/src-tauri/tests/collector_antigravity.rs`
- `app/src-tauri/tests/fixtures/antigravity/7f0c2a10-0000-4000-8000-000000000001/.system_generated/logs/transcript.jsonl`
- `app/src-tauri/tests/fixtures/antigravity/7f0c2a10-0000-4000-8000-000000000001/.system_generated/logs/transcript_full.jsonl`

*(Modificados: `app/src-tauri/src/core/collectors/mod.rs` para exportar `pub mod antigravity;` sin alterar `Collector`, `ParseOutcome` ni `LineError`; `app/.gitignore` cambiando `logs` por `/logs` para que Git no ignore las carpetas `.system_generated/logs/` de los fixtures.)*

## Decisiones

- **Validación de ruta en `accepts_path` y `parse_file`:** `AntigravityCollector::accepts_path` comprueba la estructura exacta `.../<conversación>/.system_generated/logs/transcript.jsonl` (sin distinguir mayúsculas en Windows), excluyendo automáticamente `transcript_full.jsonl` y cualquier archivo bajo `logs/chunks/`. `parse_file` también valida esta estructura para extraer `session_id = <conversación>` y devolver 0 eventos y 0 errores si se le pasa otro archivo (como `transcript_full.jsonl`).
- **Mapeo según lo medido en fase 0 (`docs/06` §1.2):** `input = input_tokens` sin restar `cache_read_tokens`, `cache_write = 0`, `reasoning = 0`, `project = None`, `dedup_key = "ag:<conversación>:<step_index>"`, deduplicando dentro del archivo con `HashSet`. Solo se procesan líneas con `source == "MODEL"` y `input_tokens` presente; el struct `RawLine` deserializa únicamente los 7 campos permitidos por §1.2.
- **Corrección de `app/.gitignore` (`logs` → `/logs`):** la plantilla inicial de `app/.gitignore` ignoraba cualquier carpeta llamada `logs` a cualquier profundidad, lo que dejó fuera los fixtures en el primer commit (`a79b38a`). Se ajustó `logs` a `/logs` y se añadieron los fixtures en `84c1b1a` con el mismo mensaje del plan (sin usar `--amend` para respetar la lista de comandos permitidos).

## Dudas

- Ninguna.
