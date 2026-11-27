---
trigger: always_on
description: Privacidad de los logs de IA y de la clave de DeepSeek en el Gestor de Tokens.
---

- Los logs de `%USERPROFILE%\.claude`, `.gemini` y opencode contienen prompts, código y razonamientos del usuario. Ningún agente los copia, resume ni cita en archivos, commits, informes de `handoff/` ni mensajes. Si necesitas ver su estructura, describe solo nombres de campos y números.
- La clave de DeepSeek nunca se escribe en ningún archivo del repositorio ni en `handoff/`.
- Las comprobaciones de la fase 0 sobre logs reales informan solo de recuentos y nombres de campos.
