---
trigger: always_on
description: Seguridad del equipo del usuario. Qué no puede hacer ningún agente fuera del proyecto.
---

- Trabajas solo dentro de `E:\Proyectos\Gestior de Tokens\Alpha` (o de la copia de trabajo que te indiquen). No leas, crees, muevas ni borres nada fuera de ella.
- Nunca ejecutes comandos que borren carpetas enteras (`rmdir /s`, `rd /s`, `del /s`, `Remove-Item -Recurse`, `rm -r`), que formateen o particionen discos, que toquen el registro, permisos (`icacls`, `takeown`), la política de ejecución o el arranque, ni que apaguen o reinicien el equipo.
- Nunca instales programas ni paquetes globales (`winget`, `choco`, `npm install -g`, `cargo install`, `rustup`). Si falta una herramienta, pídela en `handoff/preguntas.md`.
- Nunca pidas permisos de administrador ni salir del sandbox para saltarte un bloqueo. Si un comando queda bloqueado, no lo reescribas de otra forma para esquivarlo: anótalo en `handoff/preguntas.md`.
- Para borrar un archivo de `app/` usa la herramienta de edición o `git rm` de ese archivo concreto, nunca un borrado recursivo.
- Detalle y motivo: `docs/09-seguridad.md`.
