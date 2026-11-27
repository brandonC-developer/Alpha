# Cambios en la documentación

| Fecha | Documento | Cambio | Afecta a |
| --- | --- | --- | --- |
| 2026-11-26 | `01-fases.md` fase 0 | Comprobaciones de opencode concretas: ruta esperada de `opencode.db`, consultas `sqlite3 -readonly` solo de campos numéricos (esquemas v1 y v2), doble conteo en `step-finish`, archivos `-wal`/`-shm` y id de DeepSeek (`deepseek-flash` frente a `deepseek-v4-flash`). Instalación de `sqlite3` | Fase 0, T14 |
| 2026-11-26 | `README.md`, `05-ui.md` | Mockups publicados como canvas de diseño (https://claude.ai/artifact/7SNJzNUbZrM38kJ7VhCAd7), con navegación entre pantallas, diálogo de calibración, filtro de sesiones y tema claro u oscuro | T11–T13 |
| 2026-11-26 | Spec §8.6 y modelo de datos | Coste de API guardado en cada registro, con tipo (`real`, `api_equivalent`, `unknown`), precio usado y recálculo trazable; pantalla Costes | T6, T7, T12, T15 |
| 2026-11-26 | Todo `docs/`, `AGENTS.md`, `.agents/` | Versión inicial: investigación, diseño, plan de 16 tareas, 8 fases, jerarquía de agentes y modelos, contratos y mockups | Todas las fases |
