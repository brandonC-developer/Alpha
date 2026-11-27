# Resultado Tarea 3: Colector de Claude Code

- **Estado:** hecho
- **Commit:** `65fee27` (`feat: colector de Claude Code`)
- **Resumen de `cargo test`:** 19 pruebas superadas (8 en `tests/collector_claude_code.rs`, 9 en `tests/db.rs`, 1 en `tests/smoke.rs`, 1 unitaria en `gestor_core`); `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` en verde.

## Archivos creados

- `app/src-tauri/src/core/collectors/mod.rs`
- `app/src-tauri/src/core/collectors/claude_code.rs`
- `app/src-tauri/tests/collector_claude_code.rs`
- `app/src-tauri/tests/fixtures/claude_code/normal.jsonl`
- `app/src-tauri/tests/fixtures/claude_code/bloques.jsonl`
- `app/src-tauri/tests/fixtures/claude_code/incompleta.jsonl`
- `app/src-tauri/tests/fixtures/claude_code/corrupta.jsonl`

*(Modificados: `app/src-tauri/src/core/Cargo.toml`, `app/src-tauri/src/core/mod.rs` y `app/src-tauri/Cargo.lock` para añadir `serde_json` y `dirs` a `gestor_core`.)*

## Decisiones

- **`Collector` trait y filtrado de archivos:** se mantiene `trait Collector` exactamente con los 3 métodos de la spec §8.1 (`id`, `roots`, `parse_file`) más `ParseOutcome` (con `errors: Vec<LineError>`) y `LineError`. El filtrado por extensión `*.jsonl` se expone en `ClaudeCodeCollector::accepts_path(path: &Path) -> bool` para que la ingestión (T5) pueda aplicarlo sin impedir que `parse_file` acepte archivos temporales en pruebas.
- **Numeración de líneas desde `from_offset > 0`:** cuando `0 < from_offset < file_len`, se cuentan los `\n` hasta `from_offset` antes de empezar a analizar líneas, de modo que `LineError.line` corresponde siempre al número de línea real del archivo (1-based). Si `from_offset >= file_len`, `parse_file` retorna inmediatamente con `new_offset = file_len`.
- **Campos opcionales en `RawLine`:** `serde` deserializa solo los campos de `docs/06` §1.1 como `Option<...>` (sin `deny_unknown_fields` ni campos de contenido). Las líneas sin `type == "assistant"`, sin `message.usage`, sin campos obligatorios o con `model == "<synthetic>"` se omiten sin error; solo fallan como `LineError` las líneas con UTF-8 inválido, JSON corrupto o `timestamp` inválido en un evento de asistente.
- **Extracción de `project` desde `cwd`:** `project_from_cwd` recorta separadores finales (`\` y `/`) y toma el último segmento no vacío (descartando unidades raíz como `E:\`), funcionando de forma homogénea con rutas Windows y POSIX.

## Dudas

- Ninguna.
