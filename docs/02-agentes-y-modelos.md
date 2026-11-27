# Agentes y modelos: quién hace cada tarea, con qué modelo y con qué intensidad

Revisado el 26/11/2026 con datos medidos en el equipo del usuario y fuentes citadas en §8 y §9.

Tres herramientas y un presupuesto:

- **Claude Code (Claude Pro)** gestiona: diseña, documenta, responde a `handoff/` y revisa lo delicado. Es la cuota más escasa, así que **no programa** (§6).
- **Antigravity (Google AI Pro)** programa el núcleo y lo que tiene errores silenciosos. Orchestrator con Gemini 4 Neon planifica y Gemini 4 Hydrogen ejecuta.
- **opencode** programa en paralelo las tareas que las pruebas verifican bien, **primero con modelos gratuitos** y, cuando se agotan, con **DeepSeek de pago, con un tope de 5 $** (§5).
- **Gemini 4 Carbon (High)** solo en las revisiones de las puertas 3 y 7.

## 1. La jerarquía

```mermaid
flowchart TB
  U[Usuario] --> N4C["N4 · Claude Code<br/>Opus 5.5 (diseño, revisión) · Sonnet 5.5 (docs de rutina)"]
  U --> N3["N3 · Antigravity Orchestrator<br/>Gemini 4 Neon High"]
  N3 -->|núcleo| N2A["N2 · implementador-nucleo<br/>Gemini 4 Hydrogen"]
  N3 -->|interfaz base| N2U["N2 · implementador-ui / qa-app / verificador<br/>Gemini 4 Hydrogen"]
  N3 -->|revisión| RV["N2 · revisor<br/>Claude Sonnet 5.5 en Antigravity*"]
  U -->|tareas con pruebas fuertes| OC["N2 · opencode<br/>deepseek-v4.1-flash-free → deepseek-flash (5 $)"]
  U -.->|puertas 3 y 7| N4G["N4 · Gemini 4 Carbon High"]
```

\* Si la fase 0 confirma que los modelos Claude de Antigravity tienen cuota propia y que un subagente puede fijar su modelo (§4). Si no, el `revisor` sigue en Hydrogen `pro`.

| Nivel | Herramienta y modelo | Cuota que gasta | Hace | No hace nunca |
| --- | --- | --- | --- | --- |
| **N4 Gestor** | Claude Code: Opus 5.5 · Sonnet 5.5 | Claude Pro (5 h y semanal) | Spec, plan, contratos, respuestas a `handoff/`, revisión de la fase 3 | Escribir código de `app/` |
| **N4 Revisión de puerta** | Antigravity: Gemini 4 Carbon High | Google AI Pro (Gemini) | Puertas 3 y 7 | Corregir código |
| **N3 Planificador** | Antigravity: Orchestrator (Neon High) | Google AI Pro (Gemini) | Repartir tareas, fusionar ramas, informes | Implementar |
| **N2 Implementador** | Antigravity: Gemini 4 Hydrogen · opencode: DeepSeek V4.1 Flash | Google AI Pro · gratis · 5 $ | Código con TDD, verificación, QA | Cambiar `docs/` |

## 2. Carga, modelo e intensidad de cada tarea

**Carga** = tamaño (S < 200 líneas, M 200–600, L > 600). **Error** = cuánto cuesta detectarlo: *silencioso* si pasa unas pruebas débiles y se paga después (contar mal, cobrar mal, límites, secretos, hilos); *visible* si las pruebas o una captura lo detectan enseguida.

**Intensidad** = nivel de razonamiento del modelo. En Antigravity son variantes del modelo (`gemini-4-hydrogen-low|medium|high|xhigh`, lista de `agy models`). En opencode son variantes de DeepSeek V4.1 Flash (`low|high|max`): el agente arranca en `low` y `opencode run --variant high` sube la intensidad.

