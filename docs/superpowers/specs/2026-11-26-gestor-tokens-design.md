# Diseño: Gestor de Tokens (app de escritorio para Windows)

Fecha: 2026-11-26 · Estado: borrador para aprobación · Investigación previa: [`docs/00-investigacion.md`](../../00-investigacion.md) · Referencia visual: `HXqFtqPXgAAf9KE.jpg`

## 1. Intención

Una app de escritorio para Windows, residente en la bandeja, que muestra **cuántos tokens gasta el usuario con cada herramienta y cada modelo**, **cuánto le queda de cada plan** y **cuánto le costaría** ese uso a precio de API.

El usuario paga **Claude Pro** y **Google AI Pro**, y usa además **DeepSeek por API** como reserva barata. Hoy no sabe qué herramienta consume su cuota ni cuándo se le va a acabar la ventana de 5 horas. Lo descubre cuando el agente se para a mitad de una fase.

La app lee los registros que las herramientas ya escriben en el disco (sin proxy, sin tocar las herramientas), los guarda en su propia base de datos y los presenta como en la imagen de referencia: KPIs, tokens por día, mapa de calor, reparto por modelo y por agente.

## 2. Objetivos

1. **Panel de uso** (la imagen de referencia): tokens de entrada (con % desde caché), de salida (con media por turno), turnos, sesiones y tiempo activo de agente; rangos 7 / 30 / 90 días / Todo, con variación frente al periodo anterior.
2. **Tokens por día** en barras divergentes (entrada arriba, salida abajo) y **mapa de calor** día × hora («cuándo trabajan»).
3. **Modelos**: reparto por proveedor (Anthropic, Google, DeepSeek, otros) y tabla por modelo con entrada, salida, caché, turnos y coste equivalente.
4. **Agentes**: tabla por herramienta (Claude Code, Antigravity, opencode) con entrada, salida y tiempo; un clic filtra todo el panel por esa herramienta.
5. **Planes y límites**: para cada suscripción, consumo de la ventana de 5 horas y de la semanal, hora de reinicio y ritmo («a este ritmo se agota a las 17:40»); para DeepSeek, saldo y gasto del mes.
6. **Coste de API por registro** (sección 8.6): cada llamada se guarda con su coste en dólares, el precio usado y si es coste real (DeepSeek) o equivalente API (suscripciones). Pantalla **Costes** con gasto por día, modelo, herramienta y proyecto, y el rendimiento de cada suscripción («Claude Pro: 20 $ al mes, uso equivalente 312 $ → 15,6×»).
7. **Bandeja del sistema**: icono con el % de la ventana de 5 h más cargada y un menú con el resumen de hoy.
8. **Alertas** con notificaciones nativas de Windows: ventana de 5 h al 80 % y al 95 %, presupuesto mensual de DeepSeek y sesión anómala (muy por encima de la media).
9. **Histórico propio**: la app conserva los datos aunque las herramientas borren sus registros (Claude Code borra a los ~30 días).
10. **Exportar** a CSV y JSON.

## 3. Lo que no se hace

- No es multiusuario ni sincroniza con la nube.
- No hace de proxy ni intercepta peticiones de las herramientas.
- No lee el uso de las webs de chat (claude.ai, gemini.google.com): no dejan registros locales. Se tienen en cuenta solo como «uso no medido» en los límites (ver 8.5).
- No guarda prompts, respuestas, código ni razonamientos: solo números y metadatos.
- No hay versión para macOS ni Linux en la v1 (Tauri lo permitiría después).
- No controla ni corta el uso: informa y avisa.

## 4. Anti-objetivos (fallaría aunque funcione)

- **Contar mal.** Duplicar entradas de sesiones reanudadas o sumar totales acumulados como si fueran deltas. Un número inflado es peor que no tener número. La ingestión es idempotente y está probada con logs reales anonimizados.
- **Un coste que parece exacto y no lo es.** Todo coste estimado se marca como «equivalente API» con la fecha del precio usado; solo DeepSeek muestra coste real.
- **Filtrar contenido privado.** Los registros incluyen prompts y razonamientos; si la app copia o muestra algo de eso, falla aunque los números estén bien.
- **Un límite falso presentado como oficial.** Anthropic y Google no publican el tamaño de las ventanas en tokens. La app muestra el límite como estimado, dice de dónde sale y deja calibrarlo.
- **Un monitor que consume.** Si la app usa más de 1 % de CPU en reposo o más de 150 MB de RAM, molesta más de lo que ayuda.

