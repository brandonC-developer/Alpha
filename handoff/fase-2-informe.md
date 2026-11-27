# Informe de la fase 2

Fecha: 2026-11-27 · Lanzado por Claude desde la consola, una conversación por tarea y en orden (T3 → T4 → T5; T4 usa el `Collector` de T3) · Modelos: `gemini-4-hydrogen-high` (T3, T4, T5 y la corrección), `gemini-4-hydrogen-medium` (verificador), `claude-sonnet-5-5-high` (revisor) · Reserva DeepSeek: no se usó

## Puerta

| Casilla | Resultado | Evidencia |
| --- | --- | --- |
| `cargo test` en verde, con los Puntos de atención 1, 2 y 5 | Cumplida | 46 pruebas tras la corrección. P1: `mismo_mensaje_en_varias_lineas_cuenta_una_vez`. P2: `linea_incompleta_no_avanza_offset` y `reingestar_no_duplica`. P5: `importacion_interrumpida_continua` |
| Ningún struct de los colectores tiene campos de contenido | Cumplida | Lo confirman el revisor y el verificador. Claude Code solo declara `type`, `timestamp`, `sessionId`, `cwd`, `requestId` y `message{id, model, usage}`. Antigravity solo declara 7 campos numéricos o de metadatos |
| Datos reales frente a ccusage (±1 %) | **Cumplida para Claude Code** | Ver la tabla de abajo |

## Datos reales frente a ccusage

`cargo run --example contar` sobre los logs del usuario: 213 archivos, 0 fallidos, 0 errores de línea, 13.383 eventos, unos 20 s.

ccusage (`npx ccusage@latest daily --since 20261120 --timezone UTC --json --by-agent --no-cost`) frente a la app, con el mismo día UTC, programa temporal fuera del repositorio:

| Herramienta | Entrada | Escritura caché | Lectura caché | Salida | Total |
| --- | --- | --- | --- | --- | --- |
| Claude Code | +0,19 % | +0,04 % | +0,29 % | +0,12 % | **+0,28 %** |
| Antigravity | +0,12 % | — | −0,87 % | +80 % | −0,37 % |

- **Claude Code:** los 4 días cerrados coinciden exactamente (0,00 %). La pequeña diferencia de hoy se debe a que esta conversación escribía en los logs entre una lectura y otra.
- **Antigravity, salida:** la app suma exactamente el campo `output_tokens` del transcript. Se comprobó a mano: el 23/11 da 219.528 en el transcript y 219.528 en la app. ccusage lee otro archivo (`conversations/<id>.db`, protobuf con `candidatesTokenCount` y `thoughtsTokenCount` separados) y cuenta sin el razonamiento. La app lo incluye, que es su convención y lo que cobra Google. Documentado en `06` §1.2.
- **Antigravity, entrada por día:** coincide exactamente en 5 de 8 días. Los días 21 y 24 reparten distinto entre ellos (−5,6 % y +4,8 %, que se compensan), seguramente por usar otra marca de tiempo. El total cuadra con un 0,12 %.
- **opencode:** la app aún no lo lee (llega en T14). ccusage sí, y servirá para comparar en la fase 6.

## Rutas tocadas

Solo `app/` y `handoff/`. No cambió ningún archivo de configuración ni de documentación.

## Comandos bloqueados

- Revisor: `git grep`. Se retomó la misma conversación para que terminara leyendo los archivos con su herramienta.

## Commits

- `9bfa694` fix: revisión de la fase 2 (errores sin contenido, escaneo tolerante, rotación)
- `400e3b5` feat: ingestión incremental e idempotente
- `84c1b1a` y `a79b38a` feat: colector de Antigravity. Son dos porque `app/.gitignore` ignoraba las carpetas `logs` y dejó fuera los fixtures; se corrigió con `/logs`
- `65fee27` feat: colector de Claude Code

## Revisión (Claude Sonnet 5.5 High)

Sin hallazgos graves. Se corrigieron en `9bfa694`:

| Hallazgo | Qué se hizo |
| --- | --- |
| Medio: el texto del error de serde podía guardar contenido del log en `ingest_error` | Ahora solo se guarda un motivo fijo, la línea y la columna. Hay una prueba con `secreto-del-prompt` |
| Medio: un archivo que falla detenía todo el escaneo | Se cuenta en `files_failed` y el escaneo sigue |
| Medio: la rotación solo se detectaba si el archivo encogía | También se relee si el offset no cae en un salto de línea, y si el offset supera el tamaño |
| Menor: se releía el archivo hasta el offset para numerar líneas | Las líneas solo se numeran cuando hay un error |
| Menor: `Utc::now()` dentro de `ingest_file` | Ahora recibe `now` (`AGENTS.md` §4) |
| Menor: ruta relativa cuando no hay directorio de usuario | Devuelve una lista vacía de raíces |
| Menor: código sin aserciones y casos sin probar | Se quitó, y se añadieron pruebas de archivo vacío, CRLF y BOM |

Queda como política, sin cambio de código:
- Las líneas de asistente sin `model` ni marca de tiempo se saltan sin error.
- Una última línea sin salto de línea espera a que el archivo cambie, como dice el plan.

## Casillas que debe comprobar el usuario

Ninguna.

## Preguntas abiertas para Claude

Ninguna.
