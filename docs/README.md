# Documentación: Gestor de Tokens

App de escritorio para Windows (Tauri 2 + Rust + React) que mide los tokens y el coste de Claude Code, Antigravity y opencode (DeepSeek y Ollama), y vigila los límites de Claude Pro y Google AI Pro.

Claude escribe esta documentación; Antigravity la usa para programar la app en `app/`. Antigravity no edita nada de esta carpeta (ver `../AGENTS.md`).

## Por dónde empezar

| Si eres… | Lee |
| --- | --- |
| El usuario, para lanzar la construcción | [01-fases.md](01-fases.md), fase 0 |
| Antigravity, al abrir el workspace | [../AGENTS.md](../AGENTS.md) y la fase en curso de [01-fases.md](01-fases.md) |
| Un subagente con una tarea | Su archivo en `../.agents/agents/` y la tarea en el plan |
| opencode (reserva) | [../AGENTS.md](../AGENTS.md) y [02-agentes-y-modelos.md](02-agentes-y-modelos.md) §5 |

## Índice

| Documento | Contenido |
| --- | --- |
| [00-investigacion.md](00-investigacion.md) | Mercado, proyectos open-source, estudios y buenas prácticas |
| [Diseño](superpowers/specs/2026-11-26-gestor-tokens-design.md) | Qué es la app, objetivos, anti-objetivos, decisiones, coste de API |
| [Plan de implementación](superpowers/plans/2026-11-26-gestor-tokens.md) | Alcance, casos de uso, buenas prácticas con su fuente y las 16 tareas con sus pruebas |
| [01-fases.md](01-fases.md) | 8 fases con orden, agente, modelo, prompt y puerta |
| [02-agentes-y-modelos.md](02-agentes-y-modelos.md) | Jerarquía N1–N4, peso y razonamiento por tarea, modelos locales, reserva DeepSeek |
| [03-arquitectura.md](03-arquitectura.md) | Componentes, dependencias permitidas, esquema SQL exacto, seguridad, ADR |
| [04-comandos.md](04-comandos.md) | Contrato de cada comando y evento de Tauri, con sus tipos |
| [05-ui.md](05-ui.md) | Sistema visual, pantallas, estados, teclado, accesibilidad, bandeja |
| [06-colectores-y-costes.md](06-colectores-y-costes.md) | Formato de cada log, precios, fixtures, alertas y exportación |
| [07-calidad.md](07-calidad.md) | Definición de hecho, estrategia de pruebas, seguridad y rendimiento |
| [08-protocolo.md](08-protocolo.md) | Cómo se comunican Claude, Antigravity, opencode y el usuario |
| [mockups/](mockups/index.html) | 6 pantallas + bandeja + navegación ([canvas](https://claude.ai/artifact/7SNJzNUbZrM38kJ7VhCAd7); copia local en `index.html`) |
| [CHANGELOG.md](CHANGELOG.md) · [respuestas.md](respuestas.md) | Cambios en la documentación y respuestas a Antigravity |

## Configuración de Antigravity incluida

```
AGENTS.md                         reglas permanentes (también las lee opencode)
.agents/rules/                    rust · web · pruebas · privacidad (por tipo de archivo)
.agents/skills/                   ejecutar-tarea-tdd · cerrar-fase · mockup-a-react
.agents/agents/                   implementador-nucleo (pro) · implementador-ui (flash)
                                  verificador (flash) · revisor (pro) · qa-app (flash)
```

Modelo de la conversación principal: **Orchestrator** (Gemini 4 Neon planifica, Gemini 4 Hydrogen ejecuta). Gemini 4 Carbon High se reserva para las revisiones de las fases 3 y 7; Claude revisa además la fase 3.
