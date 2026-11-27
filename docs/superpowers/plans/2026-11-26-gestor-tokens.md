# Gestor de Tokens: plan de implementación

> **Para Antigravity:** lee primero `AGENTS.md`. Ejecuta las tareas en el orden de `docs/01-fases.md`, con la skill `ejecutar-tarea-tdd` y el subagente que indica cada fase. Los pasos usan casillas (`- [ ]`).

**Objetivo:** una app de escritorio para Windows, residente en la bandeja, que lee los registros locales de Claude Code, Antigravity y opencode, guarda cada llamada al modelo con sus tokens y su coste en dólares, y muestra uso, costes y límites de los planes del usuario.

**Arquitectura:** Tauri 2. Un núcleo Rust (`gestor_core`, biblioteca sin dependencias de Tauri, probada con `cargo test`) contiene colectores, ingestión, precios, costes, planes, consultas y alertas sobre SQLite. Una capa fina (`src-tauri/src/app/`) expone comandos y eventos de Tauri, la bandeja y las notificaciones. La interfaz es React 19 + TypeScript + ECharts dentro de WebView2 y solo habla con Rust por `invoke`.

**Stack:** Rust estable (≥ 1.85, MSVC) · Tauri 2 + plugins `notification`, `autostart`, `single-instance`, `dialog` · `rusqlite` (`bundled`) · `serde`/`serde_json` · `chrono` · `notify` · `tokio` · `reqwest` (`rustls-tls`) · `keyring` · `thiserror` · `tracing` · `globset` · Node 24 · React 19 + React Router · Vite · TypeScript 5 · ECharts 5 · Vitest + Testing Library.

**Spec:** `docs/superpowers/specs/2026-11-26-gestor-tokens-design.md` (leerla antes de empezar). Contratos exactos: `docs/03-arquitectura.md` (esquema SQL), `docs/04-comandos.md` (comandos de Tauri), `docs/05-ui.md` (interfaz), `docs/06-colectores-y-costes.md` (formatos de log, precios y fórmulas).

---

## Parte A: contexto para quien construye

### A.1 Alcance

| Hito | Incluye | Tareas |
| --- | --- | --- |
| **MVP** | Colectores de Claude Code y Antigravity, ingestión incremental, coste por registro, Panel | T1–T7, T10, T11 |
| **Planes y alertas** | Ventanas de 5 h y semanal, calibración, ritmo, alertas, bandeja con % | T8, T9, T10 |
| **Pantallas completas** | Costes, Sesiones, Planes, Fuentes, Ajustes | T12, T13 |
| **Reserva barata** | Colector de opencode (DeepSeek), saldo de DeepSeek, clave en el Administrador de credenciales | T14 |
| **v1.0** | Exportar e importar (CSV, JSON, FOCUS), instalador NSIS, rendimiento comprobado, README | T15, T16 |
| Después (no implementar) | Gemini CLI, Codex, receptor OTLP, actualizaciones automáticas, macOS/Linux | — |

### A.2 Casos de uso

| # | Situación | Flujo principal | Resultado |
| --- | --- | --- | --- |
| CU1 | Primera vez que abro la app | Instalar → la app detecta las carpetas de Claude Code y Antigravity → importa el histórico con barra de progreso → Panel | Veo meses de uso en menos de un minuto |
| CU2 | Quiero saber qué consume mi cuota | Panel → rango 7 días → tabla de agentes → clic en Antigravity | Todo el panel se filtra a Antigravity |
| CU3 | Estoy en mitad de una fase de Antigravity | Bandeja: el icono marca 72 % → menú: «Google · Gemini 5 h: 72 %, se agota a las 17:40» | Decido pausar o pasar a la reserva |
| CU4 | Calibro el límite | Planes → Claude Pro → «Calibrar» → escribo el % que muestra `/usage` → Guardar | La barra pasa a mostrar porcentaje y hora de agotamiento |
| CU5 | Me llega un aviso | La ventana de 5 h llega al 80 % → notificación de Windows | Lo sé antes de que el agente se pare |
| CU6 | ¿Me rinde Claude Pro? | Costes → rango 30 días | «Claude Pro: 20 $, equivalente API 312 $ → 15,6×» |
| CU7 | Uso DeepSeek de reserva | opencode con DeepSeek → la app lo lee → Costes muestra gasto real y saldo | Gasto real separado del equivalente |
| CU8 | Un modelo nuevo sin precio | Aparece `gemini-5-…` → Costes muestra «sin precio» → Ajustes → Precios → añadir | Se recalculan solo esos eventos y queda constancia |
| CU9 | Una sesión se disparó | Sesiones → fila resaltada → desglose por llamada | Veo qué llamada gastó más, sin ver contenido |
| CU10 | Un log cambia de formato | Fuentes → el colector marca errores con la línea y el motivo | El resto sigue funcionando; sé qué falla |
| CU11 | Exporto para analizar | Ajustes → Exportar CSV (FOCUS) | Archivo con tokens y coste por llamada, sin claves |

### A.3 Buenas prácticas que se aplican (y su fuente)

| Práctica | Cómo se aplica | Respaldo |
| --- | --- | --- |
| **TDD** | Cada tarea empieza con la prueba que falla | Nagappan et al. (Microsoft Research e IBM, 2008): 40–90 % menos defectos con un 15–35 % más de tiempo |
| **Núcleo puro, bordes finos** (arquitectura hexagonal) | `gestor_core` no conoce Tauri ni la interfaz; los comandos solo traducen | Cockburn, *Ports and Adapters*; facilita probar sin ventana |
| **Datos de prueba reales anonimizados** | Fixtures con la estructura exacta de los logs, sin contenido | Lección de ccusage y tokdash: los fallos vienen de formatos reales (duplicados, líneas a medias) |
| **Idempotencia** | `dedup_key` + `INSERT OR IGNORE`; reingestar no cambia totales | Principio de *exactly-once* en pipelines de datos |
| **No inventar cifras** | Coste `unknown` = `NULL`, nunca 0; límite sin calibrar = sin % | Anti-objetivos de la spec; FinOps Foundation: no atribuir mal |
| **Privacidad por diseño** | Los parsers solo deserializan campos numéricos | Cavoukian, *Privacy by Design*; RGPD art. 25 |
| **Mínimo privilegio** | Capabilities de Tauri mínimas; la interfaz no lee archivos | Guía de seguridad de Tauri 2 |
| **Revisiones pequeñas** | Un commit por tarea; el revisor ve el diff de una fase | Estudio de Cisco/SmartBear: la eficacia cae por encima de 200–400 líneas por revisión |
| **Contexto mínimo por agente** | Cada subagente lee solo su tarea y los contratos que cita | Liu et al. (Stanford, 2023), *Lost in the Middle*: los modelos usan peor la información en contextos largos |
| **Del modelo barato al caro** | Niveles N2→N4 y escalado solo tras fallar | Chen et al. (Stanford, 2023), *FrugalGPT*; Ong et al. (UC Berkeley, 2024), *RouteLLM* |
| **Decisiones registradas** | Decisiones en la spec y en `CHANGELOG.md` | Nygard, *Architecture Decision Records* |

