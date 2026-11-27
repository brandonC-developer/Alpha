# Resultado T9: Alertas de uso

- **Estado:** hecho
- **Commit:** `242c879` (`feat: alertas de uso`)
- **Resumen de `cargo test`:** 89 pruebas ejecutadas y aprobadas (0 fallidas, 1 ignorada `panel_rapido`: 6 `agy_usage`, 8 `alerts`, 12 `collector_antigravity`, 12 `collector_claude_code`, 10 `cost`, 9 `db`, 9 `ingest`, 10 `plans`, 9 `query`, 1 `smoke`, 3 unitarias de `gestor_core`); `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` en verde.

## Archivos creados

- `app/src-tauri/src/core/alerts.rs`
- `app/src-tauri/tests/alerts.rs`

## Archivos modificados

- `app/src-tauri/src/core/mod.rs` (`pub mod alerts;`)
- `app/src-tauri/src/core/plans.rs` (`pub(crate) fn active_5h_block_start`)
- `app/src-tauri/src/core/query.rs` (`pub(crate) fn compute_tool_anomalies` reutilizado entre `sessions` y `alerts`)

## Decisiones y dudas

1. **Firma pública con `now` y soporte explícito de zona horaria:** se expone `evaluate_alerts(conn: &Connection, now: DateTime<Utc>) -> Result<Vec<Alert>>` (con la firma exacta del plan, usando `chrono::Local`) delegando en `evaluate_alerts_in_tz(conn, now, tz)` para formatear horas locales (`HH:MM`), semana ISO (`%G-W%V`) y mes local (`YYYY-MM`) de forma determinista en pruebas.
2. **Claves de `setting` para umbrales y pausa:** se leen `alert.window5h.levels` (por defecto `[80, 95]`), `alert.weekly.level` (`80`), `alert.budget.level` (`80`), `alert.anomaly` (`true`) y `alerts.paused_until` (UTC ISO 8601). Si `now < paused_until`, las alertas nuevas se insertan igualmente en `alert_fired` (con `fmt_utc(now)`), pero `evaluate_alerts` no las devuelve para notificar.
3. **Clave `<plan>:<inicio del bloque>` en ventana de 5 h:** se usa `active_5h_block_start` (inicio real del bloque de `current_block`) para que la clave de deduplicación no cambie si `agy` ajusta ligeramente `resets_at` dentro del mismo bloque; las ventanas sin calibrar (`limit` / `percent` en `None`) nunca disparan alertas de porcentaje.
4. **Días restantes en `weekly.80` y saldo en `budget.80`:** cuando `ws.resets_at` existe (p. ej. vía `agy`) se calculan los días hasta el reinicio oficial redondeados hacia arriba; sin `resets_at`, se estima hasta el vencimiento de la ventana móvil de 168 h desde el último evento (o 7 días). En `budget`, si `deepseek.balance_usd` está en `setting` se muestra ese saldo; si aún no existe (antes de T14), se muestra `(budget_usd - spent_usd).max(0.0)` con formato español (`4,05 $ de 5 $ este mes` / `Saldo: 0,95 $.`).
5. **Sin dudas bloqueantes.**
