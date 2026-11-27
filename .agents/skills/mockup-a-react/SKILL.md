---
name: mockup-a-react
description: Úsala para convertir un mockup HTML de docs/mockups en una página React del Gestor de Tokens, con ECharts y los datos reales de los comandos de Tauri en lugar de los de ejemplo.
---

# Pasar un mockup a React

Los mockups están en `docs/mockups/`. Cada pantalla es un HTML estático que usa `mockups.css` (las mismas variables que `docs/05-ui.md` §1). Los gráficos del mockup son SVG dibujados a mano: en la app se hacen con ECharts.

- Los comentarios `<!-- datos: get_xxx → campo -->` dicen qué comando y qué campo alimenta cada bloque.
- Los comentarios `<!-- estado: … -->` señalan los estados que la pantalla debe tener aunque el mockup solo muestre uno.
- `Nav` está repetida en cada archivo; en React es el componente `<Nav />`.

## Pasos

1. **Lee el mockup entero**, también los comentarios.
2. **Haz la tabla de correspondencias** en un comentario al principio del componente: cada bloque del mockup → comando y campo de `docs/04-comandos.md`.
3. **Traduce la estructura** a JSX con el mismo orden, los mismos encabezados y la misma jerarquía (`h1`, `h2`, `section`, `table`).
4. **Estilos:** pasa las clases de `mockups.css` a `app/src/styles.css` con los mismos nombres. Nada de estilos en línea salvo valores calculados (ancho de una barra).
5. **Gráficos:** cada SVG del mockup → un `<Chart option={…} />` de ECharts que reproduzca su forma (barras divergentes, mapa de calor, barra de reparto, minigráficos). Colores desde las variables CSS (léelas con `getComputedStyle`), para que funcionen los dos temas. Coste equivalente API con `decal` rayado.
6. **Sustituye los datos de ejemplo** por los del comando. Los textos fijos de `docs/05-ui.md` §3 se copian literalmente.
7. **Implementa todos los estados** de `docs/05-ui.md` §4 para esa pantalla.
8. **Accesibilidad:** `aria-label` con resumen en cada gráfico y tabla equivalente oculta; `role="progressbar"` en las barras de plan; foco visible.
9. **Datos simulados:** añade a `src/mock/mockApi.ts` la respuesta de ejemplo de la pantalla, con las mismas cifras que el mockup.

## Lo que no se hace

- Añadir librerías de componentes o de gráficos distintas de ECharts.
- Cambiar textos de la interfaz para que «suenen mejor». Si un texto del mockup parece un error, pregúntalo en `handoff/preguntas.md`.
