# 09 · Seguridad del equipo: qué puede tocar cada agente

Objetivo: que ni Antigravity ni opencode puedan borrar ni cambiar nada fuera del proyecto, ni romper Windows ni otros programas. Investigado y probado el 27/11/2026.

## 1. El riesgo real

- **Ha pasado.** En diciembre de 2025, Antigravity en modo Turbo ejecutó `rmdir /s /q d:\` al «limpiar la caché» de un proyecto y borró el disco D: entero (MIT AI Incident Database n.º 1433). En el foro de Google hay otro caso de un disco C: borrado (julio de 2026, sin confirmar). opencode borró la carpeta Descargas de un usuario con `rm -rf` pese a tener `external_directory: deny` (issue #16126).
- **Lo que ya protege este PC** (medido el 27/11/2026): tu cuenta es administradora con UAC, así que los programas que lanzas corren en nivel de integridad medio y el grupo Administradores está en modo «solo denegar». Un agente **no puede** escribir en `C:\Windows`, `C:\Program Files` ni en el registro de la máquina sin que salga el aviso de UAC.
- **Lo que no protege:** todo lo que tu usuario puede escribir. Eso incluye tu perfil (`C:\Users\crbra`, con `.claude`, `.gemini`, documentos y la configuración de tus programas) y todo el disco E:. Ese es el riesgo de verdad: tus datos y la configuración de tus programas.
- **Lo que dicen los expertos:** mínimo privilegio, aislamiento en el sistema operativo y aprobación humana para lo destructivo (Microsoft, guía de seguridad de agentes de junio de 2026; Build Fast with AI; Cosmonic). Las reglas escritas en un prompt son la capa más débil: un modelo puede saltárselas. Por eso aquí se apoya todo en **controles que el programa aplica** (sandbox, permisos y listas de comandos), y las reglas de `AGENTS.md` y `.agents/rules/` son solo la última capa.

## 2. Las capas

| Capa | Qué impide | Dónde | Estado |
| --- | --- | --- | --- |
| 1. UAC | Cambiar Windows, Program Files y el registro de la máquina | Windows | Activo y medido. Regla: **nunca aceptar un aviso de UAC** mientras trabaja un agente, y nunca abrir Antigravity, opencode ni la terminal «como administrador» |
| 2. Sandbox de terminal de Antigravity | Que los comandos escriban fuera del proyecto o vean tu perfil | Antigravity | **Descartado tras la fase 0 (27/11/2026).** Con el proyecto en E:, el sandbox también deja en solo lectura el propio proyecto (S2: «Access is denied»). No dejó permisos huérfanos (S3). Se usa el Plan B: permisos sin pantalla con lista (§3.1) |
| 3. Permisos de Antigravity | Archivos fuera del proyecto y comandos peligrosos | Antigravity, ajustes del proyecto | Pendiente de aplicar (usuario, fase 0, §3) |
| 4. Permisos de opencode | Cualquier comando que no esté en la lista; editar la configuración o `.git`; cargar otras skills; leer la web | `opencode.json` | **Aplicado y probado** el 27/11/2026 (§3.2) |
| 5. Git y copia de seguridad | Perder trabajo si algo sale mal | Repositorio y otro disco o la nube | Un commit por tarea (ya está). Copia y punto de restauración antes de la fase 1 (usuario) |
| 6. Reglas y skills | Recordar al modelo lo que no debe hacer | `AGENTS.md` §7, `.agents/rules/seguridad.md` | Escritas. Capa más débil |

## 3. Configuración

### 3.1 Antigravity (la aplica el usuario en la fase 0)

Solo en el proyecto, para no cambiar tus otros proyectos: **Settings › Projects › Alpha › sobrescribir la configuración global**.

- **Permission preset:** `Default` (sandbox activado; lo que necesite salir del sandbox pide aprobación). **Nunca `Turbo` ni «Always Proceed»**: es el modo del incidente del disco D:.
- **Outside of folders file access** (`nonWorkspaceFileAccessPolicy`): **Deny**.
- **Browser JavaScript execution:** `Request Review`. `qa-app` lo necesita, y así cada vez te pide permiso.
- **Reglas `deny`.** Prevalecen sobre `ask` y `allow`, y se comparan por el inicio del comando, así que son un refuerzo, no la frontera:
  - archivos: `write_file(E:/Proyectos/Gestior de Tokens/Alpha/docs)`, `write_file(E:/Proyectos/Gestior de Tokens/Alpha/.agents)`, `write_file(E:/Proyectos/Gestior de Tokens/Alpha/.opencode)`, `write_file(E:/Proyectos/Gestior de Tokens/Alpha/.git)`, `write_file(E:/Proyectos/Gestior de Tokens/Alpha/AGENTS.md)`, `write_file(E:/Proyectos/Gestior de Tokens/Alpha/opencode.json)`;
  - comandos que borran en masa: `command(rmdir /s)`, `command(rd /s)`, `command(del /s)`, `command(Remove-Item -Recurse)`, `command(rm -r)`, `command(format)`, `command(diskpart)`;
  - comandos que cambian el sistema: `command(reg )`, `command(bcdedit)`, `command(Set-ExecutionPolicy)`, `command(takeown)`, `command(icacls)`, `command(shutdown)`, `command(Stop-Computer)`, `command(Restart-Computer)`;
  - comandos que instalan programas: `command(winget)`, `command(choco)`, `command(npm install -g)`, `command(npm i -g)`, `command(cargo install)`, `command(rustup)`;
  - git: `command(git push)`, `command(git reset --hard)`, `command(git clean)`, `command(regex:.*--no-verify.*)`.
- **Plan B**, si el sandbox no supera la prueba de §4: preset `Request Review`, que pide aprobación para cada comando. Se permiten sin preguntar solo `command(cargo test)`, `command(cargo clippy)`, `command(cargo fmt)`, `command(npm test)`, `command(npx tsc --noEmit)` y `command(git status)`. Una regla `allow` deja ejecutar el comando fuera del sandbox, así que no se añade ninguna más.

**Cuando Claude lanza la tarea desde la consola** (`02-agentes-y-modelos.md` §4.3), los permisos van en `.agents/agy-permisos.json` y se cargan solo para esa sesión, con `--settings`; tu configuración global no se toca. La lista `allow` usa expresiones regulares que no admiten `;`, `&`, `|`, `` ` ``, `$`, `<` ni `>`, así que no se pueden encadenar ni redirigir comandos. **Probado el 27/11/2026** con `agy` 1.5.18 en modo `-p --mode accept-edits`:

