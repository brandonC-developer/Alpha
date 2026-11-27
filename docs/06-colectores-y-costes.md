# Colectores, precios y costes

Contrato exacto de qué se lee de cada log, cómo se convierte en `UsageEvent`, con qué precios se calcula el coste, qué alertas hay y qué columnas se exportan.

## 1. Formatos de log

Regla común: **solo se deserializan los campos de estas tablas.** Los structs de `serde` no declaran `content`, `thinking`, `text`, `display` ni nada más; `serde` ignora el resto. Una línea que no tiene los campos obligatorios no es un error: no es un evento de uso y se salta. Un error es solo una línea que no es JSON válido.

### 1.1 Claude Code

- **Carpeta:** `%USERPROFILE%\.claude\projects\<proyecto codificado>\`
- **Archivos:** `*.jsonl` (una sesión por archivo; las subcarpetas de subagentes también cuentan).
- **Es evento si:** `type == "assistant"` y existe `message.usage`.

| Campo del log | Campo de `UsageEvent` | Notas |
| --- | --- | --- |
| `message.id` + `requestId` | `dedup_key = "cc:<id>:<requestId>"` | Si falta `requestId`, `"cc:<id>:-"`. Claude Code repite la misma línea de uso para cada bloque de contenido del mensaje: se cuenta una vez |
| `message.model` | `model` | p. ej. `claude-opus-5-5` |
| `timestamp` | `ts_utc` | ISO 8601 |
| `sessionId` | `session_id` | |
| `cwd` | `project` | Solo el último componente de la ruta (`E:\…\Alpha` → `Alpha`) |
| `message.usage.input_tokens` | `input` | Ya excluye la caché |
| `message.usage.cache_creation_input_tokens` | `cache_write` | 0 si falta |
| `message.usage.cache_read_input_tokens` | `cache_read` | 0 si falta |
| `message.usage.output_tokens` | `output` | Incluye razonamiento |
| `message.usage.output_tokens_details.thinking_tokens` | `reasoning` | 0 si falta |
| — | `tool = claude-code`, `provider = provider_for(…)` | Un `model` `"<synthetic>"` se ignora (no es una llamada real) |

### 1.2 Antigravity

Medido en la fase 0 (27/11/2026) en 63 conversaciones: 21 de la app y 42 de la CLI `agy`.

- **Carpetas** (se leen las dos):
  - `%USERPROFILE%\.gemini\antigravity\brain\<conversación>\.system_generated\logs\`: la app de Antigravity;
  - `%USERPROFILE%\.gemini\antigravity-cli\brain\<conversación>\.system_generated\logs\`: la CLI `agy`, incluidas las tareas que lanza Claude (`02-agentes-y-modelos.md` §4.3).
- **Archivo:** solo `transcript.jsonl`. Junto a él está `transcript_full.jsonl`, con exactamente el mismo uso en las 63 conversaciones. **Leer los dos contaría todo dos veces.** Las copias de `logs\chunks\` tampoco se leen.
- **Es evento si:** `source == "MODEL"` y existe `input_tokens`.
- **Privacidad:** la línea trae también `thinking` (texto del razonamiento) y `tool_calls`. No se leen ni se guardan.

| Campo del log | Campo de `UsageEvent` | Notas |
| --- | --- | --- |
| carpeta `<conversación>` + `step_index` | `dedup_key = "ag:<conversación>:<step_index>"` | Los UUID no se repiten entre la app y la CLI |
| `model` | `model` | Ids vistos: `gemini-4-neon-*`, `gemini-4-hydrogen-*`, `gemini-4-hydrogen-tiered-high`, `gemini-4-hydrogen-tiered-xhigh` (subagentes `flash` y `pro`), `gemini-4-argon-medium`, `gemini-3.8-flash-medium`, `gemini-pro-orchestrator-neon`, `gemini-flash-worker-agy` y `claude-opus-5-5-high` |
| `created_at` | `ts_utc` | |
| carpeta `<conversación>` | `session_id` | |
| — | `project = None` | El transcript no da la carpeta de trabajo de forma fiable |
| `input_tokens` | `input` | **No incluye la caché.** Medido: en 15 de 18 pasos de una conversación de la app, `input_tokens < cache_read_tokens`, lo que es imposible si la incluyera. No se resta nada |
| `cache_read_tokens` | `cache_read` | |
| — | `cache_write = 0` | El log no lo da |
| `output_tokens` | `output` | |
| — | `reasoning = 0` | El log no da el número por separado |

- **Plan:** los modelos `claude-*` gastan la cuota «Claude and GPT models» de Google AI Pro y van al plan `google-ai-pro-claude` (§2). El resto, a `google-ai-pro`.
- **Cuota real sin gastar tokens:** `agy -p "/usage" --output-format json` devuelve, para cada grupo («Gemini Models» y «Claude and GPT models»), la fracción restante de la ventana semanal y de la de 5 h, con su hora de reinicio. Consume 0 tokens. Claude decidirá antes de T8 si la app lo usa para calibrar sola; eso supondría ejecutar `agy` y salir de la regla «solo `api.deepseek.com`».

### 1.3 opencode (DeepSeek)

Medido en la fase 0 (27/11/2026) con opencode 1.19.6.

- **Base:** `%USERPROFILE%\.local\share\opencode\opencode.db` (`opencode db path`), SQLite en modo WAL. Existen las tablas `message` y `part` (v1) y `session_message` (v2), pero `session_message` está vacía: se lee `message`.
- **Cómo la abre la app, sin escribir en la carpeta de opencode:** abrirla con `sqlite3 -readonly` **crea** `opencode.db-wal` y `opencode.db-shm` (medido: aparecieron a las 09:31:31 sin opencode abierto). Con `?mode=ro&immutable=1` no se crea nada, pero no se ve lo que está aún en el `-wal`. Por eso la app **copia `opencode.db` y, si existe, `opencode.db-wal`** a su propia carpeta temporal y lee la copia. Así no toca la carpeta de opencode y ve los datos más recientes.
- **Es evento si:** `json_extract(data,'$.role') = 'assistant'` y existe `$.time.completed`. Los mensajes abortados no tienen `time.completed` y llevan tokens 0.
- **Cursor:** `time_created` del último mensaje leído. Los mensajes se actualizan en sitio, así que solo cuentan cuando ya tienen `time.completed`.

| Campo de `message.data` | Campo de `UsageEvent` | Notas |
| --- | --- | --- |
| `message.id` | `dedup_key = "oc:<id>"` | |
| `providerID` | pista de proveedor | `deepseek` → DeepSeek; `opencode` → Other (gratuitos, 0 $ real) |
| `modelID` | `model` | Vistos: `deepseek-flash` (de pago) y `deepseek-v4.1-flash-free` |
| `time.created` (ms) | `ts_utc` | |
| `session_id` (columna) | `session_id` | |
| `path.cwd` | `project` | Solo el último componente de la ruta, como en Claude Code |
| `tokens.input` | `input` | No incluye la caché: `total = input + output + reasoning + cache.read` (medido en 6 mensajes) |
| `tokens.cache.read` / `tokens.cache.write` | `cache_read` / `cache_write` | |
| `tokens.output` + `tokens.reasoning` | `output` | Se suman: en opencode `output` no incluye el razonamiento, y en `UsageEvent` sí (igual que en Claude Code) |
| `tokens.reasoning` | `reasoning` | Parte de `output`; solo informativo |
| `cost` | — | Lo calcula opencode con precios de models.dev, sin la tarifa de hora punta. No se usa como gasto real: el coste lo calcula la app (§3) |

- **No se leen las partes `step-finish`:** repiten exactamente los tokens de su mensaje (6 de 6 coincidencias). Sumarlas contaría dos veces.
- **Nunca se lee la tabla `credential`** ni las columnas de texto de `part`.

### 1.4 Cuota de Google AI Pro con `agy` (calibración automática)

Decisión del usuario (27/11/2026), ADR-6. Medido en la fase 0: tarda unos 3 s, gasta 0 tokens y no crea ninguna conversación.

- **Comando exacto**, sin shell, con `CREATE_NO_WINDOW` y 30 s como máximo: `agy -p "/usage" --output-format json --print-timeout 20s`. Se ejecuta cada 15 min, al arrancar la app y con el botón «Actualizar» de Planes, solo si `agyAutoCalibrate` es `true` y `agy` está en el PATH.
- **Se lee solo** `command.data.groups[].buckets[]`: `id`, `window`, `remaining_fraction` y `reset_time`. Los textos (`description`, `name`) no se guardan, y la salida nunca se registra entera.

| `bucket.id` | Plan | Ventana |
| --- | --- | --- |
| `gemini-5h` | `google-ai-pro` | `5h` |
| `gemini-weekly` | `google-ai-pro` | `weekly` |
| `3p-5h` | `google-ai-pro-claude` | `5h` |
| `3p-weekly` | `google-ai-pro-claude` | `weekly` |

Si aparece otro `id`, se ignora y se cuenta como aviso en Fuentes.

- **Calibración:** `percent = (1 − remaining_fraction) × 100`. Se llama a `calibrate(plan, ventana, percent)` con `source = 'agy'` y `resets_at = reset_time`.
  - Si `remaining_fraction = 1` o la app no midió uso en esa ventana, no se calibra.
  - `reset_time = 1970-01-01T00:00:00Z` significa «sin ventana abierta» y se guarda como `NULL`.
- **Límite estimado:** la media de las 8 calibraciones `agy` más recientes de esa ventana, para que un dato raro no lo mueva. Las calibraciones a mano (`source = 'user'`) siguen valiendo, para Claude Pro y si `agy` no está.
- **Reinicio:** si la última calibración `agy` tiene menos de 20 min, `resetsAt` es el oficial (`resets_at`). Si no, el que calcula la app con su bloque de 5 h.
- **Limitación:** el % oficial incluye lo que el usuario gaste en la web o en la app móvil de Gemini. La app reparte ese gasto en el límite estimado, igual que con la calibración manual.

**Fixture `agy/usage.json`** (números inventados; los textos de `description` no se leen):

```json
{"conversation_id":"","status":"SUCCESS","response":"","duration_seconds":0,"num_turns":0,
 "usage":{"input_tokens":0,"output_tokens":0,"thinking_tokens":0,"cache_read_tokens":0,"total_tokens":0},
 "command":{"name":"usage","data":{"description":"secreto-del-prompt","groups":[
  {"name":"Gemini Models","description":"x","buckets":[
   {"id":"gemini-weekly","name":"Weekly Limit Remaining","description":"x","window":"weekly","remaining_fraction":0.75,"reset_time":"2026-12-03T14:25:59Z"},
   {"id":"gemini-5h","name":"Five Hour Limit Remaining","description":"x","window":"5h","remaining_fraction":0.6,"reset_time":"2026-11-27T20:02:00Z"}]},
  {"name":"Claude and GPT models","description":"x","buckets":[
   {"id":"3p-weekly","name":"Weekly Limit Remaining","description":"x","window":"weekly","remaining_fraction":0.5,"reset_time":"2026-12-03T22:40:28Z"},
   {"id":"3p-5h","name":"Five Hour Limit Remaining","window":"5h","remaining_fraction":1,"reset_time":"1970-01-01T00:00:00Z"}]}]}}}