### A.4 Investigación: alternativas evaluadas (noviembre de 2026)

| Necesidad | Elegido | Descartado y por qué |
| --- | --- | --- |
| Marco de escritorio | **Tauri 2** | Electron: 10× más pesado. egui/Iced: gráficos a mano. Slint: licencia GPLv3 o comercial |
| Gráficos | **ECharts 5** (Apache-2.0) | Recharts: sin mapa de calor nativo. D3: demasiado bajo nivel para este alcance |
| SQLite en Rust | **`rusqlite` con `bundled`** | `sqlx`: asíncrono y con macros de compilación que piden la BD; innecesario para un solo usuario |
| Vigilar archivos | **`notify`** + escaneo periódico | Solo sondeo: más CPU. Solo `notify`: en Windows se pierden eventos bajo carga |
| Secretos | **`keyring`** (Administrador de credenciales) | Archivo cifrado propio: hay que custodiar otra clave |
| Captura de datos | **Leer logs locales** | Proxy HTTP: invasivo y no ve las suscripciones. OTLP: Claude Code no emite tokens en sus spans |
| Referencias | ai-token-monitor, tally, tokscale, ccusage, tokdash | Se estudian; no se copia código sin revisar su licencia |

Fuentes: [`docs/00-investigacion.md`](../../00-investigacion.md).

---

## Restricciones globales

- Windows 11, toolchain `stable-x86_64-pc-windows-msvc`, Node ≥ 24.22. Sin Python.
- Datos en `%APPDATA%\GestorTokens\gestor.db` (en pruebas, siempre en memoria o en un `tempfile`).
- Solo lectura sobre `%USERPROFILE%\.claude`, `%USERPROFILE%\.gemini` y la carpeta de opencode. Nunca se escribe en ellas.
- Los parsers **solo** deserializan los campos de `docs/06-colectores-y-costes.md` §1. Nunca se guarda ni se registra `content`, `thinking`, `display`, prompts ni rutas de archivos de código.
- `dedup_key`: Claude Code `cc:<message.id>:<requestId>`; Antigravity `ag:<conversación>:<step_index>`; opencode `oc:<id del mensaje>`.
- Tokens: `input` sin caché; `output` incluye razonamiento. Antigravity: `input = input_tokens` (no incluye la caché; medido en la fase 0).
- Coste: `(input×p.input + output×p.output + cache_write×p.cache_write + cache_read×p.cache_read) / 1_000_000`. Sin precio → `cost_usd = NULL`, `cost_kind = 'unknown'`.
- `cost_kind`: plan `payg` → `real`; `subscription` → `api_equivalent`.
- Plan por evento: `claude-code` → `claude-pro`; `antigravity` → `google-ai-pro` (modelos Gemini) o `google-ai-pro-claude` (modelos `claude-*` dentro de Antigravity, que tienen su propia cuota: fase 0, 27/11/2026); `opencode` + DeepSeek → `deepseek`.
- Ventana de 5 h: empieza con el primer evento posterior al fin de la anterior y dura exactamente 5 h. Ventana semanal: las últimas 168 h.
- Peso en ventanas = `cost_usd` (equivalente API). Eventos sin precio no pesan y se cuentan aparte.
- Límite = media de las calibraciones (`usado / (porcentaje / 100)`). Sin calibrar → sin porcentaje.
- Tiempo activo: suma de huecos entre eventos consecutivos de una sesión, máximo 300 s por hueco.
- Sesión anómala: total > media + 3 × desviación típica de las últimas 30 sesiones de esa herramienta, con al menos 10 sesiones de historial.
- Alertas: 5 h al 80 % y 95 %, semanal al 80 %, presupuesto de DeepSeek al 80 %; cada una, una vez por ventana, mes o sesión.
- Red: solo `https://api.deepseek.com` y solo si hay clave.
- Interfaz y mensajes en español. Números con formato `es-ES` y abreviaturas K, M, B.
- Reposo < 1 % de CPU y < 150 MB de RAM. Panel de 90 días con 1 M de eventos < 300 ms.

## Puntos de atención en la revisión

1. **Claude Code escribe una línea por bloque de contenido con el mismo `message.id` y el mismo `usage`.** Se espera contarlo una vez. → prueba `mismo_mensaje_en_varias_lineas_cuenta_una_vez` en T3.
2. **El agente está escribiendo cuando la app lee** (última línea sin `\n`). Se espera que no se pierda ni se duplique al leer de nuevo. → prueba `linea_incompleta_no_avanza_offset` en T3 y `reingestar_no_duplica` en T5.
3. **El usuario cambia un precio hacia atrás en el tiempo.** Se espera que solo cambien los eventos de ese modelo en ese rango, y que los totales cuadren. → prueba `recalculo_solo_toca_eventos_afectados` en T6.
4. **Uso cerca de medianoche o con eventos en UTC de otro día.** Se espera que «hoy», los días del gráfico y el mapa de calor usen la hora local. → prueba `dia_local_no_utc` en T7.
5. **Primera ejecución con miles de archivos y la app cerrada a mitad.** Se espera que al reabrir siga donde iba, sin duplicar. → prueba `importacion_interrumpida_continua` en T5.

---

## Estructura de archivos

```
E:\Proyectos\Gestior de Tokens\Alpha\
├─ AGENTS.md  .agents/  docs/  handoff/          (documentación: solo Claude)
└─ app/
   ├─ package.json  tsconfig.json  vite.config.ts  vitest.config.ts  index.html  README.md
   ├─ src/                                        interfaz React
   │  ├─ main.tsx  App.tsx  api.ts  types.ts  format.ts  styles.css
   │  ├─ components/ Nav.tsx  Kpi.tsx  Chart.tsx  RangePicker.tsx  States.tsx
   │  └─ pages/ Panel.tsx  Costes.tsx  Planes.tsx  Sesiones.tsx  Fuentes.tsx  Ajustes.tsx
   ├─ tests/web/                                  Vitest
   └─ src-tauri/
      ├─ Cargo.toml  build.rs  tauri.conf.json  capabilities/default.json  icons/
      ├─ resources/pricing.json
      ├─ src/
      │  ├─ main.rs                               arranque mínimo
      │  ├─ lib.rs                                pub mod core; pub mod app; run()
      │  ├─ core/                                 gestor_core: sin Tauri
      │  │  ├─ mod.rs  model.rs  time.rs  error.rs
      │  │  ├─ db/ mod.rs  schema.sql
      │  │  ├─ collectors/ mod.rs  claude_code.rs  antigravity.rs  opencode.rs
      │  │  ├─ ingest.rs  pricing.rs  cost.rs  plans.rs  query.rs  alerts.rs
      │  │  ├─ deepseek.rs  secrets.rs  export.rs
      │  └─ app/                                  capa Tauri
      │     ├─ mod.rs  state.rs  commands.rs  watcher.rs  tray.rs  notify.rs
      └─ tests/
         ├─ fixtures/ claude_code/  antigravity/  opencode/
         └─ *.rs                                   una prueba de integración por módulo
```