| Prueba | Resultado |
| --- | --- |
| `git status --short` (en la lista) | Ejecutado |
| `git log … --no-verify` | Bloqueado por la regla `deny` |
| `cargo --version` y `Get-Date` (fuera de la lista) | Denegados (`denied_actions`); la conversación se para |
| `git status; Get-Date` (encadenado) | Denegado |
| Escribir `E:\Proyectos\Gestior de Tokens\prueba-fuera-agy.txt` | Bloqueado por `agy`: «permission check failed for write_file» |
| Escribir `docs/prueba-agy.md` | No escrito: lo rechazó el modelo citando `AGENTS.md` antes de llegar a la regla `deny` |
| El mismo `Get-Date` con `--mode auto` | **Ejecutado sin estar en la lista**: por eso `auto` no se usa |

Las reglas exigen coincidencia exacta: `command(git status)` no cubre `git status --short`. Por eso la lista usa `regex:`.

Los subagentes tienen `commandExecutionPolicy: auto`: ejecutan comandos sin preguntarte. Por eso estos límites tienen que funcionar también con ellos, y la fase 0 lo comprueba (§4, S4).

### 3.2 opencode (`opencode.json`, aplicado)

opencode **no tiene sandbox de sistema operativo** en Windows, y `external_directory` no frena las escrituras hechas desde la terminal. El issue #46398 lo documenta, se cerró sin arreglo en la versión 1 y la corrección (#56390) solo llega a la versión 2. Por eso:

- **`bash`: todo denegado salvo una lista.** La lista es: `cd app…`, `cd src-tauri…`, `git status/diff/log/show/rev-parse`, `git add` solo de `app/`, `src/`, `src-tauri/` y `handoff/`, `git commit -m …`, `cargo test/clippy/fmt/check/build/tree` y `npm test`, `npm run test`, `npx tsc --noEmit`. Al final se deniegan `--no-verify`, `--amend` y `--force`, aunque vayan dentro de un comando permitido (gana la última regla que coincide).
- **`edit`:** denegado en `docs/`, `AGENTS.md`, `.agents/`, `.opencode/`, `opencode.json`, `.git/` y `.gitignore`.
- **`skill`:** solo `ejecutar-tarea-tdd`, `mockup-a-react` y `cerrar-fase`. opencode también carga por su cuenta las skills personales de `%USERPROFILE%\.claude\skills` (xlsx, docx y otras), y esta regla las oculta.
- **`external_directory`: deny**, y **`webfetch`: deny**.

**Probado el 27/11/2026** en una carpeta aislada con opencode 1.19.6 y `deepseek-v4.1-flash-free`:

