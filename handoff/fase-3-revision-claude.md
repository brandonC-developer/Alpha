# Revisión de la fase 3: Claude

Fecha: 2026-11-27 · Hecha en Claude Code, porque la revisión con Claude Opus 5.5 High en Antigravity no terminó: se agotó la cuota semanal de modelos Claude y GPT en Antigravity («Third-party quota reached», se reinicia en unas 146 h).

Alcance: lectura completa de `cost.rs` y `pricing.rs`. Sobre `plans.rs`, `query.rs` y `alerts.rs`, comprobación de los hallazgos de Carbon (`fase-3-revision-carbon.md`), que comparto.

## Confirmo de la revisión de Carbon

- **Graves:**
  - n.º 1: el tiempo activo de `session_hour` falla cuando una sesión mezcla proveedores o proyectos;
  - n.º 2: `byProject` trata como 0 $ lo que no tiene precio;
  - n.º 3: los bordes del rango no coinciden con la granularidad por hora.
- **Medios:**
  - n.º 4: `exhaustsAt` sin tope y con riesgo de pánico;
  - n.º 5: `find_price` y `upsert_price` ignoran el proveedor;
  - n.º 6: se devuelve un `resets_at` de `agy` que ya venció;
  - n.º 7: las anomalías ignoran `now`.
- **Menores:** del n.º 8 al n.º 10.

## Añado

1. **Medio · `resources/pricing.json`: faltan `claude-haiku-5-5*` y `claude-fable-5-2*`.** En los logs reales de los últimos 30 días hay 59 turnos de Haiku 5.5 y 45 de Fable 5.2, que salen como «sin precio». Los precios oficiales ya están en `06` §3.1, y Sonnet 5.5 deja de ser aproximado.
2. **Menor · `cost.rs` y `pricing.rs`: el precio se elige por la fecha UTC del evento** (`ts.date_naive()` sobre UTC). Es correcto y ahora está documentado en `06` §3.2. Falta una prueba que fije el borde: un evento a las 23:30 UTC del 30/11 usa el precio de noviembre.
3. **Menor · `upsert_price` suma `old_cost.unwrap_or(0.0)` en `old_total`.** En el registro, un evento que antes no tenía precio cuenta como 0 $ «antes». Es aceptable para el log, pero `cost_recalc_log` debería contar aparte cuántos eventos no tenían precio antes. Se deja anotado, sin cambio de esquema.

## Decisiones de contrato (Claude, ya escritas en `docs/`)

- **Filtro (`04` §Filter):** el núcleo redondea `from` hacia abajo y `to` hacia arriba a la hora UTC. La interfaz envía siempre inicios de día local.
- **`byProject`:** `usd` y `avgPerSession` valen `null` si el proyecto no tiene ningún evento con precio. La media divide solo entre las sesiones con algún evento con precio.
- **`session_hour` (`03` §3):** guarda cuatro variantes por pareja (sesión, hora): `(provider, project)`, `(provider, '*')`, `('*', project)` y `('*', '*')`. La consulta lee solo la variante que corresponde al filtro.
