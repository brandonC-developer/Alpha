---
trigger: glob
description: Reglas para la interfaz React del Gestor de Tokens.
globs: "app/src/**/*.tsx, app/src/**/*.ts, app/src/**/*.css"
---

- Componentes de función con hooks; sin librerías de estado global ni de componentes. Gráficos solo con ECharts por `components/Chart.tsx`.
- Todas las llamadas a Rust pasan por `app/src/api.ts`; ningún `invoke` dentro de los componentes.
- Tipos de `app/src/types.ts` copiados literalmente de `docs/04-comandos.md`.
- Estilos en `styles.css` con las variables de `docs/05-ui.md` §1; los dos temas deben funcionar.
- Cifras con `format.ts` (`fmtTokens`, `fmtUsd`, `fmtPct`, `fmtDuration`); nunca `toFixed` suelto en un componente.
- Cada pantalla implementa los estados de `docs/05-ui.md` §4.
- Textos de interfaz literales del mockup y de `05-ui.md` §3, en español.
- Nunca se pide, muestra ni guarda en el navegador la clave de DeepSeek después de enviarla.