| Comando o acción | Resultado |
| --- | --- |
| `git status` | Permitido |
| `New-Item -ItemType File dentro.txt` | Bloqueado por `bash` |
| `git status && New-Item …` | Bloqueado: opencode separa los comandos encadenados y comprueba cada uno |
| `New-Item -ItemType File ..\fuera2.txt` | Bloqueado por `external_directory` |
| `git commit --no-verify -m prueba` | Bloqueado: gana el `deny` final |
| Cargar la skill `xlsx` | Bloqueado; el agente solo ve las skills permitidas |
| Archivos fuera de la carpeta del proyecto | Ninguno creado |

Además, `npm install` no está en la lista: así opencode no puede añadir dependencias, que `AGENTS.md` ya prohíbe. Antes de T13, el usuario ejecuta `npm install` en la copia de trabajo (`01-fases.md`, fase 5).

### 3.3 Antes de la fase 1 (usuario)

- **Punto de restauración de Windows:** `Checkpoint-Computer -Description "Antes de Gestor de Tokens"` en una PowerShell de administrador, abierta por ti y no por un agente.
- **Copia de seguridad** de `E:\Proyectos` en otro sitio (disco externo o la nube). En este PC solo hay C: y E:, así que una copia dentro de E: no protege de un borrado de E:.

## 4. Pruebas de seguridad de la fase 0

Se hacen en este orden y el resultado se anota en `handoff/fase-0-informe.md`:

- [ ] **S1. Permisos de partida.** Ejecutar el script de S3 antes de activar el sandbox. El 27/11/2026 dio 0 entradas de AppContainer y 0 perfiles `exebox` en el perfil, `.claude`, `.gemini`, `AppData\Local` y `E:\Proyectos`.
- [ ] **S2. El sandbox contiene los comandos.** Con la configuración de §3.1, pedir al Orchestrator que ejecute esto y anote el resultado de cada comando:
  - `cargo --version` y `npm --version`: deben funcionar;
  - `New-Item -ItemType File "E:\Proyectos\Gestior de Tokens\fuera-sandbox.txt"`: **debe fallar**;
  - `New-Item -ItemType File handoff\dentro-sandbox.txt`: debe funcionar, y luego se borra;
  - `cargo new handoff\sandbox-hola` seguido de `cargo test --manifest-path handoff\sandbox-hola\Cargo.toml`: anotar si compila dentro del sandbox. El sandbox oculta tu perfil, donde están `.cargo` y `.rustup`. Luego borrar `handoff\sandbox-hola`.
- [ ] **S3. El sandbox no deja permisos huérfanos.** El bug #1323 de `antigravity-cli` sigue abierto: el sandbox de Windows deja entradas de control total para su AppContainer y perfiles `exebox-sandbox-*`, y eso rompe apps Electron como Discord. Ocurre cuando la carpeta de trabajo está dentro del perfil. La nuestra está en E:, así que probablemente no pase, pero no está comprobado. Después de S2, el usuario ejecuta en PowerShell:
  ```powershell
  foreach ($d in "$env:USERPROFILE","$env:LOCALAPPDATA","$env:USERPROFILE\.claude","$env:USERPROFILE\.gemini","E:\Proyectos","E:\Proyectos\Gestior de Tokens\Alpha") { $s=(Get-Acl $d).Sddl; "{0}: {1}" -f $d,([regex]::Matches($s,'S-1-15-2-\d+(-\d+){5,}')).Count }
  (Get-ChildItem 'HKCU:\Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppContainer\Mappings' | ForEach-Object { (Get-ItemProperty $_.PSPath).Moniker } | Where-Object { $_ -like 'exebox*' }).Count
  ```
  Todo debe dar 0. Si algo no da 0: desactivar el sandbox, pasar al Plan B (§3.1) y avisar a Claude. Claude da los pasos de limpieza del issue #1323: `icacls <carpeta> /remove:g *S-1-15-2-…` y después `icacls %USERPROFILE% /inheritance:r`. No se improvisa.
- [ ] **S4. Las reglas se aplican también a los subagentes.** Pedir a `verificador` que ejecute `git clean -n`, que solo muestra lo que borraría. Debe quedar bloqueado por la regla `deny`.
- [ ] **S5. opencode en el repositorio real.** En `E:\Proyectos\Gestior de Tokens\Alpha`, ejecutar `opencode run --agent verificador 'Ejecuta con bash: New-Item -ItemType File ..\fuera.txt; y dime qué skills tienes'`. El comando debe quedar bloqueado y el agente solo debe ver las 3 skills. Comprobar que no existe `E:\Proyectos\Gestior de Tokens\fuera.txt`.

## 5. Cómo lo gestiona y lo valida Claude

Claude escribe y mantiene esta configuración y las reglas: `opencode.json`, `.opencode/`, `.agents/`, `AGENTS.md` y este documento. **No ejecuta código de la app ni controla Antigravity en directo.** Valida lo que queda en disco.