## 5. Restricciones

- Windows 11 Pro · i5-12450H · 16 GB de RAM · RTX 2050 con 4 GB de VRAM.
- **Rust no está instalado** (se instala en la fase 0: `rustup`, MSVC Build Tools, comprobación de WebView2). Node 24.22 sí está.
- Ollama se probó y se desinstaló el 26/11/2026: con 4 GB de VRAM era demasiado lento. El proyecto no usa modelos locales.
- Planes: **Claude Pro**, **Google AI Pro** (Gemini app, Gemini CLI y Antigravity con Gemini 4), **DeepSeek API** de pago por uso como reserva.
- Registros locales verificados el 26/11/2026:
  - Claude Code: `%USERPROFILE%\.claude\projects\<proyecto>\<sesión>.jsonl`, con `message.usage` = `input_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens`, `output_tokens`, `output_tokens_details.thinking_tokens`; `message.model`; `message.id`.
  - Antigravity: `%USERPROFILE%\.gemini\antigravity\brain\` y `%USERPROFILE%\.gemini\antigravity-cli\brain\` (`<conversación>\.system_generated\logs\transcript.jsonl`; nunca `transcript_full.jsonl`, que es una copia), con `model` (p. ej. `gemini-4-neon-xhigh`), `input_tokens`, `cache_read_tokens`, `output_tokens`, `created_at`, `step_index`.
- Interfaz en español.
- Todo gratis salvo el gasto de DeepSeek, que el usuario controla.

## 6. Enfoques considerados

### 6.1 Cómo se construye la app

| Enfoque | Ventajas | Inconvenientes |
| --- | --- | --- |
| **Tauri 2 + React/TypeScript (elegido)** | Núcleo Rust; WebView2 ya viene en Windows 11; binario pequeño; bandeja, notificaciones, autoarranque e instalador oficiales; librerías de gráficos web; Antigravity ya sabe pasar mockups a React | Dos lenguajes (Rust y TS) |
| Rust puro con egui o Iced | Un solo lenguaje | Mapa de calor y barras divergentes hechos a mano; menos ejemplos |
| App web local en Node (como AZ-900) | Lo más rápido | No es una app de Windows ni usa Rust, que es lo que pidió el usuario |

### 6.2 Quién lo construye y con qué modelo

| Enfoque | Ventajas | Inconvenientes |
| --- | --- | --- |
| A. Igual que AZ-900 + nivel local | Coste 0 € | Si la cuota de Gemini se agota, la fase espera |
| **B. A + DeepSeek como reserva (elegido por el usuario)** | Nunca se queda parado; DeepSeek V4 Flash ≈ 0,28 $/M de salida | Gasto real pequeño; calidad inferior a Gemini 4 |
| C. Programa Claude Code | Máxima calidad | Claude Pro agota la ventana de 5 h en pocas tareas de Rust |

## 7. Decisiones de cómo se hace

Elegidas por el usuario:

- App de escritorio para Windows, en Rust.
- Planes medidos: Claude Pro, Google AI Pro y API barata (DeepSeek). El modelo local se descartó el 26/11/2026.
- Flujo de trabajo B: Claude documenta y revisa; Antigravity programa; DeepSeek como reserva. El nivel local para trabajo mecánico se descartó junto con Ollama.
- **Revisado el 26/11/2026 (usuario):** opencode programa junto con Antigravity, primero con su mejor modelo gratuito y, cuando se agota, con DeepSeek de pago con un tope de 5 $. Cada tarea lleva su carga, modelo e intensidad, respaldados por benchmarks independientes y estudios (`docs/02-agentes-y-modelos.md`).
- Misma estructura de trabajo que `E:\Proyectos\Azure\AZ-900`.

Decisiones de Claude (el usuario puede cambiarlas):

- **Tauri 2** con núcleo Rust y la interfaz en **React 19 + TypeScript + Vite**.
- **Gráficos:** Apache ECharts (heatmap, barras apiladas y divergentes; licencia Apache-2.0).
- **Base de datos:** SQLite con `rusqlite` (feature `bundled`, sin dependencias del sistema), modo WAL, en `%APPDATA%\GestorTokens\gestor.db`.
- **Lectura de logs:** vigilancia con el crate `notify` y, además, un escaneo completo al arrancar y cada 10 minutos, por si un evento se pierde.
- **Claves** (DeepSeek) en el Administrador de credenciales de Windows con el crate `keyring`. Nunca en la base de datos, exportaciones ni logs.
- **Precios:** tabla propia `pricing.json` versionada por fecha, con valores iniciales del catálogo de LiteLLM y de las páginas oficiales; editable en Ajustes.
- **Pruebas:** `cargo test` para el núcleo (con logs de ejemplo anonimizados en `fixtures/`) y Vitest + Testing Library para la interfaz.
- **Ubicación del código:** `E:\Proyectos\Gestior de Tokens\Alpha\app\`.

## 8. Diseño

### 8.1 Piezas

```
Interfaz (React + ECharts, WebView2)
   │  comandos e eventos de Tauri (invoke / emit)
