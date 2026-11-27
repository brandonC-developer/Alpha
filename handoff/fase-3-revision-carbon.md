# Revisión de la fase 3: Gemini 4 Carbon High

Fecha: 2026-11-27 · Lanzada por Claude con `agy -p --model gemini-4-carbon-high`, en solo lectura, sobre `git diff 37b56e4..HEAD -- app/src-tauri/src/core/`

Revisión completada sobre `git diff 37b56e4..HEAD -- app/src-tauri/src/core/`. **Sí hay hallazgos graves (3 graves, 4 medios y 3 menores).**

**Verificado sin fallos:** el razonamiento (`reasoning`) no se cobra dos veces en [cost.rs:70-82](app/src-tauri/src/core/cost.rs#L70-L82); las tarifas y cambios de fecha del `2026-12-01` para Carbon/Argon (y Orchestrators) y del `2027-01-01` para 3.8 Flash coinciden con el contrato en [pricing.json:47-166](app/src-tauri/resources/pricing.json#L47-L166); las divisiones por cero en porcentajes, `deltaPct` y `ratio` de rendimiento están protegidas; y `byDay`, `heatmap` e inicios de mes/semana usan la zona horaria local `tz`.

---

## Hallazgos graves

### 1. [ingest.rs:444-464](app/src-tauri/src/core/ingest.rs#L444-L464) · [query.rs:1709-1728](app/src-tauri/src/core/query.rs#L1709-L1728) · [query.rs:1875-1881](app/src-tauri/src/core/query.rs#L1875-L1881) — `session_hour` calcula mal el tiempo activo cuando una sesión cambia de proveedor o de `project`
- **Problema:** `refresh_session_hours` particiona una misma pareja `(session_id, hour_utc)` en varios grupos por `(tool, provider, project)` y calcula `active_secs` por separado en cada subgrupo. Luego `query_session_metrics` y `query_total_active_secs` ordenan las filas de la sesión por `(first_ep, last_ep)`, suman todos sus `active_secs` y encadenan `curr.first_ep - prev.last_ep` solo contra `scratch[i - 1]`. Cuando una sesión alterna dos proveedores en la misma hora (caso real en Antigravity al usar modelos Gemini/`google` y Claude/`anthropic` en la misma conversación, o en opencode al pasar de `*-free`/`other` a `deepseek-flash`/`deepseek`) o dos `cwd` en Claude Code:
  1. Los intervalos `[first_ep, last_ep]` se solapan y los huecos se suman en paralelo (sobreconteo) o se truncan a 300 s donde los eventos reales distaban `< 300 s` (subconteo).
  2. Si un subgrupo queda anidado dentro de otro (`scratch[0] = 10:00..10:59`, `scratch[1] = 10:02..10:02`), al cruzar a `11:01` (`scratch[2]`) se resta `prev = scratch[1]` (`10:02`) en vez del mayor `last_ep` previo (`10:59`), inflando el hueco entre horas.
  Además, `dashboard()` discrepa de `sessions()` ([query.rs:802](app/src-tauri/src/core/query.rs#L802)), que sí calcula sobre `usage_event`.
- **Caso concreto que falla:** Conversación `conv-1` en Antigravity con eventos a `10:00Z` (`google`), `10:01Z` (`anthropic`), `10:02Z` (`google`), `10:03Z` (`anthropic`), `10:04Z` (`google`) y `10:05Z` (`anthropic`). El tiempo activo real según la spec §8.3 (y `sessions()`) es `5 × 60 = 300 s`, pero `session_hour` guarda `240 s` en `google` y `240 s` en `anthropic`, y `dashboard()` sin filtro de proveedor devuelve **`480 s`** (más que los 5 minutos de reloj de toda la conversación). A la inversa, con eventos cada 4 min (`10:00`, `10:04`, `10:08`, `10:12` alternando proveedor), el real es `720 s` y `dashboard()` devuelve **`600 s`**.
- **Regla incumplida:** [2026-11-26-gestor-tokens-design.md:37](docs/superpowers/specs/2026-11-26-gestor-tokens-design.md#L37) y [§8.3:162](docs/superpowers/specs/2026-11-26-gestor-tokens-design.md#L162) (*«Tiempo activo = suma de los huecos entre eventos consecutivos de una sesión, contando como máximo 5 minutos por hueco»*); [03-arquitectura.md:222-225](docs/03-arquitectura.md#L222-L225); [2026-11-26-gestor-tokens.md:90](docs/superpowers/plans/2026-11-26-gestor-tokens.md#L90).

### 2. [query.rs:606](app/src-tauri/src/core/query.rs#L606) · [query.rs:682-706](app/src-tauri/src/core/query.rs#L682-L706) · [query.rs:2088-2105](app/src-tauri/src/core/query.rs#L2088-L2105) — `costs().by_project` cuenta las sesiones `unknown` (`NULL`) como `0 $` en el total y en `avg_per_session`
- **Problema:** A diferencia de `by_tool` ([query.rs:673](app/src-tauri/src/core/query.rs#L673)), `by_project` no filtra proyectos sin turnos con precio (`priced_turns == 0`) y `query_sessions_by_project` cuenta `COUNT(DISTINCT session_id)` sobre todas las filas de `usage_event`, incluidas las sesiones con `cost_usd IS NULL` (`cost_kind = 'unknown'`). Luego calcula `avg_per_session = usd / sessions`, dividiendo entre sesiones sin precio (tratándolas como `0 $`) y devolviendo `usd: 0.0, avg_per_session: 0.0` cuando todas las sesiones de un proyecto carecen de precio.
- **Caso concreto que falla:** En el proyecto `"Alpha"` hay 1 sesión con `claude-opus-5-5` de `10,00 $` y 9 sesiones solo con `gemini-flash-worker-agy` (`cost_usd = NULL`, `unknown`). `costs()` devuelve `ProjectCost { project: "Alpha", usd: 10.0, sessions: 10, avg_per_session: 1.0 }` en lugar de `10,00 $` sobre las sesiones con precio; y si las 10 sesiones son `unknown`, devuelve `usd: 0.0, avg_per_session: 0.0` (indistinguible de un coste `0,00 $` real de modelos gratuitos).
- **Regla incumplida:** [AGENTS.md:54](AGENTS.md#L54) (*«Un coste sin precio es `NULL` y `unknown`, nunca 0»*); [2026-11-26-gestor-tokens-design.md:214](docs/superpowers/specs/2026-11-26-gestor-tokens-design.md#L214); [2026-11-26-gestor-tokens.md:52](docs/superpowers/plans/2026-11-26-gestor-tokens.md#L52).

### 3. [query.rs:1033-1038](app/src-tauri/src/core/query.rs#L1033-L1038) · [query.rs:1147-1178](app/src-tauri/src/core/query.rs#L1147-L1178) · [query.rs:2153-2169](app/src-tauri/src/core/query.rs#L2153-L2169) — Comparar `hour_utc` (`:00:00Z`) contra `from`/`to` con minutos/segundos excluye eventos dentro del rango e incluye eventos fuera de él
- **Problema:** `Filter.from` y `Filter.to` son instantes arbitrarios `[from, to)` (p. ej. `from = now - 90 d`, `to = now` según el plan T11, o `to = now` en `compute_month_metrics`). `build_hourly_where` ejecuta `hour_utc >= fmt_utc(f.from) AND hour_utc < fmt_utc(f.to)` sobre claves truncadas a `'YYYY-MM-DDTHH:00:00Z'`, y `table_covers_full_range` evalúa `empty_range = from_str > max_h` y `full_range = from_str <= min_h && max_h < to_str`:
  1. En el borde `from`, si `f.from` no es `:00:00Z`, toda esa primera hora parcial (`hour_utc < from_str`) **queda excluida** aunque tenga eventos `>= f.from`; y si `from_str > max_h` en la hora actual, `table_covers_full_range` devuelve `empty_range = true` dejando el Panel vacío.
  2. En el borde `to` (incluido `previous_filter`, donde `prev.to = f.from`), si `to` cae a mitad de hora (`19:30:00Z`), `'19:00:00Z' < '19:30:00Z'` es verdadero e **incluye eventos de `19:30:00Z..19:59:59Z` que están fuera de la ventana** (pasando eventos del periodo actual al periodo anterior en `deltaPct`, e incluso activando `full_range = true` si `max_h == '19:00:00Z'`).
  3. Descuadra `CostsView.by_project`: `query_hourly_agg` usa `hour_utc` y `query_sessions_by_project` usa `ts_utc`.
- **Caso concreto que falla:** Con `now = 2026-11-27T19:52:10Z` y rango `from = 2026-11-20T19:52:10Z`, `to = 2026-11-27T19:52:10Z`, un evento ocurrido a `2026-11-20T19:55:00Z` (`5,00 $`) tiene `hour_utc = '2026-11-20T19:00:00Z'`: **desaparece** del periodo actual en `dashboard()` y `costs()`, **se suma por error al periodo anterior** `prev_filter` (`to = 2026-11-20T19:52:10Z`), y en `by_project` produce `sessions: 1, usd: 0.0`. Y con `from = 2026-11-27T19:10:00Z, to = 2026-11-27T19:52:10Z` teniendo eventos a `19:40:00Z` (`max_h = '19:00:00Z'`), `dashboard()` devuelve 0 turnos y 0 sesiones por `empty_range = true`.
- **Regla incumplida:** [04-comandos.md:14-16](docs/04-comandos.md#L14-L16) (`from` inclusivo y `to` exclusivo en ISO 8601 UTC); [2026-11-26-gestor-tokens-design.md:37](docs/superpowers/specs/2026-11-26-gestor-tokens-design.md#L37) (*«Contar mal»*); [2026-11-26-gestor-tokens.md:484](docs/superpowers/plans/2026-11-26-gestor-tokens.md#L484).

---

## Hallazgos medios

### 4. [plans.rs:461-476](app/src-tauri/src/core/plans.rs#L461-L476) — Pánico por desbordamiento en `exhausts_at` con ritmo muy bajo y `exhausts_at` sin tope de 168 h en la ventana semanal
- **Problema:** `let secs = (remaining / measurement.recent_30m_used) * 1800.0;` seguido de `now + Duration::milliseconds((secs * 1000.0).round() as i64)` no acota `secs` a la duración de la ventana ni usa `checked_add_signed`. En `chrono`, `Duration::milliseconds` entra en `panic!` cuando `ms > i64::MAX / 1_000` y `DateTime + Duration` entra en `panic!` cuando el año supera `262_142` (`secs > ~8.2e12`). Además, en `WindowName::Weekly` cuando `resets_at` es `None` (siempre en Claude Pro), `resets_at.is_none_or(|end| eta < end)` acepta fechas meses o años en el futuro para una ventana de 7 días.
- **Caso concreto que falla:**
  1. Con un precio editado muy bajo (p. ej. `0,00001 $/M`) y un evento pequeño en los últimos 30 min (`recent_30m_used = 1e-9 $`) con `remaining = 10 $`, `secs = 1.8e13` → **`window_status` aborta con `panic!` (`DateTime + Duration overflowed`)**.
  2. En `claude-pro` semanal con `limit = 100 $`, `used = 50 $` y `recent_30m_used = 0,01 $`, `window_status` devuelve `exhausts_at = now + 104 días`.
- **Regla incumplida:** [2026-11-26-gestor-tokens.md:392](docs/superpowers/plans/2026-11-26-gestor-tokens.md#L392) y [línea 409](docs/superpowers/plans/2026-11-26-gestor-tokens.md#L409) (`exhaustsAt` acotado a la ventana activa y sin pánicos).

### 5. [pricing.rs:99-134](app/src-tauri/src/core/pricing.rs#L99-L134) · [cost.rs:119](app/src-tauri/src/core/cost.rs#L119) · [cost.rs:281-296](app/src-tauri/src/core/cost.rs#L281-L296) — `find_price` y `upsert_price` ignoran `provider`, haciendo que prefijos de pago pisen a `"*-free"` y que comodines afecten a otros proveedores
- **Problema:** `find_price` elige por `model_pattern.len()` sin filtrar por proveedor, y `upsert_price` tampoco comprueba `ev.provider == p.provider`. Como `"*-free"` (`provider = "other"`, `0 $`) tiene longitud 6 y los prefijos de pago tienen longitud 15–30 (`"deepseek-flash*"` = 15), cualquier modelo gratuito de opencode que comparta prefijo con uno de pago toma el precio de pago; y añadir en Ajustes una tarifa comodín (p. ej. `"*"` para `deepseek`) recalcula eventos de otros proveedores.
- **Caso concreto que falla:**
  1. Un evento de opencode con `provider = Other` y `model = "deepseek-flash-free"` casa con `"deepseek-flash*"` (longitud 15 > 6 de `"*-free"`) y queda registrado con gasto `real > 0 $` en lugar de `0 $`.
  2. Insertar `PriceInput { provider: DeepSeek, model_pattern: "*", ... }` recalcula eventos `unknown` de `"gemini-flash-worker-agy"` (`provider = Google`).
- **Regla incumplida:** [06-colectores-y-costes.md:154](docs/06-colectores-y-costes.md#L154) (*«`*-free`, `big-pickle` (proveedor `other`, herramienta `opencode`): 0 $ real»*); [2026-11-26-gestor-tokens-design.md:223](docs/superpowers/specs/2026-11-26-gestor-tokens-design.md#L223).

### 6. [plans.rs:451-459](app/src-tauri/src/core/plans.rs#L451-L459) — `window_status` conserva `resets_at` oficial de `agy` aunque ya haya vencido (`agy_reset <= now`)
- **Problema:** `official_reset` usa `agy_reset` mientras `now - agy_at < 20 min`, sin exigir `agy_reset > now`. Cuando la ventana se reinicia entre dos sondeos de 15 min, devuelve una hora de reinicio en el pasado y fuerza `exhausts_at = None` (porque `eta < end` falla al ser `end <= now < eta`).
- **Caso concreto que falla:** `agy` calibra a las `19:55Z` con `reset_time = 20:02:00Z`. A las `20:05Z` (`age = 10 min < 20 min`), `window_status` devuelve `resets_at = 20:02:00Z` (en el pasado) y `exhausts_at = None`.
- **Regla incumplida:** [06-colectores-y-costes.md:102](docs/06-colectores-y-costes.md#L102) y [2026-11-26-gestor-tokens-design.md:19](docs/superpowers/specs/2026-11-26-gestor-tokens-design.md#L19).

### 7. [alerts.rs:201-245](app/src-tauri/src/core/alerts.rs#L201-L245) · [query.rs:2266-2307](app/src-tauri/src/core/query.rs#L2266-L2307) — `compute_tool_anomalies` ignora `now` y `evaluate_anomaly_alerts` escanea toda la tabla `usage_event` en cada evaluación
- **Problema:** `compute_tool_anomalies` agrupa toda `usage_event` sin filtrar `ts_utc <= now`, de modo que eventos posteriores a `now` alteran `anomalous` y `mean`. Además, `evaluate_anomaly_alerts` llama a `sessions(1970..now+1s)` (que lee toda `usage_event` y ejecuta `compute_tool_anomalies`) y vuelve a invocar `compute_tool_anomalies` en [alerts.rs:231](app/src-tauri/src/core/alerts.rs#L231); al ejecutarse tras cada ingesta en T10, con 1 M de eventos hará múltiples barridos completos innecesarios.
- **Caso concreto que falla:** Con sesiones hasta `12:00Z` y eventos ya insertados en `14:00Z`, `evaluate_alerts_in_tz(&conn, 12:30Z, &tz)` calcula la media y anomalía incluyendo las sesiones de `14:00Z`.
- **Regla incumplida:** [AGENTS.md:59](AGENTS.md#L59) (*«Toda función que dependa de la hora recibe `now` como parámetro»*); [07-calidad.md:50-51](docs/07-calidad.md#L50-L51).

---

## Hallazgos menores

### 8. [alerts.rs:150-159](app/src-tauri/src/core/alerts.rs#L150-L159) · [query.rs:2200-2208](app/src-tauri/src/core/query.rs#L2200-L2208) · [cost.rs:117](app/src-tauri/src/core/cost.rs#L117) · [cost.rs:157](app/src-tauri/src/core/cost.rs#L157) — Criterio de borde distinto en el gasto mensual de DeepSeek y llamadas directas a `Utc::now()`
- **Problema:** `alerts.rs` suma el gasto mensual con `ts_utc <= now` en `usage_event`, mientras que `query.rs` usa `hour_utc < now` en `hourly_agg`. Por otro lado, `apply_cost` y `upsert_price` usan `Utc::now()` para `cost_computed_at` y `cost_recalc_log.at` (las firmas del plan T6 no incluyen `now`).
- **Caso concreto que falla:** Con `now = 2026-11-27T19:00:00Z` y un evento de `4,10 $` exactamente en `19:00:00Z` (presupuesto `5 $`), salta la alerta `budget.80` mientras `costs().deepseek_budget.spent_usd` muestra `0,00 $`.
- **Regla incumplida:** [04-comandos.md:16](docs/04-comandos.md#L16) y [AGENTS.md:59](AGENTS.md#L59).

### 9. [agy_usage.rs:58-60](app/src-tauri/src/core/agy_usage.rs#L58-L60) — Cubetas desconocidas de `agy /usage` se descartan sin dejar constancia para Fuentes
- **Problema:** Cuando `map_bucket_id(&raw.id)` devuelve `None`, `parse_agy_usage` hace `continue` sin exponer ni registrar cuántas cubetas desconocidas aparecieron, impidiendo contarlas como aviso en Fuentes en T10.
- **Caso concreto que falla:** Un JSON de `agy /usage` con `{"id": "imagen-5h", ...}` devuelve solo las cubetas conocidas sin indicar que hubo un `id` ignorado.
- **Regla incumplida:** [06-colectores-y-costes.md:96](docs/06-colectores-y-costes.md#L96) (*«Si aparece otro `id`, se ignora y se cuenta como aviso en Fuentes»*).

### 10. [cost.rs:232](app/src-tauri/src/core/cost.rs#L232) · [query.rs:1970-1992](app/src-tauri/src/core/query.rs#L1970-L1992) — `substr(e.ts_utc, 1, 10)` impide usar `ix_event_ts` en recálculos y `fast_epoch_secs` no verifica el sufijo `'Z'`
- **Problema:** En `upsert_price`, `WHERE substr(e.ts_utc, 1, 10) >= ?1` fuerza un escaneo completo de `usage_event` en lugar de usar `ix_event_ts` con `e.ts_utc >= 'YYYY-MM-DDT00:00:00Z'`. En `query.rs`, `fast_epoch_secs` acepta cualquier `b.len() >= 19` sin exigir `b.len() == 20 && b[19] == b'Z'`, por lo que ignoraría un offset numérico si entrara vía importación en T15.
- **Caso concreto que falla:** `upsert_price` con `valid_from = 2026-12-01` sobre 1 M de eventos de noviembre recorre las 1 M de filas; y `fast_epoch_secs(b"2026-11-26T15:00:00+02:00")` interpreta `15:00:00Z` en vez de `13:00:00Z`.
- **Regla incumplida:** [03-arquitectura.md:44](docs/03-arquitectura.md#L44) y [§3:103](docs/03-arquitectura.md#L103).

