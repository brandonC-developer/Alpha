# Calidad: definición de hecho y pruebas

## 1. Una tarea está hecha cuando

- [ ] Las pruebas del plan para la tarea existen, se vieron fallar y ahora pasan.
- [ ] Toda la batería pasa: `cargo test` (en `app/src-tauri`) y, si la tarea toca la interfaz, `npm test` y `npx tsc --noEmit` (en `app/`).
- [ ] `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` sin avisos.
- [ ] Usa los nombres, firmas y valores exactos del plan y de `docs/04-comandos.md`.
- [ ] No toca archivos de otra tarea, ni `docs/`, `AGENTS.md` o `.agents/`.
- [ ] No añade dependencias fuera de `03-arquitectura.md` §2.
- [ ] Tiene un único commit con el mensaje del plan.

## 2. Una fase está hecha cuando

Se cumplen todas las casillas de su puerta en `01-fases.md` y existe `handoff/fase-N-informe.md`.

## 3. Estrategia de pruebas

| Capa | Herramienta | Qué se prueba | Cobertura esperada |
| --- | --- | --- | --- |
| Colectores | `cargo test` + fixtures de `06` §4 | Mapeo de campos, duplicados, líneas a medias, corruptas, sin contenido | Cada fila de las tablas de `06` §1 |
| Ingesta | `cargo test` + `tempfile` | Offsets, rotación, idempotencia, interrupción, agregados | Los 5 «Puntos de atención» |
| Coste | `cargo test` | Fórmula, tipos de coste, patrones, fechas, recálculo | Cada fila de `06` §2 y §3 |
| Planes y alertas | `cargo test` con `now` fijo | Bloques de 5 h, semana, calibración, ritmo, disparo único | Cada límite (79/80, 94/95 %) |
| Consultas | `cargo test` + generador sintético | KPIs, variación, hora local, filtros, anomalías | Cada campo de `Dashboard` y `CostsView` |
| Comandos | `cargo test` sin ventana | Contrato JSON, validación, errores en español | Cada comando: un caso bueno y uno malo |
| Interfaz | Vitest + Testing Library + jsdom | Formatos, estados, filtros, teclado, textos fijos | Cada estado de `05-ui.md` §4 |
| Visual | `qa-app` (navegador integrado, datos simulados) | Fidelidad a los mockups | Las 6 pantallas |
| Real | Usuario | Ventana, bandeja, notificaciones, logs reales, instalador | CU1–CU11 |
| Rendimiento | `cargo test -- --ignored panel_rapido` y Administrador de tareas | Panel < 300 ms; reposo < 1 % CPU y < 150 MB | Fase 7 |

**Hora en las pruebas:** toda función que dependa de la hora recibe `now: DateTime<Utc>` y, si hace falta, `tz`. Valores fijos: `now = 2026-11-26T16:00:00Z`, `tz = UTC−6`.

## 4. Lista de seguridad y privacidad

- [ ] Ningún struct de colector declara `content`, `thinking`, `text`, `display` ni `prompt` (`rg` en `core/collectors/`).
- [ ] La prueba `no_guarda_contenido` existe en cada colector y pasa.
- [ ] La clave de DeepSeek solo se maneja en `secrets.rs` y `deepseek.rs`; no aparece en `tracing`, en errores ni en exportaciones.
- [ ] `capabilities/default.json` coincide con `03-arquitectura.md` §5; la CSP también.
- [ ] Todo el SQL usa parámetros (`?1`, `params![]`); nunca `format!` con valores.
- [ ] Las escrituras de varias filas van en una transacción.
- [ ] La app nunca escribe en `.claude`, `.gemini` ni en la carpeta de opencode (prueba: abrirlas solo con `File::open`, nunca `OpenOptions::write`).
- [ ] Las exportaciones usan rutas relativas a `%USERPROFILE%`.

## 5. Rendimiento

- Arranque hasta mostrar el Panel con datos: ≤ 2 s (sin contar la primera importación).
- Primera importación: ≥ 50.000 eventos/s en el parser; progreso visible.
- Panel de 90 días con 1 M de eventos: < 300 ms.
- Ingesta de una línea nueva hasta `usage-updated`: < 5 s.
- Reposo: < 1 % de CPU y < 150 MB de RAM.