---

### Tarea 1: Proyecto Tauri, arranque y prueba de humo

**Peso:** M · **Razonamiento:** medio · **Agente:** `implementador-nucleo`

**Archivos:**
- Crear: todo `app/` según la estructura (solo esqueletos), `.gitignore` en la raíz (`app/node_modules/`, `app/dist/`, `app/src-tauri/target/`).
- Prueba: `app/src-tauri/tests/smoke.rs`, `app/tests/web/smoke.test.tsx`

**Interfaces:**
- Produce: `gestor_core::version() -> &'static str` (= `env!("CARGO_PKG_VERSION")`); `app::run()`; ventana principal `main` de 1280×800, título «Gestor de Tokens»; `tauri-plugin-single-instance` (una segunda instancia enfoca la ventana existente).

- [ ] **Paso 1: pruebas que fallan**

```rust
#[test] fn version_coincide_con_cargo() { assert_eq!(gestor_core::version(), env!("CARGO_PKG_VERSION")); }
```
```tsx
test('la app monta la navegación', () => { render(<App />); expect(screen.getByRole('navigation')).toBeInTheDocument(); });
```

- [ ] **Paso 2:** `cargo test --manifest-path app/src-tauri/Cargo.toml` y `npm test` en `app/` → FALLAN.
- [ ] **Paso 3:** crear el proyecto con `npm create tauri-app@latest` (React + TS) y adaptarlo a la estructura; dependencias exactas de `docs/03-arquitectura.md` §2; `capabilities/default.json` con solo `core:default`, `notification:default`, `autostart:default`, `dialog:allow-save`, `dialog:allow-open`; CSP de `docs/03-arquitectura.md` §5.
- [ ] **Paso 4:** las dos baterías → PASAN; `npm run tauri dev` abre la ventana.
- [ ] **Paso 5:** commit `chore: proyecto Tauri 2 con React y prueba de humo`.

### Tarea 2: Modelo, tiempo y base de datos

**Peso:** M · **Razonamiento:** alto · **Agente:** `implementador-nucleo`

**Archivos:**
- Crear: `core/model.rs`, `core/time.rs`, `core/error.rs`, `core/db/mod.rs`, `core/db/schema.sql`
- Prueba: `tests/db.rs`

**Interfaces:**
- Produce:
  - `enum Tool { ClaudeCode, Antigravity, Opencode }` con `as_str()` → `"claude-code" | "antigravity" | "opencode"` y `FromStr`.
  - `enum Provider { Anthropic, Google, DeepSeek, Other }` con `as_str()` → `"anthropic" | "google" | "deepseek" | "other"`.
  - `fn provider_for(tool: Tool, model: &str, provider_hint: Option<&str>) -> Provider`: prefijo `claude-` → Anthropic; `gemini-` → Google; `provider_hint` `"deepseek"` → DeepSeek; resto Other (incluidos los modelos gratuitos de opencode, `provider_hint` `"opencode"`).
  - `struct UsageEvent` exactamente como la spec §8.2 (campos `pub`, `#[derive(Debug, Clone, PartialEq)]`).
  - `fn plan_for(tool: Tool, provider: Provider) -> &'static str` (tabla de Restricciones globales; Antigravity + Anthropic → `google-ai-pro-claude`; `other` → el plan de la herramienta).
  - `fn open_db(path: &Path) -> Result<Connection>` y `fn open_memory() -> Result<Connection>`: WAL, `foreign_keys = ON`, aplican `schema.sql` si `user_version = 0` y lo ponen a 1, y siembran las 4 filas de `plan` (`claude-pro` «Claude Pro» subscription 20 $, `google-ai-pro` «Google AI Pro · Gemini» subscription 20 $, `google-ai-pro-claude` «Google AI Pro · Claude y GPT» subscription 0 $ porque es el mismo pago, y `deepseek` «DeepSeek» payg con presupuesto 5 $) con `windows` `[{"name":"5h","hours":5},{"name":"weekly","hours":168}]` en las tres suscripciones y `[]` en `deepseek`. `INSERT OR IGNORE`: nunca pisa lo que el usuario cambió.
  - `type Result<T> = std::result::Result<T, CoreError>` con `CoreError` (`thiserror`): `Db`, `Io`, `Parse { file, line, reason }`, `Secret`, `Http`.
  - `fn hours_between(a: DateTime<Utc>, b: DateTime<Utc>) -> f64` y `fn local_date<Tz: TimeZone>(ts: DateTime<Utc>, tz: &Tz) -> NaiveDate` en `time.rs`.
- El esquema exacto está en `docs/03-arquitectura.md` §3.

- [ ] **Paso 1: pruebas que fallan**

```rust
#[test] fn esquema_se_aplica_una_vez() { /* open_db dos veces sobre el mismo archivo temporal → user_version = 1 y sin error */ }
#[test] fn tablas_existen() { /* las 10 tablas de 03-arquitectura §3 existen */ }
#[test] fn planes_sembrados() { /* 4 filas en plan: «Claude Pro», «Google AI Pro · Gemini» (cuota 20 $), «Google AI Pro · Claude y GPT» (cuota 0: es el mismo pago) y «DeepSeek»; cambiar monthly_fee_usd y reabrir → se conserva */ }
#[test] fn plan_por_herramienta() {
    assert_eq!(plan_for(Tool::ClaudeCode, Provider::Anthropic), "claude-pro");
    assert_eq!(plan_for(Tool::Antigravity, Provider::Google), "google-ai-pro");
    assert_eq!(plan_for(Tool::Antigravity, Provider::Anthropic), "google-ai-pro-claude");
    assert_eq!(plan_for(Tool::Opencode, Provider::DeepSeek), "deepseek");
}
#[test] fn dedup_key_es_unica() { /* insertar dos filas con la misma dedup_key con INSERT OR IGNORE → 1 fila */ }
#[test] fn provider_por_modelo() {
    assert_eq!(provider_for(Tool::ClaudeCode, "claude-opus-5-5", None), Provider::Anthropic);
    assert_eq!(provider_for(Tool::Antigravity, "gemini-4-neon-xhigh", None), Provider::Google);
    assert_eq!(provider_for(Tool::Opencode, "deepseek-v4-flash", Some("deepseek")), Provider::DeepSeek);
    assert_eq!(provider_for(Tool::Opencode, "big-pickle", Some("opencode")), Provider::Other);
}
#[test] fn dia_local_de_un_instante_utc() {
    let tz = FixedOffset::west_opt(6 * 3600).unwrap();
    assert_eq!(local_date("2026-11-27T03:30:00Z".parse().unwrap(), &tz), NaiveDate::from_ymd_opt(2026, 11, 26).unwrap());
}
```