| Tarea | Carga | Error | Herramienta y agente | Modelo | Intensidad inicial | Si falla dos veces (verificado) | Revisión |
| --- | --- | --- | --- | --- | --- | --- | --- |
| T1 Proyecto Tauri | M | Visible | Antigravity `implementador-nucleo` | Hydrogen | High | XHigh | verificador |
| T2 Modelo y BD | M | Silencioso | Antigravity `implementador-nucleo` | Hydrogen | High | XHigh → `handoff/` | revisor |
| T3 Colector Claude Code | M | Silencioso | Antigravity `implementador-nucleo` | Hydrogen | High | XHigh → `handoff/` | revisor |
| T4 Colector Antigravity | S | Visible | Antigravity `implementador-nucleo` | Hydrogen | High | XHigh | revisor |
| T5 Ingestión | L | Silencioso | Antigravity `implementador-nucleo` | Hydrogen | High | XHigh → `handoff/` | revisor |
| T6 Precios y coste | M | **Silencioso, dinero** | Antigravity `implementador-nucleo` | Hydrogen | High | XHigh → `handoff/` | revisor + Carbon + Claude |
| T7 Consultas | L | Silencioso | Antigravity `implementador-nucleo` | Hydrogen | High | XHigh → `handoff/` | revisor + Carbon |
| T8 Planes y ventanas | M | **Silencioso, límites** | Antigravity `implementador-nucleo` | Hydrogen | High | XHigh → `handoff/` | revisor + Carbon + Claude |
| T9 Alertas | S | Visible | Antigravity `implementador-nucleo` | Hydrogen | High | XHigh | revisor |
| T10 Capa de escritorio | L | Silencioso (hilos) | Antigravity `implementador-nucleo` | Hydrogen | High | XHigh → `handoff/` | revisor |
| T11 Interfaz base y Panel | L | Visible | Antigravity `implementador-ui` | Hydrogen | High | XHigh | qa-app |
| T12 Costes y Sesiones | M | Visible | Antigravity `implementador-ui` | Hydrogen | High | XHigh | qa-app |
| T13 Planes, Fuentes, Ajustes | M | Visible | **opencode** `implementador` (en paralelo con T12) | DeepSeek V4.1 Flash | `low` | `high`; si vuelve a fallar, Antigravity `implementador-ui` | qa-app |
| T14 opencode y DeepSeek | M | Silencioso (secretos) | Antigravity `implementador-nucleo` | Hydrogen | High | XHigh → `handoff/` | revisor |
| T15 Exportar e importar | S | Visible | **opencode** `implementador` | DeepSeek V4.1 Flash | `low` | `high`; si vuelve a fallar, Antigravity `implementador-nucleo` | revisor |
| T16 Entrega | M | Visible | Antigravity `implementador-nucleo` + usuario | Hydrogen | High | XHigh | Carbon + usuario |
| Puertas (pruebas) | — | — | Antigravity `verificador` · o opencode `verificador` si falta cuota de Gemini | Hydrogen · DeepSeek V4.1 Flash | Medium · `low` | — | — |

Por qué T13 y T15 van a opencode: son las tareas de error visible que el plan permite hacer en paralelo (T12 ∥ T13) o que son pequeñas y están bien cubiertas por pruebas (T15). Ninguna toca tokens, dinero, límites ni secretos.

## 3. Reglas para elegir modelo e intensidad

