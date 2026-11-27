---
name: ejecutar-tarea-tdd
description: Úsala para implementar una tarea numerada (T1–T16) del plan del Gestor de Tokens. Aplica TDD estricto con los nombres y valores exactos del plan y termina con un commit.
---

# Ejecutar una tarea del plan con TDD

## Entrada

Un número de tarea, `TN`. El plan está en `docs/superpowers/plans/2026-11-26-gestor-tokens.md`.

## Pasos

1. **Lee la tarea entera**: peso, archivos, bloque «Interfaces» y pasos. Lee también «Restricciones globales» y, si la tarea tiene alguno, sus «Puntos de atención».
2. **Comprueba lo que consume.** Cada función, tipo o tabla de «Consume» debe existir ya en `app/` con esa firma exacta. Si falta, para y escríbelo en `handoff/preguntas.md`: significa que una tarea anterior quedó incompleta.
3. **Comprueba los prerrequisitos** de la tarea (por ejemplo, T14 necesita `docs/06` §1.3 completo). Si no se cumplen, para y pregunta.
4. **Escribe las pruebas del paso «pruebas que fallan»**, todas. Donde el plan pone `/* … */`, completa la prueba con aserciones concretas que comprueben exactamente lo que dice su nombre y los valores del comentario. Nunca dejes una prueba sin aserciones.
5. **Ejecútalas y confirma que fallan** por el motivo esperado (función, módulo o archivo inexistente), no por un error de sintaxis de la prueba. Si pasan sin código nuevo, la prueba está mal: corrígela.
6. **Implementa lo mínimo** para que pasen, con las firmas exactas de «Produce». Lógica pura en `core/` sin E/S salvo la base de datos; la hora llega como parámetro.
7. **Ejecuta la batería completa**:
   - Rust: `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` (en `app/src-tauri`).
   - Interfaz: `npm test` y `npx tsc --noEmit` (en `app/`).
   Todo debe pasar, también las pruebas de tareas anteriores.
8. **Commit** solo de los archivos de la tarea, con el mensaje exacto del plan:

   ```
   git add <archivos de la tarea>
   git commit -m "<mensaje del plan>"
   ```

9. **Informa** al agente que te invocó: tarea, archivos, pruebas nuevas, resultado de la batería y hash del commit.

## Lo que no se hace

- Cambiar una prueba para que pase en vez de arreglar el código.
- Añadir dependencias que no estén en `docs/03-arquitectura.md` §2.
- Editar `docs/`, `AGENTS.md` o `.agents/`.
- Mezclar dos tareas en un commit.
- Copiar contenido de logs reales a fixtures, mensajes o informes: los fixtures son los de `docs/06` §4.
