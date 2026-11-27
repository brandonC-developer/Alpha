# Informe de la fase 0

Fecha: 2026-11-27 · Ejecutado por Claude desde la consola: lecturas solo de números y conversaciones `agy -p` de prueba con `gemini-4-hydrogen-low` · Sin código de la app.

## Versiones

rustc 1.100.0 · cargo 1.100.0 · node v24.22.1 · npm 11.19.1 · git 2.45.0.windows.1 · sqlite3 3.53.4 · opencode 1.19.6 · agy 1.5.18 · Windows 11 26H2 (compilación 26300.9974).

## Preparación del usuario

| Paso | Resultado |
| --- | --- |
| DeepSeek recargado y consulta de prueba | Hecho. Hay una llamada `deepseek-flash` registrada en opencode a las 09:30 |
| Punto de restauración | Hecho, según el usuario (Claude no puede leerlo sin permisos de administrador) |
| Copia de seguridad | Hecha: remoto `origin` en GitHub (`brandonC-developer/Alpha`). Hay que subir los commits nuevos con `git push` |

## Comprobaciones

| Casilla | Resultado | Evidencia |
| --- | --- | --- |
| Lo que carga una sesión `agy -p` | `AGENTS.md`, las reglas `always_on` (`privacidad.md`, `seguridad.md`), las 3 skills del proyecto y los 5 subagentes | Las reglas `glob` (`pruebas`, `rust`, `web`) se cargan al tocar `app/`, que aún no existe. También se ven unas 30 skills de plugins globales del usuario (Cloudflare, swarm y otras) |
| S1, permisos de partida | 0 entradas de AppContainer y 0 perfiles `exebox` | Perfil, `AppData\Local`, `.claude`, `.gemini`, `E:\Proyectos` y `Alpha` |
| S2, sandbox (`agy --sandbox`) | `cargo --version` y `npm --version` funcionan. Escribir fuera del proyecto: bloqueado. **Escribir dentro del proyecto también: «Access is denied»** (`handoff\dentro-sandbox.txt` y `cargo new`) | En Windows, el sandbox deja en solo lectura las carpetas fuera del perfil, y el proyecto está en E:. **No sirve para programar → Plan B** |
| S3, permisos huérfanos tras el sandbox | 0 en todas las carpetas, 0 perfiles `exebox` y 0 carpetas `Packages\exebox*` | El bug #1323 no se reprodujo |
| S4, reglas `deny` en un subagente | El `verificador` intentó `git clean -n` y quedó bloqueado: «permission check failed … Permission denied» | |
| S5, opencode en el repositorio real | Bloqueó la escritura fuera del proyecto, el borrado recursivo y `npm install`. Solo ve las 3 skills | Prueba del 27/11, en `09-seguridad.md` §3.2 |
| Modelo real de cada nivel de subagente | En los transcripts aparecen `gemini-4-hydrogen-tiered-high` y `gemini-4-hydrogen-tiered-xhigh` | Coincide con los issues #1242 y #1846: `flash` = High y `pro` = XHigh |
| Cuota de Claude en Antigravity | **Grupo propio.** `agy -p "/usage"` devuelve «Gemini Models» y «Claude and GPT models», cada uno con su límite semanal y su límite de 5 h | Hoy: Gemini 73 % semanal y 98 % en 5 h; Claude/GPT 50 % semanal y 100 % en 5 h. Consultarlo gasta 0 tokens |
| Caché de Antigravity dentro de la entrada | **No: `input_tokens` excluye la caché** | En 1.632 de 1.744 pasos con caché, `input_tokens < cache_read_tokens`, en 19 modelos de la app y la CLI. La documentación suponía lo contrario |
| Archivos de Antigravity | `transcript.jsonl` y `transcript_full.jsonl` tienen el mismo uso en las 63 conversaciones. Hay dos raíces: `antigravity\brain` (app, 21) y `antigravity-cli\brain` (CLI, 42) | Leer los dos archivos contaría todo dos veces |
| Claude Code, deduplicación | Archivo 1: 69 líneas `assistant`, 20 `message.id` únicos, 16 repetidos (hasta 10 veces). Archivo 2: 349, 127 y 101 | Confirma `docs/06` §1.1 |
| opencode, base | `opencode db path` = `C:\Users\crbra\.local\share\opencode\opencode.db`, modo WAL. Existen `message`/`part` y `session_message`, pero esta está vacía | |
| opencode, tokens | `total = input + output + reasoning + cache.read` en 6 de 6 mensajes. Las partes `step-finish` repiten los tokens del mensaje (6 de 6) | Ids de modelo: `deepseek-flash` y `deepseek-v4.1-flash-free`. `cost` de la llamada de pago: 0,00132 $ |
| opencode, `-wal`/`-shm` | **`sqlite3 -readonly` los crea**: aparecieron a las 09:31:31 sin opencode abierto. Con `?mode=ro&immutable=1` no se crea nada | Probado en una copia. Decisión en `06` §1.3: la app lee una copia |
| opencode, configuración | `implementador`: `opencode/deepseek-v4.1-flash-free`, variante `low`. Skills permitidas: las 3 | `opencode debug agent implementador` |
| opencode, gratuitos | `deepseek-v4.1-flash-free` sigue en la lista. Novedad: `beam-free` | `opencode models opencode` |
| Permisos de `agy` sin pantalla | Con `--mode accept-edits`, lo que no está en `allow` se deniega y la conversación se para. Las reglas exigen coincidencia exacta, así que se usa `regex:`. `--mode auto` deja pasar comandos «seguros» | `09-seguridad.md` §3.1 |

## Lo que debe hacer el usuario

- Resuelto el 27/11/2026: copia en GitHub; Google AI Pro cuesta 20 $/mes; DeepSeek está recargado con 5 $ y el saldo recargado no caduca (FAQ oficial). En opencode constan 5 llamadas de pago, que cuestan 0,0054 $ según opencode.

## Decisiones tomadas sin estar en el plan

- **Nuevo plan `google-ai-pro-claude`** para los modelos Claude dentro de Antigravity (cuota propia). Ya está en la spec, el plan (T2), `03`, `04` y `06`.
- **El colector de Antigravity lee dos raíces**, solo `transcript.jsonl`, y **no resta la caché** (T3).
- **El colector de opencode lee una copia de la base** (T14).
- **Sandbox de Antigravity:** no se usa con el proyecto en E:. Las tareas se lanzan sin pantalla, con lista de permisos.

## Preguntas abiertas para Claude

- ¿La app debería calibrar Google AI Pro sola con `agy -p "/usage"`, que no gasta tokens? Supone ejecutar `agy` desde la app. Se decide antes de T8.
- La pantalla Planes tiene que mostrar los dos grupos de Google AI Pro. Hay que actualizar los mockups antes de T13.