1. **El modelo lo decide el tipo de error, no el tamaño.** Error silencioso: Antigravity Hydrogen. Error visible y tarea paralelizable o pequeña: opencode. En Terminal-Bench 4.0, Hydrogen saca 54,0 % y DeepSeek V4.1 Flash 31,8 % (§8.1); la diferencia solo se acepta donde las pruebas atrapan los fallos.
2. **Con una comprobación fiable, se empieza por la intensidad baja y se sube solo tras un fallo verificado.** Empezar en el nivel más bajo y escalar tras un fallo verificado iguala o mejora al nivel máximo fijo, con menos coste (Sun et al., 2026, §8.2). Aquí la comprobación fiable son las pruebas TDD y la comparación con los mockups.
3. **Sin comprobación fiable, se empieza en High, nunca en XHigh.** La escalera de Sun et al. exige una comprobación externa fiable, y en el núcleo los errores son silenciosos. Aun así, XHigh cuesta de 2,4 a 4,5 veces más que Low sin mejorar a High (Weinberger y Hozez, 2026), y de High a XHigh se pierden tareas que High resolvía (Sun et al., 2026). XHigh queda solo como segundo intento.
4. **Escalar, nunca empezar por arriba** (FrugalGPT, Stanford 2023): dos fallos verificados en un nivel → el siguiente. Al final de la escalera: parar y escribir en `handoff/preguntas.md`.
5. **Revisa una familia de modelos distinta de la que programó.** Los modelos evaluadores reconocen y prefieren sus propios textos (Panickssery et al., NeurIPS 2024). Por eso el `revisor` pasa a Claude cuando la fase 0 lo permita (§4), y Carbon y Claude revisan la fase 3.
6. **No se piden «piensa más» ni «considera varias alternativas» en los prompts.** Esas frases multiplican el razonamiento de 1,6 a 7,4 veces sin más aciertos (Weinberger y Hozez, 2026). La intensidad se elige con el nivel, no con palabras (lo mismo dice la guía oficial de Claude Code, §6).

## 4. Los agentes

### 4.1 Antigravity (`.agents/agents/`)

| Agente | Modelo e intensidad | Ejecución de comandos | Hace | No hace |
| --- | --- | --- | --- | --- |
| Conversación principal | Orchestrator (Neon High) | — | Lee la fase, invoca subagentes, fusiona, escribe informes | Implementar |
| `implementador-nucleo` | `pro` (Hydrogen; ver nota) | `auto` | T1–T10, T14, T16 con TDD | Tocar `app/src/` (salvo T1) |
| `implementador-ui` | `flash` (Hydrogen High) | `auto` | T11, T12 y lo que opencode devuelva | Tocar `app/src-tauri/` |
| `verificador` | `flash` | `auto` | `cargo test`, `cargo clippy`, `npm test`, `tsc`, puertas | Cambiar archivos |
| `revisor` | `pro` hoy; Claude Sonnet 5.5 High si la fase 0 lo permite | `sandbox` | Revisa el diff de la fase contra spec, plan y contratos | Cambiar archivos |
| `qa-app` | `flash` | `auto` | Navegador integrado con datos simulados: compara con los mockups | Cambiar `app/` |

Nota: en AZ-900 se midió `flash` = Hydrogen High y `pro` = Hydrogen XHigh. La fase 0 comprueba si un archivo de `.agents/agents/` acepta un id exacto de `agy models` (por ejemplo `gemini-4-hydrogen-high`). Si lo acepta, `implementador-nucleo` usa High y XHigh queda solo para el segundo intento (§3.3). Si no, se mantiene `pro`.

La ventana real de Tauri (bandeja, notificaciones, logs reales) la prueba **el usuario** en las puertas 4 y 7: Antigravity no puede controlar una ventana nativa.

### 4.2 opencode (`opencode.json` y `.opencode/agents/`)

opencode lee `AGENTS.md` y las skills de `.agents/skills/` sin configuración extra. Además:

- `opencode.json` carga las reglas de `.agents/rules/*.md` (`instructions`). También **bloquea la edición** de `docs/**`, `AGENTS.md`, `.agents/**` y de su propia configuración, impide `git push` y `--no-verify`, y bloquea las carpetas fuera del proyecto. Probado el 26/11/2026: opencode se negó a editar un `docs/` de prueba y sí editó `app/`.
- `.opencode/agents/implementador.md` (solo T13 y T15; sigue la ficha de `implementador-ui` o de `implementador-nucleo` según la tarea) y `verificador.md`, con modelo `opencode/deepseek-v4.1-flash-free` y variante `low`. Remiten a `.agents/agents/`, que sigue siendo la fuente.

Uso, desde la raíz del proyecto (o desde la copia de trabajo de la rama, ver `01-fases.md` fase 5):

```powershell
opencode run --agent implementador "Ejecuta T13"                    # intensidad low (la del agente)
opencode run --agent implementador --variant high "Ejecuta T13"     # tras un fallo verificado
opencode run --agent verificador "Comprueba la puerta 5"
```