Núcleo Rust (Tauri 2, tokio)
   ├─ Colectores (trait Collector)
   │    ├─ ClaudeCodeCollector   ~/.claude/projects/**/*.jsonl
   │    ├─ AntigravityCollector  ~/.gemini/antigravity/brain/*/.system_generated/logs/transcript.jsonl
   │    ├─ OpencodeCollector     sesiones de opencode (DeepSeek)            [v0.3]
   │    └─ DeepSeekBalance       GET https://api.deepseek.com/user/balance  [v0.3]
   ├─ Normalizador → UsageEvent
   ├─ Precios (pricing.json, versionado por fecha)
   ├─ Planes y límites (ventanas 5 h y semanales, estimación y calibración)
   ├─ Agregador (consultas por rango, modelo, agente, hora)
   ├─ Alertas → tauri-plugin-notification
   ├─ Bandeja → icono con % y menú
   └─ Almacén SQLite (rusqlite, WAL)
```

El trait común:

```rust
pub trait Collector: Send + Sync {
    fn id(&self) -> &'static str;                 // "claude-code", "antigravity", …
    fn roots(&self) -> Vec<PathBuf>;              // carpetas a vigilar
    fn parse_file(&self, path: &Path, from_offset: u64) -> Result<ParseOutcome>;
}
pub struct ParseOutcome { pub events: Vec<UsageEvent>, pub new_offset: u64 }
```

### 8.2 Evento normalizado

```rust
pub struct UsageEvent {
    pub dedup_key: String,        // único: ver 8.3
    pub tool: Tool,               // ClaudeCode | Antigravity | Opencode
    pub provider: Provider,       // Anthropic | Google | DeepSeek | Other
    pub model: String,            // tal como viene: "claude-opus-5-5", "gemini-4-neon-xhigh"
    pub ts_utc: DateTime<Utc>,
    pub session_id: String,
    pub project: Option<String>,  // carpeta de trabajo, si el log la da
    pub input: u64,               // sin caché
    pub cache_write: u64,
    pub cache_read: u64,
    pub output: u64,              // incluye razonamiento
    pub reasoning: u64,           // parte de output, si se conoce
}
```

Nombres alineados con la convención OpenTelemetry GenAI (`gen_ai.usage.input_tokens`, etc.) para poder exportar después sin traducir.

### 8.3 Ingestión correcta

- **Incremental:** por cada archivo se guarda ruta, tamaño, fecha de modificación y offset leído. Solo se lee lo nuevo. Si el archivo encoge (rotación), se relee desde 0.
- **Idempotente:** `dedup_key` es la clave primaria y se inserta con `INSERT OR IGNORE`.
  - Claude Code: `message.id` + `requestId`. Las sesiones reanudadas repiten mensajes y deben contar una vez.
  - Antigravity: `<conversación>:<step_index>`.
- **Solo números:** el parser deserializa solo los campos de 8.2 (structs con `serde` que ignoran el resto). Nunca guarda `content`, `thinking` ni rutas de archivos de código.
- **Líneas rotas:** una línea JSON incompleta al final (el agente aún está escribiendo) no avanza el offset; se reintenta en la siguiente lectura. Una línea corrupta en medio se salta y se cuenta en `ingest_error`.
- **Entrada sin caché, siempre.** Las herramientas no cuentan igual:
  - Claude Code: `input_tokens` ya excluye la caché → `input = input_tokens`.
  - Antigravity: `input_tokens` **no** incluye la caché → `input = input_tokens`. Medido en la fase 0 (27/11/2026): en 1.632 de 1.744 pasos con caché, `input_tokens < cache_read_tokens` (19 modelos, app y CLI). La suposición inicial, que restaba la caché, era errónea.
- **Definiciones del panel:**
  - *Turno* = un evento con uso (una llamada al modelo).
  - *Sesión* = un archivo de sesión de Claude Code o una conversación de Antigravity.
  - *Tiempo activo* = suma de los huecos entre eventos consecutivos de una sesión, contando como máximo 5 minutos por hueco.
  - *Concurrencia* = número máximo de sesiones con eventos en el mismo minuto.
- **Primera ejecución:** importa todo el histórico disponible, mostrando el progreso.
- **Herramienta desconocida o formato cambiado:** el colector marca el error en la pantalla Fuentes; no rompe el resto.

### 8.4 Modelo de datos

- `usage_event`: los campos de 8.2, más `cost_usd` (REAL, NULL si no hay precio), `cost_kind` (`real` | `api_equivalent` | `unknown`), `price_id` y `cost_computed_at`; índices por `ts_utc`, `model`, `tool`.
- `cost_recalc_log`: fecha, precio cambiado, eventos afectados, coste anterior y nuevo (total).
- `source_file`: ruta, colector, tamaño, mtime, offset, última lectura, errores.
- `ingest_error`: archivo, línea, motivo, fecha.
- `price`: id, proveedor, patrón de modelo, válido_desde, válido_hasta, $/M de entrada, de salida, de escritura en caché y de lectura de caché, fuente (URL) y si lo editó el usuario.
- `plan`: id, nombre, proveedor, herramientas incluidas, tipo (`subscription` | `payg`), ventanas (JSON: duración y límite estimado en tokens ponderados), cuota mensual en $, presupuesto mensual.
- `plan_calibration`: plan, fecha, % que mostraba la herramienta oficial, tokens medidos en ese momento.
- `alert_rule` y `alert_fired`: regla, umbral, última vez que saltó (para no repetir).
- `daily_agg` y `hourly_agg`: agregados precalculados por día y hora × modelo × herramienta, para que el panel cargue al instante.
- `setting`: clave, valor.

### 8.5 Planes y límites

Ninguna de las dos suscripciones publica sus límites en tokens, así que la app los **estima** y lo dice.

- **Ventana deslizante de 5 h y ventana semanal** por plan. La de 5 h empieza con el primer mensaje tras un periodo sin ventana activa, igual que hace `ccusage blocks`.
- **Tokens ponderados**: la cuota no gasta igual con todos los modelos. Peso por defecto = coste equivalente API del evento (así un token de Opus pesa más que uno de Hydrogen).
- **Límite estimado** = el valor que el usuario calibra. Calibrar: el usuario mira `/usage` en Claude Code o «View Usage» en Antigravity, escribe el % que ve y la app calcula el límite (`medido / %`). Varias calibraciones se promedian. Sin calibración, la barra muestra solo el consumo, sin porcentaje.
- **Uso no medido:** si el usuario también usa claude.ai o la app de Gemini, el % real será mayor que el medido. La pantalla lo recuerda junto a la barra.
- **Ritmo:** con el consumo de los últimos 30 minutos, la app estima a qué hora se agotaría la ventana.
- **DeepSeek:** saldo real con `GET /user/balance` cada 15 minutos (si hay clave) y coste real por evento con su tabla de precios.
Planes iniciales: `claude-pro` (Claude Code), `google-ai-pro` (Antigravity, Gemini CLI: modelos Gemini), `google-ai-pro-claude` (modelos Claude dentro de Antigravity, con cuota propia), `deepseek` (opencode con DeepSeek).

### 8.6 Coste de API

Cada registro de uso se guarda **con su coste en dólares**, calculado al ingerirlo.

**Fórmula** (precios en $ por millón de tokens, del `price` vigente en la fecha del evento):

```
cost_usd = ( input       × price.input
           + output      × price.output
           + cache_write × price.cache_write
           + cache_read  × price.cache_read ) / 1_000_000
```

- Los tokens de razonamiento ya van dentro de `output` y se cobran a precio de salida; no se suman dos veces.
- Se guarda con precisión completa (`REAL`); se redondea solo al mostrar (4 decimales por evento, 2 en totales).

**Tipo de coste (`cost_kind`):**

| Tipo | Cuándo | Qué significa |
| --- | --- | --- |
| `real` | Plan `payg` (DeepSeek) | Lo que se cobra de verdad; se contrasta con el saldo de `/user/balance` |
| `api_equivalent` | Plan `subscription` (Claude Pro, Google AI Pro) | Lo que costaría el mismo uso pagando por API; no es un gasto |
| `unknown` | Modelo sin precio (p. ej. un modelo nuevo) | `cost_usd = NULL`; la interfaz muestra «sin precio» y un enlace para añadirlo. **Nunca se cuenta como 0** |

**Precios:**

- `pricing.json` de serie, con precios de las páginas oficiales de Anthropic, Google y DeepSeek (y el catálogo de LiteLLM como respaldo), cada uno con fecha y URL de origen.
- Modelos por patrón: `gemini-4-neon-*` cubre `-high` y `-xhigh`; `claude-opus-5-5*` cubre las variantes con fecha.
- Si Google no publica precio de API para un modelo de Antigravity, se usa el de su equivalente público más cercano y se marca como «aproximado» en la tabla de precios.
- Historial de precios: un cambio crea una fila nueva con `válido_desde`; las filas antiguas no se borran.

**Recálculo:** al editar o añadir un precio, la app recalcula en una transacción solo los eventos de ese modelo dentro del rango de fechas afectado, actualiza `price_id`, `cost_usd` y `cost_computed_at`, y escribe una línea en `cost_recalc_log` (eventos afectados, total anterior y total nuevo). La pantalla de Ajustes enseña ese historial.

**Pantalla Costes:**

- KPIs del rango: **gasto real** (DeepSeek), **equivalente API** (suscripciones), **cuota pagada** (suma de cuotas mensuales prorrateadas) y **rendimiento** = equivalente API / cuota pagada, por suscripción.
- Coste por día, apilado por proveedor; real y equivalente con estilos distintos (sólido y rayado) para no confundirlos.
- Tablas por modelo, por herramienta y por proyecto: tokens, coste y coste medio por sesión.
- Proyección del mes al ritmo actual y barra del presupuesto de DeepSeek.
- Desglose de una sesión: lista de llamadas con tokens y coste (sin contenido).
- Aviso permanente: «El equivalente API es una referencia: con tu suscripción no pagas por token».

**Exportación:** CSV y JSON por evento con `ts_utc`, herramienta, modelo, los cuatro tipos de token, `cost_usd`, `cost_kind` y el precio usado; compatible con las columnas básicas de FinOps FOCUS (`BilledCost`, `ConsumedQuantity`, `ChargePeriodStart`).

### 8.7 Pantallas

1. **Panel** (la imagen de referencia): 4 KPIs con minigráfico y variación; tokens por día; mapa de calor; modelos; agentes. Filtros: rango, herramienta, proveedor y proyecto.
2. **Costes**: la de 8.6.
3. **Planes**: una tarjeta por plan con las barras de 5 h y semanal, el reinicio, el ritmo y el botón «Calibrar». DeepSeek muestra saldo, gasto del mes y presupuesto.
4. **Sesiones**: tabla de sesiones (fecha, herramienta, proyecto, modelo principal, tokens, coste, duración), ordenable, con las anómalas resaltadas; al abrir una, su desglose por llamada.
5. **Fuentes**: cada colector con su carpeta, archivos leídos, último evento, errores y botón «Reescanear».
6. **Ajustes**: precios (con historial de recálculos), planes, alertas, clave de DeepSeek, autoarranque con Windows, tema claro u oscuro, exportar e importar.
7. **Menú de la bandeja**: tokens y coste de hoy, % de cada ventana de 5 h y «Abrir panel».

### 8.8 Alertas

- Ventana de 5 h al 80 % y al 95 % (solo si el plan está calibrado).
- Ventana semanal al 80 %.
- Gasto de DeepSeek del mes al 80 % del presupuesto.
- Sesión anómala: tokens totales > media + 3 desviaciones de las últimas 30 sesiones de esa herramienta (el estudio de arXiv:2604.22750 muestra variaciones de hasta 30× entre ejecuciones de la misma tarea).
- Cada alerta salta una vez por ventana o por sesión.

### 8.9 Privacidad y seguridad

- Solo lectura sobre las carpetas de las herramientas; nunca las escribe.
- Ninguna conexión de red salvo `api.deepseek.com` (si hay clave) y la comprobación de actualizaciones (desactivable).
- Clave de DeepSeek en el Administrador de credenciales de Windows.
- CSP estricta en la interfaz; permisos (capabilities) de Tauri mínimos: sin acceso al sistema de archivos desde JavaScript; toda lectura la hace Rust.
- Exportaciones sin claves ni rutas personales completas (rutas relativas a `%USERPROFILE%`).

### 8.10 Rendimiento

- Reposo: < 1 % de CPU, < 150 MB de RAM (WebView2 incluido).
- Panel de 90 días con 1 millón de eventos: < 300 ms (gracias a los agregados).
- Importación inicial: ≥ 50.000 eventos/s en el parser.

### 8.11 Pruebas

- Parsers probados con **logs reales anonimizados** (sin contenido; solo la estructura y los números) en `app/src-tauri/tests/fixtures/`: sesión normal, sesión reanudada con duplicados, línea incompleta al final, línea corrupta, archivo rotado.
- Ventanas de 5 h: inicio, fin, huecos, cambio de día y de horario de verano.
- Coste: fórmula con los cuatro tipos de token (valores calculados a mano en la prueba); razonamiento no sumado dos veces; cambio de precio a mitad del rango; modelo sin precio (`unknown`, `NULL`, nunca 0); recálculo que solo toca los eventos afectados y deja línea en `cost_recalc_log`; `real` frente a `api_equivalent` según el tipo de plan.
- Interfaz: estados vacío, cargando, error y con datos; filtro por agente; teclado.

## 9. Jerarquía de trabajo para construirla

Resumen (el detalle está en `docs/02-agentes-y-modelos.md`):

| Nivel | Razonamiento | Modelo | Trabajo |
| --- | --- | --- | --- |
| N4 Arquitecto | Máximo | Claude Opus 5.5 (Claude Code, Claude Pro) · Gemini 4 Carbon High | Diseño, documentación, revisiones de puerta |
| N3 Planificador | Alto | Gemini 4 Neon (Orchestrator) | Repartir tareas y fusionar |
| N2 Implementador | Medio o Extra | Gemini 4 Hydrogen (`pro` para Rust y lógica, `flash` para interfaz) | Código con TDD |
| N2 opencode | Variante `low`, escala a `high` | DeepSeek V4.1 Flash en opencode: gratis (`opencode/deepseek-v4.1-flash-free`) y, al agotarse, de pago (`deepseek-flash`, tope de 5 $) | T13, T15 y verificación: tareas con error visible |

## 10. Lo que queda para quien lo construya

- La forma exacta del icono de bandeja con el porcentaje.
- La estrategia de agregación (vistas materializadas o tablas mantenidas al insertar), siempre que cumpla 8.10.
- El texto de los mensajes de error, siguiendo el tono de los mockups.

## 11. Decisiones de Claude para revisar

- Tauri 2 + React 19 + TypeScript + ECharts; `rusqlite` con `bundled`.
- Datos en `%APPDATA%\GestorTokens\`.
- Coste guardado en cada registro al ingerirlo (no calculado al consultar), con recálculo trazable cuando cambia un precio.
- Precios de modelos de Antigravity sin precio público: el del equivalente público más cercano, marcado «aproximado».
- Límites de los planes como estimación calibrada por el usuario, con peso = coste equivalente API.
- El uso de claude.ai y de la app de Gemini queda fuera de la medición.
- opencode como puerta única para DeepSeek: un solo colector lo mide. Los modelos gratuitos de opencode salen como proveedor `other`, con coste real 0 $ (precio `*-free` en `06` §3).
- El plan construye la v1.0 completa en hitos: primero Claude Code y Antigravity con el panel (MVP), después planes y alertas, y al final opencode y DeepSeek. La ruta exacta donde opencode guarda sus sesiones en Windows se comprueba en la fase 0, porque opencode aún no está instalado.