- [ ] **Paso 2:** `cargo test --test db` → FALLA.
- [ ] **Paso 3:** implementar. `schema.sql` embebido con `include_str!`.
- [ ] **Paso 4:** `cargo test` → PASA.
- [ ] **Paso 5:** commit `feat: modelo de datos y esquema SQLite`.

### Tarea 3: Colector de Claude Code

**Peso:** M · **Razonamiento:** alto · **Agente:** `implementador-nucleo`

**Archivos:**
- Crear: `core/collectors/mod.rs`, `core/collectors/claude_code.rs`, `tests/fixtures/claude_code/*.jsonl`
- Prueba: `tests/collector_claude_code.rs`

**Interfaces:**
- Consume: `UsageEvent`, `Tool`, `provider_for` (T2).
- Produce:
  - `trait Collector` exactamente como la spec §8.1, con `ParseOutcome { events: Vec<UsageEvent>, new_offset: u64, errors: Vec<LineError> }` y `LineError { line: u64, reason: String }`.
  - `struct ClaudeCodeCollector { root: PathBuf }` con `ClaudeCodeCollector::default_root() -> PathBuf` (`%USERPROFILE%\.claude\projects`). `roots()` devuelve `[root]`; solo archivos `*.jsonl`.
- Formato y mapeo de campos: `docs/06-colectores-y-costes.md` §1.1. Fixtures: los de §4 de ese documento, copiados literalmente.

- [ ] **Paso 1: pruebas que fallan**

```rust
#[test] fn sesion_normal() { /* normal.jsonl → 2 eventos; el primero: input 2, cache_write 22158, cache_read 29587, output 415, reasoning 271, model "claude-opus-5-5", project Some("Alpha") */ }
#[test] fn mismo_mensaje_en_varias_lineas_cuenta_una_vez() { /* bloques.jsonl: 3 líneas con el mismo message.id y requestId → 1 evento */ }
#[test] fn linea_incompleta_no_avanza_offset() { /* incompleta.jsonl: última línea sin '\n' → 1 evento y new_offset = byte de inicio de esa línea */ }
#[test] fn linea_corrupta_se_salta_y_se_informa() { /* corrupta.jsonl → 2 eventos y errors.len() == 1 con su número de línea */ }
#[test] fn lineas_sin_usage_se_ignoran() { /* mensajes de usuario y resúmenes → 0 eventos, 0 errores */ }
#[test] fn no_guarda_contenido() { /* el Debug de cada evento no contiene "secreto-del-prompt" presente en el fixture */ }
#[test] fn lee_desde_offset() { /* parse_file(normal, offset de la 2.ª línea) → solo el 2.º evento */ }
```

- [ ] **Paso 2:** `cargo test --test collector_claude_code` → FALLA.
- [ ] **Paso 3:** implementar `parse_file` leyendo línea a línea con `BufReader`; structs `serde` con solo los campos de §1.1 (sin `deny_unknown_fields`). Deduplicación dentro del archivo con un `HashSet` de `dedup_key`.
- [ ] **Paso 4:** → PASA.
- [ ] **Paso 5:** commit `feat: colector de Claude Code`.

### Tarea 4: Colector de Antigravity

**Peso:** S · **Razonamiento:** medio · **Agente:** `implementador-nucleo` (en paralelo con T3)

**Archivos:**
- Crear: `core/collectors/antigravity.rs`, `tests/fixtures/antigravity/<uuid>/.system_generated/logs/transcript.jsonl`
- Prueba: `tests/collector_antigravity.rs`

**Interfaces:**
- Consume: `Collector`, `ParseOutcome` (T3).
- Produce: `struct AntigravityCollector { roots: Vec<PathBuf> }`, `default_roots()` = `%USERPROFILE%\.gemini\antigravity\brain` y `%USERPROFILE%\.gemini\antigravity-cli\brain`; solo archivos `transcript.jsonl` bajo `*/.system_generated/logs/` (nunca `transcript_full.jsonl`, que es una copia, ni `chunks/`). `session_id` = nombre de la carpeta de la conversación.

- [ ] **Paso 1: pruebas que fallan**

```rust
#[test] fn input_sin_restar_cache() { /* paso con input_tokens 26040 y cache_read_tokens 14167 → input 26040, cache_read 14167, output 509 */ }
#[test] fn input_menor_que_cache() { /* input_tokens 100, cache_read_tokens 150 → input 100, cache_read 150 (es lo normal: 94 % de los pasos medidos) */ }
#[test] fn ignora_transcript_full() { /* transcript_full.jsonl idéntico junto a transcript.jsonl → no se lee; siguen 3 eventos */ }
#[test] fn lee_tambien_la_cli() { /* segunda raíz antigravity-cli/brain con 1 paso de modelo → 1 evento más */ }
#[test] fn solo_pasos_de_modelo() { /* USER_INPUT y pasos sin input_tokens → ignorados */ }
#[test] fn dedup_por_step_index() { /* mismo step_index dos veces → 1 evento, dedup_key "ag:<uuid>:8" */ }
#[test] fn modelo_y_proveedor() { /* "gemini-4-neon-xhigh" → Provider::Google, tool Antigravity */ }
```

- [ ] **Paso 2:** → FALLA. **Paso 3:** implementar. **Paso 4:** → PASA.
- [ ] **Paso 5:** commit `feat: colector de Antigravity`.

### Tarea 5: Ingestión incremental

**Peso:** L · **Razonamiento:** alto · **Agente:** `implementador-nucleo`

**Archivos:**
- Crear: `core/ingest.rs`
- Prueba: `tests/ingest.rs`

**Interfaces:**
- Consume: `open_memory` (T2), colectores (T3, T4), `apply_cost` (T6: hasta que exista, llamar a un `CostHook` que no hace nada; T6 lo conecta).
- Produce:
  - `struct IngestStats { files_seen: u64, files_read: u64, new_events: u64, duplicates: u64, line_errors: u64 }`
  - `fn ingest_file(conn: &mut Connection, c: &dyn Collector, path: &Path) -> Result<IngestStats>`: lee desde el offset de `source_file`; si el tamaño actual < tamaño guardado, relee desde 0; inserta en una transacción; actualiza `hourly_agg`; guarda errores en `ingest_error`.
  - `fn scan_all(conn: &mut Connection, cs: &[Box<dyn Collector>], progress: &dyn Fn(u64, u64)) -> Result<IngestStats>`: recorre `roots()`, salta archivos sin cambios (mismo tamaño y mtime).

- [ ] **Paso 1: pruebas que fallan**

