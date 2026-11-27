# AGENTS.md: reglas permanentes para Antigravity (y opencode)

Este repositorio tiene dos autores con papeles separados:

- **Claude (Anthropic) escribe toda la documentación**: `docs/`, `AGENTS.md`, `.agents/` y los mockups.
- **Antigravity programa**: todo lo que hay en `app/`. opencode (DeepSeek o modelo local) solo actúa como reserva, con estas mismas reglas.

Estas reglas se aplican siempre, a cualquier agente y subagente.

## 1. Qué puedes y qué no puedes tocar

| Ruta | Antigravity / opencode |
| --- | --- |
| `app/**` | Crear, editar y borrar |
| `handoff/**` | Crear y editar (preguntas e informes para Claude) |
| `docs/**`, `AGENTS.md`, `.agents/**`, `HXqFtqPXgAAf9KE.jpg` | **Solo lectura.** Nunca los edites, ni para «arreglar» una errata |
| `%USERPROFILE%\.claude`, `.gemini`, carpeta de opencode | **Solo lectura**, y solo el código de la app al ejecutarse. Los agentes no copian su contenido a ningún sitio |

Si la documentación está mal, es ambigua o contradice el código, **no la cambies**: escribe la duda en `handoff/preguntas.md` (formato en `docs/08-protocolo.md`) y sigue con lo que no dependa de ella.

## 2. Fuente de verdad, en este orden

1. `docs/superpowers/specs/2026-11-26-gestor-tokens-design.md`: el diseño (qué y por qué).
2. `docs/superpowers/plans/2026-11-26-gestor-tokens.md`: las 16 tareas (cómo, con sus pruebas).
3. `docs/01-fases.md`: en qué orden, con qué agente y qué modelo.
4. `docs/03-arquitectura.md` a `docs/07-calidad.md`: contratos detallados.
5. `docs/mockups/*.html`: aspecto de cada pantalla.

Si dos fuentes se contradicen, gana la de número menor y lo anotas en `handoff/preguntas.md`. Excepción: los **nombres exactos** de tablas, columnas, comandos, campos JSON y variables CSS los fijan `docs/03-arquitectura.md`, `docs/04-comandos.md`, `docs/05-ui.md` y `docs/06-colectores-y-costes.md`.

## 3. Cómo se trabaja

- **Una tarea del plan cada vez**, en el orden de `docs/01-fases.md`. No adelantes trabajo de otra tarea.
- **TDD obligatorio**: primero la prueba que falla, ejecútala y comprueba que falla; luego el código mínimo; luego la batería completa en verde. Usa la skill `ejecutar-tarea-tdd`.
- **Nombres, firmas y valores exactos** del plan y de los contratos.
- **Un commit por tarea** con el mensaje que indica el plan. Nunca `--no-verify`.
- **Sin dependencias nuevas** fuera de `docs/03-arquitectura.md` §2. Si crees que hace falta una, pregúntalo en `handoff/preguntas.md`.
- **Al cerrar una fase** usa la skill `cerrar-fase`: no empieces la siguiente sin su informe en `handoff/`.

## 4. Reglas del producto que no se negocian

- Interfaz y mensajes **en español**.
- **Nunca se guarda, registra, muestra ni copia el contenido** de los logs leídos (prompts, respuestas, razonamientos, rutas de código). Solo números y metadatos de `docs/06-colectores-y-costes.md` §1.
- **Nunca se escribe** en las carpetas de Claude Code, Antigravity u opencode.
- **Nunca se cuenta dos veces** una llamada: `dedup_key` + `INSERT OR IGNORE`.
- **Un coste sin precio es `NULL` y `unknown`, nunca 0.** El equivalente API nunca se presenta como gasto real.
- **Un límite sin calibrar no muestra porcentaje.**
- La **clave de DeepSeek** solo vive en el Administrador de credenciales de Windows. Nunca en la base de datos, exportaciones, logs, errores ni respuestas de comandos.
- **Red:** solo `https://api.deepseek.com`, y solo si hay clave.
- `gestor_core` **no depende de Tauri**.
- Toda función que dependa de la hora recibe `now` (y `tz` si hace falta) como parámetro.

## 5. Escalado cuando algo falla

1. Una tarea falla sus pruebas **dos veces** con el subagente asignado → repítela con el nivel `pro`.
2. Falla **dos veces más** con `pro` → para, escribe el bloqueo en `handoff/preguntas.md` y avisa al usuario. No improvises otra arquitectura.
3. Nunca desactives, borres, marques `#[ignore]` ni debilites una prueba para que pase.
4. En opencode (reserva): solo tareas `flash` (T11–T13 y verificación). Si una falla dos veces, para y espera a que vuelva la cuota de Gemini.

## 6. Entorno

Windows 11 Pro · i5-12450H · 16 GB RAM · RTX 2050 4 GB · Rust estable MSVC (se instala en la fase 0) · Node 24.22 · Ollama · opencode · comandos en PowerShell. Rutas con espacios: siempre entre comillas (`"E:\Proyectos\Gestior de Tokens\Alpha"`).
