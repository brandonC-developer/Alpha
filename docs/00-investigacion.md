# Investigación: Gestor de consumo de tokens multi‑plataforma (Windows, Rust)

Fecha: 2026-11-26 · Referencia visual: `HXqFtqPXgAAf9KE.jpg`

## 1. Qué muestra la imagen de referencia (lo que hay que replicar)

| Bloque | Métrica | Por qué importa |
|---|---|---|
| KPIs | Input tokens (con **% desde caché**), output tokens (**por turno**), tareas/turnos, tiempo de agente (concurrencia "hasta 10 a la vez") | Separa volumen bruto de coste real (la caché cuesta ~10% del input normal) |
| Tokens por día | Barras divergentes input (arriba) / output (abajo) | El input domina el coste en agentes |
| "When they work" | Heatmap día × hora | Patrones de uso, picos vs. ventanas de límite |
| Models | Reparto por proveedor (Anthropic/Google/OpenAI) y por modelo: input, output, tareas | Comparar eficiencia entre modelos |
| Agents | Claude Code, Antigravity, Codex, Grok CLI, Junie, opencode; clic = filtrar | La fuente de datos es **la herramienta**, no solo el proveedor |
| Rango | 7 / 30 / 90 días / All + delta % vs periodo anterior | Tendencias |

Conclusión: el producto es un **agregador local de logs de agentes + APIs**, no un proxy. Falta en la imagen (y conviene añadir): **coste en $**, **límites de plan** (ventanas 5 h / 7 d, cuotas de tiers gratuitos) y **alertas**.

## 2. Proyectos open‑source existentes

