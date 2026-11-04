# ADR-004 — Sistema de tareas y asignación multi-persona / multi-agente

**Estado:** aceptado · **Fecha:** 04/11/2026

## Contexto

Varias personas y/o agentes van a trabajar en el proyecto a la vez. Hace falta un sistema
de tareas visible en el repo, con asignación y dependencias, sin depender de servicios
externos (presupuesto 0 €).

## Decisión

- Una tarea = un archivo `tasks/T-*.md` con frontmatter (id, titulo, estado, fase,
  asignado, depends, criterios de aceptación) + descripción libre.
- Estados: `backlog → ready → in_progress → done`, con `blocked` como estado lateral.
- `asignado` es único por tarea: una persona (`ana`) o un agente (`agent:vibe`).
- `scripts/tasks.sh` gestiona todo: `list/ready/show/new/claim/release/block/done/validate/board`.
- `tasks/board.md` es la vista Kanban generada (no se edita a mano).
- `validate` comprueba: ids únicos, estados válidos, dependencias existentes, orden
  (ready/in_progress/done exigen deps en done), in_progress con asignado, y **sin ciclos**.
- Convenciones de commit: `T-XXX: ...`.

## Consecuencias

- Paralelismo real: cada cual coge tareas `ready` con `claim` y nadie pisa otra tarea.
- El estado del proyecto es auditable con `git log` + el board.
- Coste cero y funciona offline.