```rust
#[test] fn reingestar_no_duplica() { /* scan_all dos veces → new_events la 2.ª vez = 0, filas iguales */ }
#[test] fn solo_lee_lo_nuevo() { /* copiar fixture a tempdir, scan, añadir una línea, scan → new_events = 1 */ }
#[test] fn archivo_rotado_se_relee() { /* truncar y reescribir más corto → se relee desde 0 sin duplicar lo ya visto */ }
#[test] fn importacion_interrumpida_continua() { /* scan con un colector que falla en el 2.º archivo → el 1.º queda guardado con su offset; el 2.º scan termina y el total = el de un scan limpio */ }
#[test] fn agregado_horario_cuadra() { /* suma de hourly_agg = suma de usage_event por tokens */ }
#[test] fn errores_de_linea_se_guardan() { /* corrupta.jsonl → 1 fila en ingest_error con archivo y línea */ }
```

- [ ] **Paso 2:** → FALLA. **Paso 3:** implementar. **Paso 4:** → PASA.
- [ ] **Paso 5:** crear `app/src-tauri/examples/contar.rs`: abre una BD temporal, hace `scan_all` con las rutas reales por defecto e imprime una tabla por herramienta y modelo (turnos y los cuatro tipos de token) de los últimos 7 días. Desde T6 añade el coste equivalente de los últimos 30 días. Solo números: nunca imprime contenido ni rutas completas.
- [ ] **Paso 6:** commit `feat: ingestión incremental e idempotente`.

### Tarea 6: Precios y coste por registro

**Peso:** M · **Razonamiento:** alto · **Agente:** `implementador-nucleo`

**Archivos:**
- Crear: `core/pricing.rs`, `core/cost.rs`, `resources/pricing.json` (valores de `docs/06-colectores-y-costes.md` §3)
- Modificar: `core/ingest.rs` (conectar `apply_cost`)
- Prueba: `tests/cost.rs`

**Interfaces:**
- Produce:
  - `struct Price { id: i64, provider: Provider, model_pattern: String, valid_from: NaiveDate, valid_to: Option<NaiveDate>, input: f64, output: f64, cache_write: f64, cache_read: f64, source_url: String, approximate: bool, user_edited: bool }`
  - `fn seed_prices(conn: &Connection) -> Result<usize>`: carga `pricing.json` embebido; no pisa filas `user_edited`.
  - `fn find_price(conn: &Connection, model: &str, at: NaiveDate) -> Result<Option<Price>>`: patrones con `*` final (`globset`); gana el patrón más largo; `valid_from ≤ at < valid_to`.
  - `fn compute_cost(e: &UsageEvent, p: &Price) -> f64` (fórmula de Restricciones globales).
  - `enum CostKind { Real, ApiEquivalent, Unknown }` (`"real" | "api_equivalent" | "unknown"`).
  - `fn apply_cost(conn: &Connection, dedup_key: &str) -> Result<()>`
  - `fn upsert_price(conn: &mut Connection, p: PriceInput) -> Result<RecalcResult>`; `RecalcResult { events: u64, old_total: f64, new_total: f64 }`; escribe `cost_recalc_log`.

- [ ] **Paso 1: pruebas que fallan**

```rust
#[test] fn formula_cuatro_tipos() {
    // precio de prueba: input 5, output 25, cache_write 6.25, cache_read 0.5 ($/M)
    // evento: input 2, output 415, cache_write 22158, cache_read 29587
    // (2×5 + 415×25 + 22158×6.25 + 29587×0.5) / 1e6 = 0.16367...
    assert!((compute_cost(&ev, &p) - 0.1636710).abs() < 1e-7);
}
#[test] fn razonamiento_no_se_cobra_dos_veces() { /* reasoning 271 no cambia el coste */ }
#[test] fn sin_precio_es_unknown_y_null() { /* modelo "modelo-raro" → cost_usd NULL, cost_kind "unknown" */ }
#[test] fn suscripcion_es_api_equivalent() { /* Claude Code → "api_equivalent"; opencode+deepseek → "real" */ }
#[test] fn patron_mas_largo_gana() { /* "gemini-4-*" y "gemini-4-neon-*" → gana el segundo para "gemini-4-neon-high" */ }
#[test] fn precio_por_fecha() { /* dos precios con valid_from distinto → cada evento usa el de su fecha */ }
#[test] fn recalculo_solo_toca_eventos_afectados() { /* upsert de un precio de "claude-opus-5-5*" desde 2026-11-20 → solo cambian eventos de ese modelo desde esa fecha; RecalcResult cuadra con la suma antes y después; 1 fila en cost_recalc_log */ }
#[test] fn seed_no_pisa_precios_editados() { /* precio user_edited → seed_prices no lo cambia */ }
```

- [ ] **Paso 2:** → FALLA. **Paso 3:** implementar. **Paso 4:** → PASA (también `tests/ingest.rs`: los eventos ingeridos ya llevan coste).
- [ ] **Paso 5:** commit `feat: precios versionados y coste por registro`.

### Tarea 7: Consultas del Panel, Costes y Sesiones

**Peso:** L · **Razonamiento:** alto · **Agente:** `implementador-nucleo`

**Archivos:**
- Crear: `core/query.rs`
- Prueba: `tests/query.rs` (con un generador de eventos sintéticos en `tests/common/mod.rs`)

**Interfaces:**
- Consume: tablas de T2–T6.
- Produce (tipos `Serialize` con nombres en `camelCase`, idénticos a `docs/04-comandos.md` §2):
  - `struct Filter { from: DateTime<Utc>, to: DateTime<Utc>, tool: Option<Tool>, provider: Option<Provider>, project: Option<String> }`
  - `fn dashboard<Tz: TimeZone>(conn, f: &Filter, tz: &Tz) -> Result<Dashboard>`: KPIs (input total = input + cache_read + cache_write, % desde caché, output, output por turno, turnos, sesiones, tiempo activo, concurrencia máxima), variación frente al periodo anterior de igual duración, tokens por día local, mapa de calor 7×24 (lunes = fila 0), por modelo, por herramienta.
  - `fn costs<Tz: TimeZone>(conn, f, tz, now) -> Result<CostsView>`: KPIs de coste (real, equivalente, cuota prorrateada, rendimiento por suscripción), coste por día y proveedor, por modelo, por herramienta, por proyecto, proyección del mes, eventos sin precio.
  - `fn sessions(conn, f) -> Result<Vec<SessionRow>>` (con `anomalous: bool`) y `fn session_detail(conn, session_id: &str) -> Result<Vec<CallRow>>`.

- [ ] **Paso 1: pruebas que fallan**

