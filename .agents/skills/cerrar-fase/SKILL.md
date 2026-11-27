---
name: cerrar-fase
description: Úsala al terminar una fase de construcción del Gestor de Tokens. Comprueba la puerta de la fase con el verificador y el revisor, y escribe el informe para Claude en handoff/.
---

# Cerrar una fase

## Entrada

Un número de fase, `N` (1 a 7). Las puertas están en `docs/01-fases.md`.

## Pasos

1. Invoca el subagente `verificador` con «comprueba la puerta de la fase N». Incluye en su informe la salida de `git diff --name-only <primer commit de la fase>~1..HEAD`: solo puede haber rutas de `app/` y `handoff/` (`docs/09-seguridad.md` §5).
2. Invoca el subagente `revisor` con «revisa la fase N», salvo en la fase 5 (la revisa `qa-app`) y la 7 (la revisa Carbon). En las fases 3 y 7, recuerda al usuario la revisión con **Carbon High** que describe `docs/01-fases.md`; en la fase 3, también la de **Claude**.
3. Si el `verificador` marca alguna casilla como no cumplida, o el `revisor` encuentra algo grave: invoca al implementador de la fase para corregirlo, como una tarea más con TDD y su propio commit `fix: …`. Después vuelve al paso 1. Como máximo dos vueltas; después, escala según `AGENTS.md` §5.
4. Escribe `handoff/fase-N-informe.md` con esta plantilla:

```markdown
# Informe de la fase N

Fecha: AAAA-MM-DD · Modelo principal: … · Niveles usados: … · ¿Se usó la reserva DeepSeek?: no / sí (tareas)

## Puerta
| Casilla | Resultado | Evidencia |
| --- | --- | --- |

## Rutas tocadas
<git diff --name-only de la fase>

## Comandos bloqueados o que pidieron aprobación
<lista; vacía si no hubo>

## Commits
<git log --oneline de la fase>

## Revisión
<hallazgos del revisor y qué se hizo con cada uno>

## Casillas que debe comprobar el usuario
<lista de las marcadas «no comprobable», con el paso exacto>

## Decisiones tomadas sin estar en el plan
<lista; vacía si no hubo>

## Preguntas abiertas para Claude
<enlace a las entradas de handoff/preguntas.md; vacía si no hay>
```

5. Avisa al usuario: «Fase N cerrada; informe en `handoff/fase-N-informe.md`. Pide a Claude que valide la seguridad de la fase». No empieces la fase siguiente hasta que el usuario lo diga.