## 5. opencode: gratis primero, DeepSeek después (tope de 5 $)

### 5.1 Qué modelo gratuito y por qué

| Modelo gratuito (`opencode/…`) | Terminal-Bench 4.0 (independiente) | Datos | Veredicto |
| --- | --- | --- | --- |
| **`deepseek-v4.1-flash-free`** | **31,8 %** (DeepSeek V4.1 Flash 1017) | Sin retención, no entrena | **Elegido.** El mejor gratuito medido, y es el mismo modelo que el de pago (`deepseek-flash`): al pasar a pago no cambia el comportamiento |
| `mimo-v2.6-flash-free` | 24,2 % | Sin retención, no entrena | Segunda opción para trabajo corto (verificar, resumir) |
| `hy4-flash-free` | 15,2 % (Hy4 Flash) | Sin retención | No para programar |
| `nemotron-3.5-super-vl-free` · `-lightning-free` | 0,0 % (Super VL) · sin dato | NVIDIA: «no envíes datos confidenciales»; registra el uso | Descartado |
| `big-pickle`, `tidepool-alpha-free`, `seaglass-alpha-free`, `iceman-free`, `flint-40b-free` | Sin medición independiente (modelos «stealth») | **Guardan los datos y los usan para entrenar** durante el periodo gratuito | Descartados: sin evidencia y con el código del proyecto como precio |

Referencia: Gemini 4 Hydrogen 54,0 %, Neon 61,6 %, Carbon 63,1 %, Claude Sonnet 5.5 64,1 % y Opus 5.5 65,2 % en la misma tabla.

### 5.2 La cadena

1. **`opencode/deepseek-v4.1-flash-free`**, con la intensidad de §2.
2. **Cuando el gratuito se agota**, cambiar a **`deepseek/deepseek-flash`** (de pago), con la misma intensidad. Se nota porque opencode devuelve «Rate limit exceeded» o el aviso del límite diario: opencode confirma que existe un límite diario para los modelos gratuitos, pero no publica la cifra. Es el mismo modelo, así que la tarea sigue igual:
   ```powershell
   opencode run --agent implementador -m deepseek/deepseek-flash --variant low "Continúa T13"
   ```
3. **Dos fallos verificados** (con cualquiera de los dos) → la tarea vuelve a Antigravity (columna «Si falla dos veces» de §2).
4. **Nunca** para tareas de error silencioso (T2, T3, T5–T8, T10, T14), ni para `revisor` o Carbon.

### 5.3 El presupuesto de 5 $

- **Tope:** 5 $ en total, en el plan `deepseek` del Gestor de Tokens (presupuesto 5 $). Antes de la fase 6 se vigila en la consola de DeepSeek; después, en Planes.
- **Precio** (`deepseek-flash`, por millón de tokens): entrada 0,15 $ (0,003 $ si sale de la caché), salida 0,60 $ **fuera de hora punta**. En hora punta todo cuesta el doble.
- **Hora punta en tu zona (UTC−6):** de domingo a jueves de 19:00 a 22:00, y de lunes a viernes de 00:00 a 04:00. DeepSeek la define en UTC (01:00–04:00 y 06:00–10:00, de lunes a viernes). Siempre que se pueda, pagar fuera de esas horas.
- **Cuánto da 5 $ (estimación):** en Terminal-Bench 4.0, cada tarea de DeepSeek V4.1 Flash costó 0,54 $ de media, y son tareas de terminal duras de hasta 8 h. Una tarea de interfaz del plan, con unas 60 llamadas, ~60 K de contexto por llamada y 90 % de caché, saldría por unos 0,15 $ fuera de hora punta. Con 5 $ caben entre ~9 y ~30 tareas, lo que sobra para T13, T15 y verificaciones. La cifra real la mide el Gestor en cuanto exista T14.
- **No usar `deepseek-v4-pro`:** cuesta 4,4 veces más y saca 14,1 % en Terminal-Bench 4.0, por debajo de Flash.

## 6. Claude: cómo gastar menos cuota

