#!/usr/bin/env bash
# Sistema de tareas de Radar (ADR-004): un archivo por tarea en tasks/T-*.md con
# frontmatter YAML (id, titulo, estado, fase, asignado, depends, criterios).
# Pensado para que trabajen varias personas o agentes a la vez: cada tarea tiene
# un único asignado (persona o agent:<nombre>) y dependencias explícitas.
#
# Uso:
#   scripts/tasks.sh list [--estado E] [--asignado A] [--fase F]
#   scripts/tasks.sh ready            # promueve backlog -> ready cuando las deps están done
#   scripts/tasks.sh show T-004
#   scripts/tasks.sh new "Título" --fase F2 [--dep T-003] [--asignado agent:x]
#   scripts/tasks.sh claim T-004 agent:ana
#   scripts/tasks.sh release T-004
#   scripts/tasks.sh block T-004
#   scripts/tasks.sh done T-004
#   scripts/tasks.sh validate         # reglas: ids únicos, deps existen y en orden, sin ciclos...
#   scripts/tasks.sh board            # regenera tasks/board.md
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TASKS_DIR="$ROOT/tasks"
ESTADOS="backlog ready in_progress blocked done"

fm() { awk 'BEGIN{c=0} /^---[[:space:]]*$/{c++; if(c==2) exit; next} c==1{print}' "$1"; }
campo() { fm "$1" | awk -v k="$2" -F':' 'tolower($1)==tolower(k){sub(/^[^:]*:[ ]*/,""); gsub(/^"|"$/,""); print; exit}'; }
deps_de() { local d; d="$(campo "$1" depends)"; d="${d#[}"; d="${d%]}"; echo "$d" | tr ',' ' ' | tr -s ' ' | sed 's/^ //; s/ $//'; }
archivo_de() { echo "$TASKS_DIR/$1.md"; }
set_campo() { # set_campo archivo clave valor (solo dentro del frontmatter)
  local f="$1" k="$2" v="$3"
  awk -v k="$k" -v v="$v" 'BEGIN{c=0} /^---[[:space:]]*$/{c++; print; next} c==1 && index(tolower($0), tolower(k)":")==1 {print k": "v; next} {print}' "$f" > "$f.tmp" && mv "$f.tmp" "$f"
}
todos() { ls "$TASKS_DIR"/T-*.md 2>/dev/null | sort; }
estado_de() { campo "$(archivo_de "$1")" estado; }

cmd="${1:-list}"; shift || true