```rust
#[test] fn kpis_basicos() { /* 3 eventos conocidos → totales exactos, cachePct, outputPerTurn */ }
#[test] fn variacion_periodo_anterior() { /* 100 tokens antes, 250 ahora → deltaPct 150.0; sin periodo anterior → None */ }
#[test] fn dia_local_no_utc() { /* evento 2026-11-27T03:30Z con UTC−6 → cuenta en 2026-11-26 y en el mapa de calor jueves 21 h */ }
#[test] fn tiempo_activo_con_tope() { /* huecos de 60 s y 3600 s → 360 s */ }
#[test] fn concurrencia_maxima() { /* 3 sesiones con eventos en el mismo minuto → 3 */ }
#[test] fn filtro_por_herramienta() { /* tool = Antigravity → solo sus eventos en todos los bloques */ }
#[test] fn rendimiento_suscripcion() { /* 30 días, cuota 20 $, equivalente 312 $ → 15.6 */ }
#[test] fn coste_unknown_no_suma_cero() { /* eventos sin precio → aparecen en unpricedEvents, no en los totales */ }
#[test] fn sesion_anomala() { /* 30 sesiones ~1 M y una de 40 M → anomalous true; con 9 de historial → false */ }
#[test] fn panel_rapido() { /* 1 M de eventos sintéticos (marcado #[ignore], se ejecuta en la fase 7) → dashboard 90 días < 300 ms */ }
```

- [ ] **Paso 2:** → FALLA. **Paso 3:** implementar sobre `hourly_agg` para totales y series; `usage_event` solo para sesiones y concurrencia. **Paso 4:** → PASA.
- [ ] **Paso 5:** commit `feat: consultas del panel, costes y sesiones`.

### Tarea 8: Planes, ventanas, calibración y ritmo

**Peso:** M · **Razonamiento:** alto · **Agente:** `implementador-nucleo` (en paralelo con T7)

**Archivos:**
- Crear: `core/plans.rs`, `core/agy_usage.rs`, `tests/fixtures/agy/usage.json` (literal de `06` §1.4)
- Prueba: `tests/plans.rs`, `tests/agy_usage.rs`

**Interfaces:**
- Produce:
  - `struct Plan` y `enum PlanKind { Subscription, Payg }` que leen las filas de `plan` sembradas en T2; `fn update_plan(conn, plan_id, monthly_fee_usd: f64, budget_usd: Option<f64>) -> Result<Plan>`.
  - `fn current_block(points: &[(DateTime<Utc>, f64)], now: DateTime<Utc>, hours: i64) -> Option<Block>`; `Block { start, end, used: f64 }`. `points` ordenados.
  - `fn calibrate(conn, plan_id: &str, window: WindowName, percent: f64, now) -> Result<f64>` (devuelve el nuevo límite); `percent` en (0, 100].
  - `core/agy_usage.rs`: `fn parse_agy_usage(json: &str) -> Result<Vec<AgyBucket>>`, `AgyBucket { plan_id, window: WindowName, remaining_fraction: f64, resets_at: Option<DateTime<Utc>> }`; `fn apply_agy_usage(conn, buckets: &[AgyBucket], now) -> Result<usize>` (llama a `calibrate` con `source = agy`; formato en `06` §1.4).
  - `fn window_status(conn, plan_id: &str, now) -> Result<Vec<WindowStatus>>`; `WindowStatus { name, used, limit: Option<f64>, percent: Option<f64>, resetsAt: Option<DateTime<Utc>>, exhaustsAt: Option<DateTime<Utc>>, unpricedEvents: u64 }`.

- [ ] **Paso 1: pruebas que fallan**

```rust
#[test] fn bloque_empieza_con_primer_evento() { /* eventos 09:10, 10:00, 13:00 → bloque 09:10–14:10 con los tres */ }
#[test] fn hueco_largo_abre_bloque_nuevo() { /* 09:10 y 15:00 → a las 15:30 el bloque es 15:00–20:00 */ }
#[test] fn sin_eventos_recientes_no_hay_bloque() { /* último evento hace 6 h → None */ }
#[test] fn semanal_son_168_horas() { /* evento de hace 169 h no cuenta */ }
#[test] fn calibracion_calcula_limite() { /* usado 3.0, 40 % → límite 7.5; segunda calibración 8.5 → media 8.0 */ }
#[test] fn calibracion_rechaza_porcentaje_invalido() { /* 0 y 120 → error */ }
#[test] fn sin_calibrar_no_hay_porcentaje() { /* percent None, limit None */ }
#[test] fn agy_usage_se_interpreta() { /* fixture agy/usage.json → 4 buckets: gemini-5h y gemini-weekly → google-ai-pro; 3p-5h y 3p-weekly → google-ai-pro-claude; reset 1970-01-01 → None */ }
#[test] fn agy_usage_calibra() { /* used 3.0 en la ventana 5h y remaining_fraction 0.6 → calibración source agy, 40 %, límite 7.5, resets_at oficial */ }
#[test] fn agy_usage_sin_uso_no_calibra() { /* remaining_fraction 1 o used 0 → ninguna fila nueva */ }
#[test] fn agy_usa_las_ultimas_8() { /* 10 calibraciones agy → el límite es la media de las 8 más recientes */ }
#[test] fn agy_usage_malformado() { /* JSON sin command.data.groups → Err(Parse), sin tocar la BD */ }
#[test] fn ritmo_estima_agotamiento() { /* límite 8, usado 6, 1 $ en los últimos 30 min → exhaustsAt = now + 1 h, solo si es antes del fin del bloque */ }
```

- [ ] **Paso 2:** → FALLA. **Paso 3:** implementar. **Paso 4:** → PASA.
- [ ] **Paso 5:** commit `feat: planes, ventanas de uso y calibración`.

### Tarea 9: Alertas

**Peso:** S · **Razonamiento:** medio · **Agente:** `implementador-nucleo`

**Archivos:** Crear `core/alerts.rs` · Prueba `tests/alerts.rs`

**Interfaces:**
- Consume: `window_status` (T8), `sessions` (T7).
- Produce: `struct Alert { rule: String, key: String, title: String, body: String }`; `fn evaluate_alerts(conn, now) -> Result<Vec<Alert>>`: devuelve solo alertas no disparadas antes (`alert_fired` por `rule` + `key`) y las registra. Reglas y textos exactos en `docs/06-colectores-y-costes.md` §5.

- [ ] **Paso 1: pruebas que fallan**

```rust
#[test] fn ventana_80_salta_una_vez() { /* 81 % → 1 alerta; evaluar otra vez → 0 */ }
#[test] fn ventana_nueva_vuelve_a_avisar() { /* nuevo bloque al 82 % → 1 alerta */ }
#[test] fn sin_calibrar_no_avisa() { /* plan sin límite → 0 */ }
#[test] fn presupuesto_deepseek() { /* gasto real del mes 8.1 de 10 → 1 alerta */ }
#[test] fn sesion_anomala_avisa() { /* sesión anómala → 1 alerta con key = session_id */ }
#[test] fn textos_en_espanol() { /* el título de la de 5 h es "Claude Pro: 80 % de la ventana de 5 h" */ }
```

- [ ] **Paso 2–4:** FALLA → implementar → PASA.
- [ ] **Paso 5:** commit `feat: alertas de uso`.

### Tarea 10: Capa de escritorio (comandos, vigilancia, bandeja, notificaciones)

**Peso:** L · **Razonamiento:** alto · **Agente:** `implementador-nucleo`