Al recibir `handoff/fase-N-informe.md`, el usuario pide «valida la seguridad de la fase N» y Claude comprueba, solo leyendo:

1. **Rutas tocadas.** `git diff --name-only <primer commit de la fase>~1..HEAD` solo puede contener `app/` y `handoff/`. Cualquier otra ruta y la fase no se acepta.
2. **La configuración no cambió.** `git diff <…>..HEAD -- opencode.json .opencode .agents AGENTS.md docs .gitignore` debe salir vacío.
3. **Nada raro en git.** Ningún archivo en `.git/hooks` que no sea `*.sample`; un commit por tarea con el mensaje del plan; nada en `app/` que lea o escriba fuera de lo que dicen `03-arquitectura.md` y `06-colectores-y-costes.md`.
4. **Permisos huérfanos.** El usuario ejecuta el script de S3 y Claude compara con S1.
5. **Comandos bloqueados.** Se cuentan las veces que el informe o el usuario dicen que un comando fue bloqueado o pidió aprobación. Si se repiten, se ajusta la lista; no se abre de más.

Si algo falla, Claude lo anota en `docs/respuestas.md` y dice qué hacer (revertir el commit, restaurar la copia, cambiar un ajuste). Claude no arregla `app/`.

Claude Code también pide tu aprobación según tus ajustes de permisos, y Claude solo escribe en las rutas de la documentación (`AGENTS.md` §1).

## 6. Lo que sigue siendo un riesgo

- **El código que escribe el agente se ejecuta.** `cargo test` y `npm test` están permitidos y ejecutan las pruebas que escribe el propio agente. En opencode corren con tus permisos; en Antigravity, dentro del sandbox si supera la fase 0. Lo mitigan el `revisor`, Carbon, Claude y la copia de seguridad, pero no lo eliminan.
- **Las listas `deny` de Antigravity se comparan por el inicio del comando** y se pueden esquivar escribiéndolo de otra forma. La frontera es el sandbox o, en el Plan B, tu aprobación de cada comando.
- **Las propias herramientas tienen fallos** (#1323, #46398). Por eso se prueban en la fase 0 y Claude vuelve a revisar los avisos al cambiar de versión.
- **Opción más fuerte, no adoptada:** una cuenta de Windows estándar solo para los agentes, que Microsoft recomienda para separar la identidad de un agente, o una máquina virtual. Aísla de verdad tus datos, pero obliga a instalar Rust, Node y opencode otra vez para esa cuenta. Si quieres este nivel, Claude lo documenta como paso de la fase 0.

## 7. Fuentes

- Incidentes: [MIT AI Incident Database n.º 1433](https://airisk.mit.edu/ai-incidents/1433-google-antigravity-reportedly-deleted-user-s-entire-d) · [How-To Geek](https://www.howtogeek.com/google-antigravity-ide-deleted-someones-entire-drive/) · [foro de Google: disco C: borrado](https://discuss.ai.google.dev/t/critical-bug-antigravity-ai-agent-wiped-my-entire-hard-drive-c/175715) · [opencode #16126](https://github.com/anomalyco/opencode/issues/16126).
- Antigravity: [Terminal sandbox (documentación oficial)](https://antigravity.google/docs/sandbox/) · [Mete Atamel: Agent Permissions in Antigravity](https://atamel.dev/posts/2026/09-14_agent_permissions_agy/) · [CybeDefend: qué bloquea el sandbox](https://www.cybedefend.com/en/blog/is-google-antigravity-safe) · [#1323](https://github.com/google-antigravity/antigravity-cli/issues/1323) · [#1161](https://github.com/google-antigravity/antigravity-cli/issues/1161) · [foro: AppContainer huérfanos](https://discuss.ai.google.dev/t/critical-bug-gemini-in-sandbox-fails-to-run-or-acess-env-variables-orphaned-appcontainer-ids-that-break-discord-antigravity-2-0-and-other-electron-apps/172028).
- opencode: [#46398](https://github.com/anomalyco/opencode/issues/46398) · [referencia de permisos](https://agenticcontrolplane.com/blog/opencode-permissions-reference) · [skills](https://opencode.ai/docs/skills/).
- Prácticas: [Windows platform security for AI agents (Microsoft, 2026)](https://blogs.windows.com/windowsdeveloper/2026/06/02/windows-platform-security-for-ai-agents/) · [Build Fast with AI: cómo asegurar agentes de código](https://blog.buildfastwithai.com/how-to-secure-ai-coding-agents-in-2026-permissions-sandboxing-mcp-secrets) · [Cosmonic: cómo elegir un sandbox](https://cosmonic.com/ai-sandbox/choosing-an-agent-sandbox/).
