# Sistema de tareas de Radar (ADR-004)

El proyecto se organiza con **una tarea por archivo** en `tasks/T-*.md`. Pensado para que
puedan trabajar varias personas y/o agentes a la vez sin pisarse: cada tarea tiene un único
`asignado` (una persona, p. ej. `ana`, o un agente, p. ej. `agent:vibe`) y dependencias
explícitas que impiden empezar antes de tiempo.

## Formato de una tarea

```yaml
---
id: T-004
titulo: "Fuente Overpass: scan por municipio y sector"
estado: backlog            # backlog | ready | in_progress | blocked | done
fase: F1                   # fase del roadmap (F0..F5) o F-infra
asignado: unassigned       # persona o agent:<nombre>
depends: [T-003]           # tareas que deben estar en done antes de empezar
criterios:
  - "radar scan --municipio Pamplona --sector hosteleria inserta negocios en SQLite"
---
Descripción libre, notas, decisiones, enlaces...
```

## Ciclo de vida

```
backlog --(deps en done)--> ready --(claim)--> in_progress --(done)--> done
                              ^                    |
                              |----(block/release)--+
```

- `backlog → ready`: automático con `scripts/tasks.sh ready` cuando todas las `depends` están `done`.
- `ready → in_progress`: con `scripts/tasks.sh claim T-XXX <asignado>` (pone tu nombre/agente).
- `in_progress → done`: con `scripts/tasks.sh done T-XXX`.
- Si te atascas: `scripts/tasks.sh block T-XXX`; para soltarla: `release T-XXX`.

## Comandos

```bash
scripts/tasks.sh list                          # todas las tareas
scripts/tasks.sh list --estado ready           # las que puedes coger ya
scripts/tasks.sh list --asignado agent:vibe   # mis tareas
scripts/tasks.sh ready                         # promueve backlog->ready si procede
scripts/tasks.sh show T-004
scripts/tasks.sh new "Título de la tarea" --fase F2 --dep T-008 --asignado agent:x
scripts/tasks.sh claim T-004 agent:vibe
scripts/tasks.sh done T-004
scripts/tasks.sh validate                      # reglas de coherencia (ids, deps, ciclos...)
scripts/tasks.sh board                         # regenera tasks/board.md
```

## Reglas para trabajar en paralelo

1. Antes de empezar una tarea, ejecuta `claim`. Si está `in_progress` de otra persona/agente, no la toques.
2. Una tarea por persona/agente a la vez; si bloqueas, marca `blocked` y coge otra `ready`.
3. Al terminar, `done` + commit con mensaje `T-XXX: ...` y ejecuta `validate` y `board`.
4. Si una tarea grande se puede partir, crea subtareas con `new` y añádelas como `depends`.
5. El contrato de la API (`api/openapi.yaml`) es generado: nunca se edita a mano; se regenera con
   `cargo run -p radar-cli -- api-spec > api/openapi.yaml` y el cliente TS con `npm run gen:api`.