Recomendaciones de la guía oficial de costes de Claude Code (§9), aplicadas a este proyecto:

- **Modelo según el trabajo:** Opus 5.5 para diseño, contratos y la revisión de la fase 3; **Sonnet 5.5** (`/model`) para documentación de rutina (CHANGELOG, `respuestas.md`, cambios pequeños). La guía: «Sonnet handles most coding tasks well and costs less than Opus. Reserve Opus for complex architectural decisions».
- **Intensidad:** `/effort low` para ida y vuelta rápida; `medium` por defecto; `max` nunca en este proyecto.
- **`/clear` al cambiar de tarea** («Stale context wastes tokens on every subsequent message»). Una conversación por tarea de documentación.
- **Prompts concretos con rutas** («Lee `handoff/fase-3-informe.md` y actualiza `docs/06` §3»), nunca «revisa el proyecto».
- **Modo plan** (Shift+Tab) antes de cambios grandes en la documentación, para no rehacer trabajo.
- **Sin subagentes salvo para trabajo en paralelo:** su consumo cuenta igual, y la guía avisa de que los equipos de agentes gastan unas 7 veces más.
- **Contexto fijo corto:** `AGENTS.md` por debajo de 200 líneas (hoy 62). Lo especializado va en skills, que se cargan solo al usarse.
- **Caché:** en la suscripción dura alrededor de una hora; tras una pausa más larga, el primer mensaje vuelve a procesar todo. Agrupar el trabajo de Claude en bloques seguidos.
- **`/usage`** antes de cada puerta y **`/context`** si una sesión crece.
- **Revisión de la fase 3 fuera de Claude Pro:** si la fase 0 confirma que Claude Opus 5.5 en Antigravity tiene cuota propia, la revisión de la fase 3 se hace allí con Opus 5.5 High, con el mismo prompt. Claude Code solo lee el resultado.

## 7. Modelos locales: descartados

El 26/11/2026 se probó Ollama en el equipo del usuario (RTX 2050 con 4 GB de VRAM y 16 GB de RAM). `qwen3:4b` solo cabía en parte en la GPU, y una consulta corta desde opencode no terminó en 5 minutos. Ollama se desinstaló y el proyecto **no usa modelos locales**. El trabajo mecánico lo hace el mismo agente que tiene la tarea.

## 8. Evidencia

### 8.1 Benchmark independiente