case "$cmd" in
  list)
    est=""; asig=""; fase=""
    while [[ $# -gt 0 ]]; do case "$1" in
      --estado) est="$2"; shift 2;;
      --asignado) asig="$2"; shift 2;;
      --fase) fase="$2"; shift 2;;
      *) echo "filtro desconocido: $1" >&2; exit 2;;
    esac; done
    printf "%-7s %-12s %-14s %-4s %s\n" ID ESTADO ASIGNADO FASE TITULO
    for f in $(todos); do
      id=$(basename "$f" .md); e=$(campo "$f" estado); a=$(campo "$f" asignado); fa=$(campo "$f" fase); t=$(campo "$f" titulo)
      [[ -n "$est" && "$e" != "$est" ]] && continue
      [[ -n "$asig" && "$a" != "$asig" ]] && continue
      [[ -n "$fase" && "$fa" != "$fase" ]] && continue
      printf "%-7s %-12s %-14s %-4s %s\n" "$id" "$e" "$a" "$fa" "$t"
    done
    ;;
  ready)
    for f in $(todos); do
      id=$(basename "$f" .md); e=$(campo "$f" estado)
      [[ "$e" != "backlog" ]] && continue
      ok=1
      for d in $(deps_de "$f"); do [[ -z "$d" ]] && continue; [[ "$(estado_de "$d")" == "done" ]] || ok=0; done
      if [[ $ok == 1 ]]; then set_campo "$f" estado ready; echo "$id -> ready"; fi
    done
    ;;
  show)
    cat "$(archivo_de "$1")"
    ;;
  new)
    titulo=""; fase="F1"; deps=""; asig="unassigned"
    while [[ $# -gt 0 ]]; do case "$1" in
      --fase) fase="$2"; shift 2;;
      --dep) deps="$deps$2, "; shift 2;;
      --asignado) asig="$2"; shift 2;;
      *) titulo="$*"; break;;
    esac; done
    max=0; for f in $(todos); do id=$(basename "$f" .md); n=${id#T-}; (( 10#$n > max )) && max=$((10#$n)); done
    id=$(printf "T-%03d" $((max+1)))
    deps="[${deps%, }]"
    cat > "$(archivo_de "$id")" <<EOF
---
id: $id
titulo: "$titulo"
estado: backlog
fase: $fase
asignado: $asig
depends: $deps
criterios:
  - "(pendiente definir criterios de aceptación)"
---
Descripción de la tarea.
EOF
    echo "creada $id: $titulo"
    ;;
  claim)
    id="$1"; asig="$2"; f=$(archivo_de "$id"); e=$(campo "$f" estado)
    if [[ "$e" == "backlog" ]]; then
      ok=1; for d in $(deps_de "$f"); do [[ -z "$d" ]] && continue; [[ "$(estado_de "$d")" == "done" ]] || ok=0; done
      [[ $ok == 1 ]] || { echo "$id sigue en backlog: dependencias sin terminar" >&2; exit 1; }
      set_campo "$f" estado ready
    fi
    [[ "$e" == "ready" || "$e" == "blocked" ]] || { echo "$id no está ready (estado: $e)" >&2; exit 1; }
    set_campo "$f" estado in_progress; set_campo "$f" asignado "$asig"
    echo "$id asignada a $asig (in_progress)"
    ;;
  release)
    id="$1"; f=$(archivo_de "$id"); set_campo "$f" estado ready; set_campo "$f" asignado unassigned
    echo "$id liberada (ready, unassigned)"
    ;;
  block)
    id="$1"; f=$(archivo_de "$id"); set_campo "$f" estado blocked
    echo "$id bloqueada"
    ;;
  done)
    id="$1"; f=$(archivo_de "$id"); e=$(campo "$f" estado)
    [[ "$e" == "in_progress" || "$e" == "ready" ]] || { echo "$id no se puede completar desde $e" >&2; exit 1; }
    set_campo "$f" estado done; echo "$id -> done"
    ;;
  validate)
    errores=0
    declare -A vistos
    for f in $(todos); do
      id=$(basename "$f" .md)
      [[ -z "$(campo "$f" id)" ]] && { echo "ERROR $id: sin id"; errores=1; continue; }
      [[ "$(campo "$f" id)" == "$id" ]] || { echo "ERROR $id: id del frontmatter no coincide con el archivo"; errores=1; }
      [[ -n "${vistos[$id]:-}" ]] && { echo "ERROR: id duplicado $id"; errores=1; }
      vistos[$id]=1
      e=$(campo "$f" estado)
      [[ " $ESTADOS " == *" $e "* ]] || { echo "ERROR $id: estado '$e' no válido"; errores=1; }
      [[ -n "$(campo "$f" titulo)" ]] || { echo "ERROR $id: sin titulo"; errores=1; }
      [[ -n "$(campo "$f" fase)" ]] || { echo "ERROR $id: sin fase"; errores=1; }
      a=$(campo "$f" asignado); [[ -n "$a" ]] || { echo "ERROR $id: sin asignado"; errores=1; }
      if [[ "$e" == "in_progress" && "$a" == "unassigned" ]]; then echo "ERROR $id: in_progress sin asignado"; errores=1; fi
      for d in $(deps_de "$f"); do
        [[ -z "$d" ]] && continue
        [[ -f "$(archivo_de "$d")" ]] || { echo "ERROR $id: dependencia $d no existe"; errores=1; continue; }
        if [[ "$e" == "ready" || "$e" == "in_progress" || "$e" == "done" ]]; then
          [[ "$(estado_de "$d")" == "done" ]] || { echo "ERROR $id: $e pero dependencia $d está en $(estado_de "$d")"; errores=1; }
        fi
      done
    done
    # detección de ciclos (DFS)
    if ! python3 - "$TASKS_DIR" <<'PYEOF'
import sys, os, re
d = sys.argv[1]
grafo = {}
for fn in os.listdir(d):
    if not fn.endswith('.md'): continue
    txt = open(os.path.join(d, fn)).read()
    m = re.search(r'^---\n(.*?)\n---', txt, re.S | re.M)
    if not m: continue
    fid = re.search(r'^id:\s*(\S+)', m.group(1), re.M).group(1)
    dep = re.search(r'^depends:\s*\[(.*?)\]', m.group(1), re.M)
    deps = [x.strip() for x in dep.group(1).split(',') if x.strip()] if dep else []
    grafo[fid] = deps
estado = {}
def dfs(n, stack):
    estado[n] = 1
    for m in grafo.get(n, []):
        if estado.get(m) == 1:
            print(f"ERROR: ciclo detectado: {' -> '.join(stack + [n, m])}"); return False
        if estado.get(m, 0) == 0 and m in grafo:
            if not dfs(m, stack + [n]): return False
    estado[n] = 2
    return True
ok = True
for n in list(grafo):
    if estado.get(n, 0) == 0:
        ok = dfs(n, []) and ok
sys.exit(0 if ok else 1)
PYEOF
    then errores=1; fi
    if [[ $errores == 0 ]]; then echo "validate OK: $(todos | wc -l) tareas, sin errores"; else echo "validate: HAY ERRORES" >&2; exit 1; fi
    ;;
  board)
    out="$TASKS_DIR/board.md"
    {
      echo "# Board de tareas — Radar"
      echo ""
      echo "Generado con \`scripts/tasks.sh board\`. No editar a mano."
      echo ""
      for e in backlog ready in_progress blocked done; do
        echo "## $e"
        echo ""
        echo "| ID | Título | Asignado | Fase | Deps |"
        echo "|---|---|---|---|---|"
        for f in $(todos); do
          [[ "$(campo "$f" estado)" == "$e" ]] || continue
          id=$(basename "$f" .md)
          echo "| $id | $(campo "$f" titulo) | $(campo "$f" asignado) | $(campo "$f" fase) | $(deps_de "$f") |"
        done
        echo ""
      done
    } > "$out"
    echo "board regenerado en $out"
    ;;
  *)
    echo "comando desconocido: $cmd" >&2; exit 2;;
esac