**Archivos:**
- Crear: `app/mod.rs`, `app/state.rs`, `app/commands.rs`, `app/watcher.rs`, `app/tray.rs`, `app/notify.rs`, `app/agy.rs` (`AgyRunner`: ejecuta `agy` sin shell con tiempo límite; la ruta del ejecutable se inyecta para poder probarlo)
- Prueba: `tests/commands.rs` (llama a las funciones de comando sin ventana, con `AppState` sobre `open_memory`)

**Interfaces:**
- Consume: todo `core`.
- Produce: los comandos y eventos exactos de `docs/04-comandos.md` (nombres `snake_case`, argumentos y respuestas `camelCase`); `AppState { db: Mutex<Connection>, collectors: Vec<Box<dyn Collector>> }`; vigilancia con `notify` (debounce 2 s) + escaneo cada 10 min; tras cada ingesta: `evaluate_alerts` → notificación; evento `usage-updated`; bandeja con icono y menú de `docs/05-ui.md` §6; cerrar la ventana la oculta (la app sigue en la bandeja); «Salir» en el menú.

- [ ] **Paso 1: pruebas que fallan**

```rust
#[test] fn get_dashboard_devuelve_contrato() { /* serde_json del resultado tiene las claves de 04-comandos §2 */ }
#[test] fn calibrate_plan_valida() { /* percent 150 → Err con message "El porcentaje debe estar entre 1 y 100" */ }
#[test] fn rescan_devuelve_estadisticas() { /* con fixtures en un tempdir → newEvents > 0 */ }
#[test] fn agy_se_ejecuta_sin_shell() { /* AgyRunner con un ejecutable falso que imprime el fixture → refresh_agy_usage calibra; args exactos ["-p","/usage","--output-format","json","--print-timeout","20s"] */ }
#[test] fn agy_ausente_o_lento() { /* sin ejecutable → code "unavailable"; ejecutable que tarda 31 s → code "timeout" y proceso terminado */ }
#[test] fn errores_como_texto_en_espanol() { /* un CoreError::Db llega como CommandError { code: "db", message } sin rutas absolutas */ }
```

- [ ] **Paso 2–4:** FALLA → implementar → PASA. Prueba manual: `npm run tauri dev`, escribir en Claude Code → el Panel se actualiza solo en < 5 s.
- [ ] **Paso 5:** commit `feat: comandos, vigilancia de logs, bandeja y notificaciones`.

### Tarea 11: Interfaz base y Panel

**Peso:** L · **Razonamiento:** medio · **Agente:** `implementador-ui`

**Archivos:**
- Crear: `src/main.tsx`, `App.tsx`, `api.ts`, `types.ts`, `format.ts`, `styles.css`, `components/*`, `pages/Panel.tsx`
- Prueba: `tests/web/format.test.ts`, `tests/web/panel.test.tsx`

**Interfaces:**
- Consume: comandos de `docs/04-comandos.md`; mockups `docs/mockups/Panel.html` y `Nav.html`.
- Produce: `api.ts` con una función tipada por comando (`getDashboard(filter)`, …) sobre `invoke`, y `listen('usage-updated')`. Si `window.__TAURI_INTERNALS__` no existe y `import.meta.env.DEV` es verdadero, `api.ts` usa `src/mock/mockApi.ts` con datos de ejemplo iguales a los de los mockups: así `npm run dev` abre la interfaz en un navegador normal (puerto 1420) para las pruebas visuales y el `qa-app`. El mock acepta `?estado=vacio` y `?estado=error` en la URL para forzar esos estados, y nunca entra en la compilación de producción; `format.ts` con `fmtTokens(n)` («11,6 B», «61,3 M», «22,9 K», «415»), `fmtUsd(n)` («0,16 $», «312,40 $»), `fmtPct(n)` («+999 %»), `fmtDuration(s)` («347 h», «23 min»); `<Chart option={…} />` envoltorio de ECharts con tema claro y oscuro.

- [ ] **Paso 1: pruebas que fallan**

```ts
test.each([[11_600_000_000,'11,6 B'],[61_300_000,'61,3 M'],[22_900,'22,9 K'],[415,'415']])('fmtTokens %d', (n, s) => expect(fmtTokens(n)).toBe(s));
test('el panel pinta los 4 KPIs con datos de ejemplo', /* api simulada → "Tokens de entrada", "11,6 B", "98 % desde caché" */);
test('clic en un agente filtra el panel', /* clic en "Antigravity" → getDashboard llamado con tool "antigravity" */);
test('estado vacío', /* sin eventos → texto de 05-ui §4 "Aún no hay uso registrado" */);
test('rangos 7/30/90/Todo', /* clic en "90 días" → nueva llamada con from = now − 90 d */);
```

- [ ] **Paso 2–4:** FALLA → implementar con la skill `mockup-a-react` → PASA.
- [ ] **Paso 5:** commit `feat: interfaz base y panel`.

### Tarea 12: Pantallas Costes y Sesiones

**Peso:** M · **Razonamiento:** medio · **Agente:** `implementador-ui`

**Archivos:** Crear `pages/Costes.tsx`, `pages/Sesiones.tsx` · Prueba `tests/web/costes.test.tsx`, `tests/web/sesiones.test.tsx`

- [ ] **Paso 1: pruebas que fallan**

```ts
test('distingue real y equivalente', /* dos series con nombres "Real" y "Equivalente API"; aviso fijo de 05-ui §3 visible */);
test('rendimiento por suscripción', /* "Claude Pro" · "15,6×" */);
test('eventos sin precio enlazan a Ajustes', /* "3 llamadas sin precio" → enlace a /ajustes#precios */);
test('sesión anómala resaltada', /* fila con aria-label que contiene "anómala" */);
test('desglose de sesión', /* clic en fila → getSessionDetail; tabla de llamadas sin columna de contenido */);
```

- [ ] **Paso 2–4:** FALLA → implementar → PASA.
- [ ] **Paso 5:** commit `feat: pantallas de costes y sesiones`.

### Tarea 13: Pantallas Planes, Fuentes y Ajustes

**Peso:** M · **Razonamiento:** medio · **Agente:** `implementador-ui`

**Archivos:** Crear `pages/Planes.tsx`, `pages/Fuentes.tsx`, `pages/Ajustes.tsx` · Prueba `tests/web/planes.test.tsx`, `tests/web/ajustes.test.tsx`

- [ ] **Paso 1: pruebas que fallan**