```

## 2. Plan de cada evento

| `tool` | `provider` | `plan_id` | `cost_kind` |
| --- | --- | --- | --- |
| `claude-code` | `anthropic` | `claude-pro` | `api_equivalent` (o `unknown` sin precio) |
| `antigravity` | `google` | `google-ai-pro` | `api_equivalent` (o `unknown`) |
| `antigravity` con modelo `claude-*` | `anthropic` | `google-ai-pro-claude` | `api_equivalent` (precios de Anthropic) |
| `opencode` | `deepseek` | `deepseek` | `real` (o `unknown`) |
| cualquiera | `other` | según la herramienta | `unknown` |

## 3. Precios iniciales (`resources/pricing.json`)

Formato:

```json
[{ "provider": "anthropic", "modelPattern": "claude-opus-5-5*", "validFrom": "2026-01-01",
   "input": 4.0, "output": 20.0, "cacheWrite": 5.0, "cacheRead": 0.20,
   "sourceUrl": "https://platform.claude.com/docs/en/about-claude/pricing", "approximate": false }]
```

### 3.1 Valores ($ por millón de tokens, revisados el 26/11/2026)

| Patrón | Entrada | Salida | Escritura caché | Lectura caché | Aproximado | Fuente |
| --- | --- | --- | --- | --- | --- | --- |
| `claude-opus-5-5*` | 4,00 | 20,00 | 5,00 | 0,20 | no | Página de precios de Anthropic y varias fuentes coincidentes |
| `claude-sonnet-5-5*` | 2,00 | 10,00 | 2,50 | 0,10 | **sí** (una sola fuente; caché = 1,25× y 0,05× de la entrada, como Opus) | DevelopersDigest |
| `gemini-4-*` | — | — | — | — | — | **Sin precio público de API** para Neon, Hydrogen ni Carbon. Ver 3.2 |
| `deepseek-flash*`, `deepseek-v4-flash*` (alias retirado que DeepSeek sirve con el mismo modelo y precio) | 0,15 | 0,60 | 0 | 0,003 | **sí**: tarifa fuera de hora punta, vigente desde el 16/08/2026. En hora punta (01–04 y 06–10 UTC, de lunes a viernes) cuesta el doble. El gasto real exacto lo da el saldo | api-docs.deepseek.com/quick_start/pricing |
| `*-free`, `big-pickle` (proveedor `other`, herramienta `opencode`) | 0 | 0 | 0 | 0 | no: modelos gratuitos de opencode Zen. Coste 0 real, no «sin precio» | opencode.ai/docs/zen |

### 3.2 Pendiente antes de la fase 3 (lo hace Claude)

- **Gemini 4 (Neon, Hydrogen, Carbon):** Google solo publica precio de API para Gemini 4 Argon, que se retira. Antes de la fase 3, Claude revisa `ai.google.dev/gemini-api/docs/pricing`; si siguen sin precio, se usa el de Argon como aproximación para los tres (marcado `approximate: true`) y se documenta aquí. Mientras tanto, los eventos de Antigravity salen como «sin precio» (`unknown`), que es el comportamiento correcto.
- **Ids sin precio:** `gemini-pro-orchestrator-*`, `gemini-flash-worker-agy` y `gemini-3.8-flash-*` (vistos en la fase 0) no tienen precio de API: salen como «sin precio» (`unknown`). Los `gemini-4-hydrogen-tiered-*` los cubre el patrón `gemini-4-*`.
- **Cuota de Google AI Pro: 20 $/mes** (confirmada por el usuario el 27/11/2026). Se siembra en `google-ai-pro`. Es una sola cuota mensual para los planes `google-ai-pro` y `google-ai-pro-claude`; el rendimiento se calcula sumando los dos.
- **DeepSeek, saldo:** el saldo recargado no caduca; solo caduca el saldo regalado (`granted_balance`), según la FAQ oficial. DeepSeek no publica una recarga mínima oficial: las fuentes de terceros hablan de 2 $ a 5 $. El usuario recargó 5 $ (27/11/2026).
- **DeepSeek hora punta:** la v1 usa una sola tarifa por modelo. Si la diferencia entre el gasto calculado y la bajada de saldo supera el 10 % en un mes, se abre una tarea nueva para tarifas por franja horaria.

## 4. Fixtures exactos

Se copian literalmente en `app/src-tauri/tests/fixtures/`. Todos los textos son inventados; `"secreto-del-prompt"` existe para comprobar que no se guarda.

**`claude_code/normal.jsonl`** (2 eventos):

```jsonl
{"type":"user","sessionId":"s-001","cwd":"E:\\Proyectos\\Gestior de Tokens\\Alpha","timestamp":"2026-11-26T14:59:50.000Z","message":{"role":"user","content":"secreto-del-prompt"}}
{"type":"assistant","sessionId":"s-001","cwd":"E:\\Proyectos\\Gestior de Tokens\\Alpha","timestamp":"2026-11-26T15:00:00.000Z","requestId":"req_A","message":{"id":"msg_A","model":"claude-opus-5-5","role":"assistant","content":[{"type":"text","text":"secreto-del-prompt"}],"usage":{"input_tokens":2,"cache_creation_input_tokens":22158,"cache_read_input_tokens":29587,"output_tokens":415,"output_tokens_details":{"thinking_tokens":271}}}}
{"type":"assistant","sessionId":"s-001","cwd":"E:\\Proyectos\\Gestior de Tokens\\Alpha","timestamp":"2026-11-26T15:01:00.000Z","requestId":"req_B","message":{"id":"msg_B","model":"claude-opus-5-5","role":"assistant","content":[],"usage":{"input_tokens":10,"cache_creation_input_tokens":0,"cache_read_input_tokens":51745,"output_tokens":120}}}
```

**`claude_code/bloques.jsonl`**: tres veces la segunda línea de `normal.jsonl`, con `content` distinto (`thinking`, `text`, `tool_use`) y el mismo `message.id`, `requestId` y `usage` → 1 evento.

**`claude_code/incompleta.jsonl`**: la segunda línea de `normal.jsonl` con `\n`, seguida de la tercera **sin** `\n` final y cortada a la mitad.

**`claude_code/corrupta.jsonl`**: `normal.jsonl` con una línea `{"type":"assistant", esto no es json` entre la segunda y la tercera (línea 3).

**`antigravity/7f0c…/.system_generated/logs/transcript.jsonl`** (carpeta `7f0c2a10-0000-4000-8000-000000000001`):

```jsonl
{"step_index":0,"source":"USER_EXPLICIT","type":"USER_INPUT","status":"DONE","created_at":"2026-11-21T17:47:22Z","content":"secreto-del-prompt"}
{"step_index":1,"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE","created_at":"2026-11-21T17:47:22Z","model":"gemini-4-neon-xhigh","input_tokens":14166,"cache_read_tokens":0,"output_tokens":799,"thinking":"secreto-del-prompt"}
{"step_index":8,"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE","created_at":"2026-11-21T17:49:15Z","model":"gemini-4-neon-xhigh","input_tokens":26040,"cache_read_tokens":14167,"output_tokens":509}
{"step_index":8,"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE","created_at":"2026-11-21T17:49:15Z","model":"gemini-4-neon-xhigh","input_tokens":26040,"cache_read_tokens":14167,"output_tokens":509}
{"step_index":9,"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE","created_at":"2026-11-21T17:50:02Z","model":"gemini-4-hydrogen-high","input_tokens":100,"cache_read_tokens":150,"output_tokens":20}
```

Resultado esperado: 3 eventos (pasos 1, 8 y 9); el paso 8 con `input` 26040 y `cache_read` 14167; el 9 con `input` 100 y `cache_read` 150. Junto a él va una copia idéntica llamada `transcript_full.jsonl`, que no debe leerse.

## 5. Alertas

| Regla (`rule`) | `key` | Condición | Título | Cuerpo |
| --- | --- | --- | --- | --- |
| `window5h.<nivel>` | `<plan>:<inicio del bloque>` | `percent ≥ nivel` (80 y 95) | «Claude Pro: 80 % de la ventana de 5 h» | «Se reinicia a las 15:10. A este ritmo se agota a las 14:20.» (la segunda frase solo si hay `exhaustsAt`) |
| `weekly.80` | `<plan>:<semana ISO>` | `percent ≥ 80` | «Google AI Pro: 80 % del límite semanal» | «Quedan N días para que se renueve por completo.» |
| `budget.80` | `deepseek:<AAAA-MM>` | gasto real del mes ≥ 80 % del presupuesto | «DeepSeek: 4,05 $ de 5 $ este mes» | «Saldo: 0,95 $.» |
| `anomaly` | `<session_id>` | sesión anómala | «Sesión fuera de lo normal en Antigravity» | «41,2 M tokens, 12× tu media. Ábrela en Sesiones.» |

«Pausar avisos 1 h» (menú de la bandeja) guarda `alerts.paused_until` en `setting`; mientras dure, las alertas se registran pero no se notifican.

## 6. Exportación

CSV con separador `,`, UTF-8 con BOM (para Excel), fechas ISO 8601 UTC. Columnas, en este orden:

```
ChargePeriodStart,ServiceName,ResourceId,ConsumedQuantity,ConsumedUnit,BilledCost,BillingCurrency,x_CostKind,x_Tool,x_Model,x_Project,x_SessionId,x_InputTokens,x_CacheWriteTokens,x_CacheReadTokens,x_OutputTokens,x_ReasoningTokens,x_PriceId,x_ApiEquivalentCost
```

- `ServiceName` = proveedor; `ResourceId` = modelo; `ConsumedQuantity` = total de tokens; `ConsumedUnit` = `Tokens`; `BillingCurrency` = `USD`.
- `BilledCost`: coste solo si `x_CostKind = real`; 0 si es `api_equivalent`; vacío si `unknown`. El equivalente API va en `x_ApiEquivalentCost`.
- JSON: un objeto `{ "version": 1, "exportedAt": …, "events": [UsageEvent con coste…], "prices": [...] }`. `import_data` solo acepta `version: 1`.