[Terminal-Bench 4.0 en Vals.ai](https://www.vals.ai/benchmarks/terminal-bench-4), actualizado el 19/11/2026: 66 tareas de terminal, el mismo arnés para todos (mini-swe-agent), media de 3 ejecuciones y coste por tarea.

| Modelo | Puesto | Aciertos | Coste por tarea |
| --- | --- | --- | --- |
| Claude Opus 5.5 | 2 | 65,15 % | 12,04 $ |
| Claude Sonnet 5.5 | 3 | 64,14 % | 10,75 $ |
| Gemini 4 Carbon | 6 | 63,13 % | 7,45 $ |
| Gemini 4 Neon | 7 | 61,62 % | 6,42 $ |
| Gemini 4 Hydrogen | 14 | 54,04 % | 7,56 $ |
| DeepSeek V4.1 Flash 1017 | 29 | 31,82 % | 0,54 $ |
| MiMo V2.6 Flash | 37 | 24,24 % | 0,21 $ |
| Hy4 Flash | 46 | 15,15 % | 0,10 $ |
| DeepSeek V4 Pro 0813 | 47 | 14,14 % | 3,31 $ |
| Nemotron 3.5 Super VL | 69 | 0,00 % | 0,86 $ |

Se usa Terminal-Bench 4.0 y no SWE-bench Verified porque Vals.ai marca SWE-bench Verified como archivado (no evalúa modelos nuevos) y sus resultados están saturados (el primero saca 97 %). Las cifras que publica cada fabricante (por ejemplo, el 90,6 % de DeepSeek en Terminal-Bench 2.1) no se usan para decidir.

### 8.2 Estudios

| Estudio | Qué midió | Qué se aplica aquí |
| --- | --- | --- |
| Sun, Kwon, Hasan, Zhu, Zhou y Bianchi (Johns Hopkins, Together AI y Cornell), *What Effort Level Should I Choose? On the Economics of Reasoning Effort in LLM Agents*, arXiv:2610.34491, oct. 2026 | 8 modelos, niveles de esfuerzo de none a max, en Terminal-Bench 2.1 y SWE-Bench Pro | El nivel máximo cuesta hasta 42 veces más que el mínimo. Un nivel alto falla tareas que uno bajo resolvía. Empezar bajo y escalar solo tras un fallo verificado iguala o mejora al máximo fijo, pero requiere una comprobación externa fiable (§3.2 y §3.3) |
| Weinberger y Hozez (PointFive), *Prompt-Induced Waste in Coding Agents*, arXiv:2608.01347, ago.–sep. 2026, prerregistrado | Coste por tarea resuelta según esfuerzo, arnés y redacción del prompt | XHigh frente a Low cuesta 2,4–4,5 veces más; con Sonnet 5, XHigh queda dominado por High (1,76 $ → 2,32 $ → 3,00 $ por tarea resuelta). Las frases de «piensa más» inflan el razonamiento sin más aciertos (§3.3 y §3.6). Muestra pequeña (n = 25 por modelo) |
| Cuadron et al. (UC Berkeley, ETH Zúrich, UIUC, CMU), *The Danger of Overthinking*, arXiv:2502.08235, 2025 | 4.018 trayectorias en SWE-bench Verified | Pensar de más empeora los resultados. o1 con esfuerzo alto: 29,1 % por 1.400 $; dos intentos con esfuerzo bajo eligiendo el que menos «sobrepiensa»: 27,3 % por 800 $ |
| Chen, Zaharia y Zou (Stanford), *FrugalGPT*, 2023; Ong et al. (UC Berkeley/LMSYS), *RouteLLM*, 2024 | Cascadas y enrutado entre modelos baratos y caros | Empezar barato y escalar (§3.4) |
| Panickssery, Bowman y Feng, *LLM Evaluators Recognize and Favor Their Own Generations*, NeurIPS 2024 | Sesgo de un modelo al evaluar sus propios textos | Revisa una familia distinta (§3.5) |
| Liu et al. (Stanford), *Lost in the Middle*, 2023 | Pérdida de información en contextos largos | Una tarea por invocación y contexto mínimo |

## 9. Fuentes

- Benchmark: [Vals.ai Terminal-Bench 4.0](https://www.vals.ai/benchmarks/terminal-bench-4) · [Vals.ai SWE-bench Verified (archivado)](https://www.vals.ai/benchmarks/swebench).
- Estudios: [arXiv:2610.34491](https://arxiv.org/abs/2610.34491) · [arXiv:2608.01347](https://arxiv.org/abs/2608.01347) · [arXiv:2502.08235](https://arxiv.org/abs/2502.08235) · FrugalGPT, RouteLLM, *Lost in the Middle* y Panickssery et al. (NeurIPS 2024).
- opencode: [Zen, modelos gratuitos y su política de datos](https://opencode.ai/docs/zen) · [plan Go: límite diario de los gratuitos](https://opencode.ai/console/go) · `opencode models opencode --verbose` (11 modelos gratuitos, 26/11/2026).
- DeepSeek: [precios y hora punta](https://api-docs.deepseek.com/quick_start/pricing) · [límites de concurrencia](https://api-docs.deepseek.com/quick_start/rate_limit/) · [cambio de precios del 16/08/2026](https://api-docs.deepseek.com/news/news260813).
- Claude Code: [gestión de costes](https://code.claude.com/docs/en/costs) · [buenas prácticas](https://code.claude.com/docs/en/best-practices).
- Antigravity: `agy models` (26/11/2026) · [precios de Google Antigravity](https://antigravity.google/pricing/) (Claude Opus/Sonnet 5.5 incluidos en Google AI Pro; las fuentes no coinciden en si su cuota es aparte, por eso se comprueba en la fase 0).