```ts
test('plan sin calibrar no muestra porcentaje', /* "Sin calibrar" y botón "Calibrar" */);
test('calibrar envía el porcentaje', /* escribir 40 → calibratePlan("claude-pro","5h",40) */);
test('google en una tarjeta con dos grupos', /* planes google-ai-pro y google-ai-pro-claude → una tarjeta «Google AI Pro» con «Modelos Gemini» y «Modelos Claude y GPT», 4 barras */);
test('actualizar llama a agy', /* clic en «Actualizar» → refreshAgyUsage(); calibratedBy agy hace 3 min → etiqueta «% oficial de agy» */);
test('sin agy se calibra a mano', /* refreshAgyUsage → code unavailable → texto «Sin agy: calibra a mano en Antigravity › View Usage» */);
test('recordatorio de uso no medido', /* texto de 05-ui §3 bajo las barras de suscripción */);
test('editar precio muestra el recálculo', /* upsertPrice → "Recalculadas 120 llamadas: 4,10 $ → 5,30 $" */);
test('la clave de DeepSeek no se muestra', /* tras guardar, el campo muestra "Guardada" y nunca el valor */);
test('fuente con errores', /* colector con 2 errores → "2 líneas no se pudieron leer" y botón "Reescanear" */);
```

- [ ] **Paso 2–4:** FALLA → implementar → PASA.
- [ ] **Paso 5:** commit `feat: pantallas de planes, fuentes y ajustes`.

### Tarea 14: opencode, saldo de DeepSeek y secretos

**Peso:** M · **Razonamiento:** alto · **Agente:** `implementador-nucleo`

**Prerrequisito:** `docs/06-colectores-y-costes.md` §1.3 con el formato de opencode confirmado en la fase 0. Si sigue «pendiente», para y pregunta en `handoff/preguntas.md`.

**Archivos:** Crear `core/collectors/opencode.rs`, `core/deepseek.rs`, `core/secrets.rs`, fixtures de opencode · Prueba `tests/collector_opencode.rs`, `tests/deepseek.rs`, `tests/secrets.rs`

**Interfaces:**
- Produce: `OpencodeCollector`; `fn parse_balance(json: &str) -> Result<Balance>`, `Balance { currency: String, total: f64 }`; `async fn fetch_balance(client: &reqwest::Client, key: &str) -> Result<Balance>`; `fn set_secret(name: &str, value: &str) -> Result<()>`, `fn get_secret(name: &str) -> Result<Option<String>>`, `fn delete_secret(name: &str) -> Result<()>`; servicio de keyring `"GestorTokens"`, nombre `"deepseek_api_key"`.

- [ ] **Paso 1: pruebas que fallan**

```rust
#[test] fn opencode_deepseek() { /* fixture → un evento DeepSeek (plan deepseek, real) y uno de opencode/deepseek-v4.1-flash-free (provider other, plan deepseek, real, 0 $) */ }
#[test] fn no_escribe_en_la_carpeta_de_opencode() { /* fixture opencode.db en WAL dentro de una carpeta temporal → tras leer, la carpeta tiene los mismos archivos y fechas: se leyó una copia (06 §1.3) */ }
#[test] fn ignora_step_finish_y_abortados() { /* partes step-finish con los mismos tokens → no suman; mensaje sin time.completed → no es evento */ }
#[test] fn tokens_de_opencode() { /* input 551, output 416, reasoning 121, cache.read 8448 → input 551, output 416 + 121 = 537, cache_read 8448 */ }
#[test] fn balance_usd() { /* {"is_available":true,"balance_infos":[{"currency":"USD","total_balance":"8.42",…}]} → 8.42 */ }
#[test] fn balance_malformado() { /* JSON sin balance_infos → Err(Parse) */ }
#[test] #[ignore] fn keyring_ida_y_vuelta() { /* set → get → delete con un nombre de prueba; se ejecuta a mano en Windows */ }
#[test] fn la_clave_no_sale_en_logs() { /* el Debug de los errores de fetch_balance no contiene la clave */ }
```

- [ ] **Paso 2–4:** FALLA → implementar → PASA. Conectar el saldo a `get_plans` (cada 15 min) y la clave a `set_deepseek_key`.
- [ ] **Paso 5:** commit `feat: opencode, saldo de DeepSeek y clave segura`.

### Tarea 15: Exportar e importar

**Peso:** S · **Razonamiento:** medio · **Agente:** `implementador-nucleo`

**Archivos:** Crear `core/export.rs` · Prueba `tests/export.rs`

**Interfaces:** `fn export_csv(conn, f: &Filter, w: &mut dyn Write) -> Result<u64>`; `fn export_json(…)`; `fn import_json(conn: &mut Connection, r: &mut dyn Read) -> Result<IngestStats>` (en una transacción; `INSERT OR IGNORE`). Columnas exactas en `docs/06-colectores-y-costes.md` §6.

- [ ] **Paso 1: pruebas que fallan**

```rust
#[test] fn cabecera_focus() { /* primera línea = columnas de 06 §6 */ }
#[test] fn sin_rutas_absolutas_ni_claves() { /* el CSV no contiene "C:\\Users" ni "deepseek_api_key" */ }
#[test] fn ida_y_vuelta() { /* export_json → import_json en BD vacía → mismos totales */ }
#[test] fn importar_corrupto_no_toca_nada() { /* JSON roto → Err y 0 filas nuevas */ }
```

- [ ] **Paso 2–4:** FALLA → implementar → PASA.
- [ ] **Paso 5:** commit `feat: exportar e importar`.

### Tarea 16: Empaquetado, rendimiento y README

**Peso:** M · **Razonamiento:** medio · **Agente:** `implementador-nucleo` + usuario

- [ ] **Paso 1:** `tauri.conf.json`: instalador **NSIS** (por usuario, sin permisos de administrador), `webviewInstallMode: downloadBootstrapper`, icono.
- [ ] **Paso 2:** autoarranque desactivado por defecto; se activa en Ajustes.
- [ ] **Paso 3:** `cargo test -- --ignored panel_rapido` → < 300 ms. Medir reposo con el Administrador de tareas 10 min: < 1 % de CPU y < 150 MB (anotarlo en el README).
- [ ] **Paso 4:** `app/README.md`: instalar, primera ejecución, calibrar, añadir precios, exportar, desinstalar, dónde están los datos.
- [ ] **Paso 5:** `npm run tauri build` genera `app/src-tauri/target/release/bundle/nsis/*.exe`; el usuario lo instala y recorre CU1–CU11.
- [ ] **Paso 6:** commit `chore: instalador, rendimiento y README`.

---

## Cobertura de la spec

| Spec | Tareas |
| --- | --- |
| §2.1 Panel, §2.2 por día y mapa de calor, §2.3 modelos, §2.4 agentes | T7, T11 |
| §2.5 Planes y límites, §8.5 | T8, T13 |
| §2.6 y §8.6 Coste de API | T6, T7, T12 |
| §2.7 Bandeja | T10 |
| §2.8 y §8.8 Alertas | T9, T10 |
| §2.9 Histórico propio, §8.3 Ingestión | T3, T4, T5 |
| §2.10 Exportar | T15 |
| §8.9 Privacidad | T3, T4 (`no_guarda_contenido`), T10 (capabilities), T14 (keyring), T15 |
| §8.10 Rendimiento | T7 (`panel_rapido`), T16 |
| DeepSeek | T14 |