### 2.1 Apps de escritorio en Rust/Tauri (lo más cercano a tu objetivo)
| Proyecto | Stack | Cobertura | Nota |
|---|---|---|---|
| [soulduse/ai-token-monitor](https://github.com/soulduse/ai-token-monitor) | Tauri, tray | Claude Code, Codex, OpenCode · **Windows y macOS** | El mejor punto de partida para estudiar en Windows |
| [goodnight000/tally](https://github.com/goodnight000/tally) | Tauri + Rust backend + SQLite | Varias herramientas locales; dashboards tokens/gasto/modelos | Buena arquitectura de ingestión → SQLite |
| [hitheshkaranth/OpenTokenMonitor](https://awesome.ecosyste.ms/projects/github.com%2Fhitheshkaranth%2Fopentokenmonitor) | Tauri, Tokio, reqwest | Claude, Codex, Gemini | Tray residente |
| [burnrate](https://docs.rs/crate/burnrate/0.1.1) | Tauri 2 + React | Claude Code, Codex, OpenRouter, Runpod | Incluye APIs de pago (OpenRouter) |
| [bluvenr/tokenowl](https://github.com/bluvenr/tokenowl) | Tauri + SQLite | — | |
| [HduSy/tokenscope](https://github.com/HduSy/tokenscope) | Tauri 2 + React | Coste diario, por modelo | Solo macOS |
| [kimdj2/claude-token-monitor](https://github.com/kimdj2/claude-token-monitor) | Tauri 2 + Svelte | Claude (vía ccusage) | Ejemplo simple de widget de bandeja |
| [xbotter/codex-cost](https://github.com/xbotter/codex-cost) | Tauri 2 tray | Codex | Mínimo |
| [tokenusage](https://docs.rs/crate/tokenusage/1.1.4) / [cc-monitor-rs](https://docs.rs/crate/cc-monitor-rs/latest/source/README.md) | Rust CLI/TUI/GUI | Claude Code, Codex | Parsers en Rust reutilizables como referencia |

### 2.2 CLIs / dashboards locales (referencia de parsers y formatos)
- **ccusage** (ryoppippi, npm) — el estándar de facto para Claude Code; reportes diarios, sesión y **bloques de 5 h** ([docs blocks](https://www.mintlify.com/ryoppippi/ccusage/features/blocks-reports)).
- **ccusage (PyPI)** — distinto proyecto: lee los **límites reales** (5 h y 7 d) desde la API de Anthropic ([pypi](https://pypi.org/project/ccusage/)). Clave para mostrar "% del plan".
- [Claude-Code-Usage-Monitor](https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor) — presets por plan (Pro/Max).
- [tokscale](https://github.com/junhoyeo/tokscale) — multi‑herramienta; rutas: `~/.claude/projects/`, `~/.codex/sessions/`, `~/.gemini/tmp/`.
- [tokdash](https://pypi.org/project/tokdash/) — índice SQLite incremental; **advierte que Claude Code y Gemini CLI borran sesiones > ~30 días** → hay que ingerir continuamente para no perder histórico.
- [code-meter](https://pypi.org/project/code-meter/) — incluye Google Antigravity.
- [tokentracker-cli](https://socket.dev/npm/package/tokentracker-cli/overview/1.2.1) — dice cubrir 43 herramientas.
- [JedIV/token-tracker](https://github.com/JedIV/token-tracker) — detalle útil: en Codex usar `last_token_usage` por turno para **no contar doble** (los totales son acumulativos).

### 2.3 Observabilidad LLM (para APIs propias, no para suscripciones)
- **Langfuse** (MIT, tracing + coste por usuario + evals), **Helicone** (proxy, licencia a verificar), **LiteLLM** (gateway, presupuestos, emite OTel), **OpenLIT** (OTel nativo). Patrón común: LiteLLM → OpenLIT/OTLP → Langfuse ([comparativa](https://tidal-2j3.pages.dev/research/langfuse-vs-litellm-vs-openlit), [PostHog](https://posthog.com/compare/best-open-source-llm-observability-tools)).
- Son servidores pesados; para una app de escritorio basta con **recibir OTLP** localmente y reutilizar el **catálogo de precios de LiteLLM** (`model_prices_and_context_window.json`) como tabla de precios.

> Licencias y actividad no verificadas repo por repo: revisarlas antes de copiar código.

## 3. Fuentes de datos a soportar (3 tipos de "plan")

| Tipo | Ejemplos | Cómo se obtiene el dato | Qué mostrar |
|---|---|---|---|
| **Suscripción** | Claude Pro/Max, ChatGPT Plus/Pro (Codex), Gemini/Antigravity, Grok, JetBrains (Junie) | Logs locales JSONL de cada CLI + endpoint de límites cuando exista | % de ventana 5 h / semanal, hora de reset, "coste equivalente API" |
| **API de pago** | Anthropic, OpenAI, Google, xAI, OpenRouter | Admin/Usage APIs de cada proveedor + OTLP + logs | $ reales facturados vs estimados, presupuestos |
| **Tiers gratuitos** | Gemini API free, Groq, OpenRouter `:free`, Mistral, Cohere | Contadores locales (RPM/RPD/TPD) + cabeceras `x-ratelimit-*` | Cuota restante del día, aviso antes de agotar |

Los límites gratuitos cambian a menudo (p. ej. Google dejó de publicar límites free en docs en 2026, Groq ~30 RPM / 14.4k RPD en modelos pequeños) → **configurables por el usuario, nunca hardcodeados** ([OpenRouter: free APIs 2026](https://openrouter.ai/blog/tutorials/free-llm-apis-compared/)).

Telemetría estándar: Claude Code exporta métricas y eventos vía **OpenTelemetry** (`CLAUDE_CODE_ENABLE_TELEMETRY=1`, OTLP). Usar nombres de la convención **OTel GenAI** (`gen_ai.usage.input_tokens`, `gen_ai.usage.output_tokens`, `gen_ai.request.model`, …), aún en estado *Development*; ojo: `input_tokens` en semconv incluye lecturas/escrituras de caché ([OTel GenAI](https://opentelemetry.io/docs/specs/semconv/gen-ai/gen-ai-spans/), [Honeycomb](https://docs.honeycomb.io/send-data/agents)).

## 4. Estudios académicos y de expertos

- **"How Do AI Agents Spend Your Money?"** (arXiv:2604.22750, 2026; Microsoft Research / Stanford Digital Economy Lab; NeurIPS 2026) — [paper](https://arxiv.org/pdf/2604.22750)
  - Las tareas agénticas consumen **~1000× más tokens** que chat/razonamiento de código.
  - **El input domina el coste**, no el output → coincide con tu imagen (11.6B input vs 61M output, 98% caché).
  - Variación de **hasta 30×** entre ejecuciones de la misma tarea; más gasto ≠ más acierto (el acierto toca techo a coste intermedio).
  - La elección de modelo pesa más que la dificultad de la tarea; los modelos **no saben predecir** su propio consumo (r ≤ 0.39) y lo subestiman.
  - → Implicación de diseño: mostrar **distribuciones y outliers por tarea**, no solo totales; alertas por sesión anómala.
- **Tokenomics** (arXiv:2601.14470) — consumo por fase del ciclo de desarrollo en sistemas multi‑agente → etiquetar uso por proyecto/fase.
- **Token Economics for LLM Agents** (arXiv:2605.09104) — revisión computación + economía; la atribución de coste es un problema abierto.
- **FinOps Foundation – State of Tokenomics (sept. 2026)** y la nueva **Tokenomics Foundation** (Linux Foundation, jun. 2026) — [finops.org](https://www.finops.org/insights/token-economics/)
  - El token es la unidad de cuenta portable, pero el valor se mide en **resultados** (coste por tarea completada / PR / ticket).
  - Medir solo volumen lleva a mala atribución; añadir costes fijos (suscripciones).
  - Exportar en formato **FOCUS** y etiquetar consistentemente (proyecto, herramienta, modelo).

## 5. Buenas prácticas consolidadas (para el diseño)

1. **Local‑first y privacidad**: leer logs locales; **no guardar prompts ni código** por defecto, solo métricas. Claves API en **Windows Credential Manager** (crate `keyring`), nunca en texto plano.
2. **Ingestión incremental e idempotente**: guardar offset/hash por fichero, deduplicar por `message.id`/`request_id` (Claude Code repite entradas en sesiones reanudadas), usar deltas por turno en Codex.
3. **Retener histórico propio**: las CLIs borran logs a ~30 días → la app debe copiar a su SQLite en segundo plano.
4. **Separar 4 tipos de token**: input, output, cache‑write, cache‑read (y reasoning cuando exista). Sin esto el coste es falso.
5. **Coste estimado vs facturado**: etiquetar claramente; precios versionados por fecha (el precio de un modelo cambia).
6. **Normalizar a OTel GenAI** internamente → permite exportar a Grafana/Langfuse sin reescribir.
7. **Límites como datos**: tabla editable de planes/cuotas, con fuente "oficial (API)" o "estimada (logs)".
8. **Métricas de valor**: coste por tarea, tokens por turno, % caché, tiempo de agente, concurrencia.
9. **Alertas** (notificación nativa de Windows): 80% de ventana 5 h, presupuesto diario, sesión 3σ por encima de la media.
10. **Bajo consumo**: watcher de ficheros (no polling agresivo), arranque con Windows opcional, residente en bandeja.

## 6. Propuesta de arquitectura (Rust, Windows)

```
┌────────────── Tauri 2 (UI WebView2: Svelte/React + gráficos) ──────────────┐
│ Dashboard (réplica de la imagen) · Planes/Límites · Alertas · Ajustes       │
└───────────────▲─────────────────────────────────────────────────────────────┘
                │ comandos Tauri / eventos
┌───────────────┴──────────────── núcleo Rust (tokio) ────────────────────────┐
│ Collectors (trait Collector):                                               │
│   • LogCollector: Claude Code, Codex, Gemini CLI, Antigravity, opencode,    │
│     Grok CLI, Junie…  (crate `notify` para vigilar %USERPROFILE%\.claude…)  │
│   • OtlpReceiver: servidor OTLP/HTTP local (axum + opentelemetry-proto)     │
│   • ProviderApi: Anthropic/OpenAI usage & cost APIs, OpenRouter /key        │
│   • RateLimit: límites 5h/7d, cuotas free (RPM/RPD/TPD)                     │
│ Normalizer → UsageEvent{ts, tool, provider, model, in, out, cache_r,        │
│              cache_w, reasoning, session, project, task_id, duration}       │
│ Pricing: catálogo LiteLLM + overrides de usuario, versionado por fecha      │
│ Storage: SQLite (rusqlite/sqlx), WAL, tablas agregadas diarias/horarias     │
│ Alerts: tauri-plugin-notification · Tray: icono con % de ventana actual     │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Crates clave**: `tauri` 2 (+ tray, notification, autostart, updater), `tokio`, `notify`, `rusqlite` o `sqlx`, `serde`/`serde_json`, `chrono`, `reqwest`, `keyring`, `axum` + `opentelemetry-proto` (receptor OTLP), `tiktoken-rs` (conteo OpenAI) / `tokenizers` (HuggingFace) solo para estimaciones; para Claude usar el endpoint `count_tokens` o los usos que ya vienen en los logs.

**¿Por qué Tauri 2 y no egui/Iced/Slint?** Para un dashboard rico en gráficos (heatmap, barras divergentes) las librerías web (ECharts, Recharts, D3) ahorran meses; Tauri usa WebView2 (ya incluido en Windows 11), binario ~5–10 MB, instalador MSI/NSIS, tray y notificaciones oficiales, y es lo que usan casi todos los proyectos de §2.1. egui/Iced serían opción si quieres **100% Rust sin web**, a cambio de construir los gráficos a mano. Slint: ojo a su licencia (GPLv3 o royalty‑free con condiciones).

## 7. Hoja de ruta sugerida

1. **MVP (2–3 semanas)**: Claude Code + Codex desde logs → SQLite → KPIs + tokens por día + tabla de modelos. Tray con tokens de hoy.
2. **v0.2**: Gemini CLI, Antigravity, opencode; heatmap; filtro por agente; rangos 7/30/90/All con delta %.
3. **v0.3**: tabla de precios + coste $; planes de suscripción con ventanas 5 h/7 d; alertas.
4. **v0.4**: receptor OTLP; APIs de proveedores (facturado real); tiers gratuitos con cuotas.
5. **v1.0**: exportación CSV/FOCUS, autoupdate firmado, instalador MSI, tests con logs de ejemplo anonimizados.

## Fuentes
- Proyectos: enlaces en §2.
- [arXiv:2604.22750 — How Do AI Agents Spend Your Money?](https://arxiv.org/pdf/2604.22750) · [Stanford Digital Economy Lab](https://digitaleconomy.stanford.edu/publication/how-do-ai-agents-spend-your-money-analyzing-and-predicting-token-consumption-in-agentic-coding-tasks/)
- [arXiv:2601.14470 — Tokenomics](https://arxiv.org/pdf/2601.14470) · [arXiv:2605.09104 — Token Economics for LLM Agents](https://arxiv.org/pdf/2605.09104)
- [FinOps Foundation — State of Tokenomics](https://www.finops.org/insights/token-economics/) · [Microsoft FinOps toolkit + FOCUS](https://techcommunity.microsoft.com/blog/finopsblog/managing-azure-openai-costs-with-the-finops-toolkit-and-focus-turning-tokens-int/4413886)
- [OpenTelemetry GenAI semconv](https://opentelemetry.io/docs/specs/semconv/gen-ai/gen-ai-spans/) · [OTel blog GenAI observability](https://opentelemetry.io/blog/2026/genai-observability/index.md)
- [Rust GUI: Tauri vs Iced vs egui (benchmark)](https://lukaskalbertodt.github.io/2023/02/03/tauri-iced-egui-performance-comparison.html) · [LogRocket: estado de GUIs en Rust](https://blog.logrocket.com/state-of-rust-gui-libraries)
- [tiktoken-rs](https://docs.rs/crate/tiktoken-rs/latest)
- [Free LLM APIs 2026 (OpenRouter)](https://openrouter.ai/blog/tutorials/free-llm-apis-compared/)
