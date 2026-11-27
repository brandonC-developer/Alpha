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

- **Carpeta:** `%USERPROFILE%\.gemini\antigravity\brain\<conversación>\.system_generated\logs\`
- **Archivo:** `transcript.jsonl`
- **Es evento si:** `source == "MODEL"` y existe `input_tokens`.

| Campo del log | Campo de `UsageEvent` | Notas |
| --- | --- | --- |
| carpeta `<conversación>` + `step_index` | `dedup_key = "ag:<conversación>:<step_index>"` | |
| `model` | `model` | p. ej. `gemini-4-neon-xhigh`, `gemini-4-hydrogen-high` |
| `created_at` | `ts_utc` | |
| carpeta `<conversación>` | `session_id` | |
| — | `project = None` | El transcript no da la carpeta de trabajo de forma fiable |
| `input_tokens − cache_read_tokens` | `input` | Mínimo 0. **Confirmar en la fase 0** con 3 conversaciones distintas |
| `cache_read_tokens` | `cache_read` | |
| — | `cache_write = 0` | El log no lo da |
| `output_tokens` | `output` | |
| — | `reasoning = 0` | El log no lo da por separado |

### 1.3 opencode (DeepSeek)

**Pendiente de confirmar en la fase 0.** opencode no está instalado. Cuando se instale, la fase 0 anota en `handoff/fase-0-informe.md`: la carpeta de datos en Windows, si guarda las sesiones en archivos JSON o en SQLite, y un ejemplo de mensaje del asistente **sin contenido**. Con eso Claude completa esta sección antes de la fase 6. Se espera, según la documentación pública de opencode, un mensaje con `id`, `sessionID`, `modelID`, `providerID`, `time.created` y `tokens { input, output, reasoning, cache { read, write } }`.

## 2. Plan de cada evento

| `tool` | `provider` | `plan_id` | `cost_kind` |
| --- | --- | --- | --- |
| `claude-code` | `anthropic` | `claude-pro` | `api_equivalent` (o `unknown` sin precio) |
| `antigravity` | `google` | `google-ai-pro` | `api_equivalent` (o `unknown`) |
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
- **Cuota de Google AI Pro en dólares:** el usuario la confirma en la fase 0 (se usa para el rendimiento del plan).
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

Resultado esperado: 3 eventos (pasos 1, 8 y 9); el paso 8 con `input` 11873; el 9 con `input` 0.

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
