#!/usr/bin/env python3
"""Mide fuentes reales; --db ejecuta también el binario de producción sobre una copia.

Solo usa la biblioteca estándar. No importa resultados en la cartera original.
"""

import argparse
import concurrent.futures
import datetime
import hashlib
import json
import pathlib
import shutil
import sqlite3
import subprocess
import tempfile
import unicodedata
import urllib.parse
import urllib.request


PORTAL = "https://datosabiertos.navarra.es"
DENA = "directorio-de-establecimientos-con-actividad-economica-en-navarra"
DATASETS = [
    DENA,
    "restaurantes-inscritos-en-el-registro-de-turismo-de-navarra",
    "alojamientos-inscritos-en-el-registro-de-turismo-de-navarra",
    "empresas-de-actividades-inscritas-en-el-registro-de-turismo-de-navarra",
    "registro-industrial-de-navarra-empresas-divisi-n-a",
    "registro-industrial-de-navarra-empresas-divisi-n-b",
    "registro-industrial-de-navarra-empresas-divisi-n-c",
    "datos-de-cooperativas-vigentes-inscritas-en-el-registro-de-cooperativas-de-navarra",
    "registro-de-entidades-deportivas-y-federaciones-en-navarra",
    "registro-de-asociaciones-juveniles-en-navarra",
]
SOURCE_NOTES = {
    "datos-de-cooperativas-vigentes-inscritas-en-el-registro-de-cooperativas-de-navarra":
        "Cooperativas de distintas actividades. Conservar la clase cooperativa y clasificar también su actividad; no limitarse a cooperativas industriales.",
    "registro-de-entidades-deportivas-y-federaciones-en-navarra":
        "Entidades y federaciones; comprobar vigencia y actividad local antes de incorporar. No todas son empresas mercantiles.",
    "registro-de-asociaciones-juveniles-en-navarra":
        "Dataset histórico 2018–2021, actualizado en 2021. Solo sirve como pista: contrastar vigencia, bajas y ubicación con una fuente reciente antes de incorporar.",
}


def api(action, **params):
    url = PORTAL + "/api/3/action/" + action + "?" + urllib.parse.urlencode(params)
    request = urllib.request.Request(url, headers={"User-Agent": "RadarNavarra/coverage-probe"})
    with urllib.request.urlopen(request, timeout=30) as response:
        data = json.load(response)
    if not data.get("success"):
        raise RuntimeError(f"{action}: {data.get('error')}")
    return data["result"]


def source(slug):
    package = api("package_show", id=slug)
    resource = next(r for r in package["resources"] if r.get("datastore_active"))
    data = api("datastore_search", resource_id=resource["id"], limit=0)
    return {
        "dataset": slug,
        "url": PORTAL + "/dataset/" + slug,
        "licencia": package.get("license_id"),
        "recurso": resource["id"],
        "actualizado": resource.get("last_modified"),
        "registros_sin_filtrar": data["total"],
        "campos": [field["id"] for field in data["fields"]],
        "nota_cobertura": SOURCE_NOTES.get(slug),
    }


def normalized(value):
    return "".join(
        c for c in unicodedata.normalize("NFD", value or "").casefold()
        if c.isalnum() and not unicodedata.combining(c)
    )


def dena_profile(resource, year, names):
    if year is None:
        latest = api("datastore_search", resource_id=resource, fields="anio", sort="anio desc", limit=1)
        year = int(latest["records"][0]["anio"])
    rows = []
    ids = set()
    total = None
    while total is None or len(rows) < total:
        page = api(
            "datastore_search", resource_id=resource,
            filters=json.dumps({"anio": year}),
            fields="_id,anio,nombre,nombre_cmun,codpost,cnae09_local,cnae09_descrip_local",
            sort="_id asc", limit=10000, offset=len(rows),
        )
        if total is not None and total != page["total"]:
            raise RuntimeError("DENA ha cambiado durante la lectura; repetir la prueba")
        total = page["total"]
        if not page["records"] and len(rows) < total:
            raise RuntimeError("Paginación incompleta en DENA")
        for row in page["records"]:
            if row["_id"] in ids:
                raise RuntimeError("Registro repetido durante la paginación de DENA")
            ids.add(row["_id"])
        rows.extend(page["records"])
    counts = {}
    for row in rows:
        code = str(row.get("cnae09_local") or "").removesuffix(".0")
        counts[code] = counts.get(code, 0) + 1
    return {
        "anio": year, "registros": len(rows),
        "con_municipio": sum(bool(row.get("nombre_cmun")) for row in rows),
        "sin_descripcion_cnae": sum(not row.get("cnae09_descrip_local") for row in rows),
        "por_codigo_original": dict(sorted(counts.items())),
        "busquedas_nombre": {
            name: [{key: row.get(key) for key in ("nombre", "nombre_cmun", "cnae09_local", "cnae09_descrip_local")}
                   for row in rows if normalized(name) in normalized(row.get("nombre"))]
            for name in names
        },
        "nota": "Registros de establecimientos del año indicado; pendientes de deduplicar y clasificar. Los códigos y descripciones originales pueden discrepar.",
    }


def fingerprint(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def inventory(path):
    with sqlite3.connect(path) as conn:
        return {
            "negocios": conn.execute("SELECT count(*) FROM negocios").fetchone()[0],
            "admisibles": conn.execute("SELECT count(*) FROM negocios WHERE admisible=1").fetchone()[0],
            "vinculos_fuente": conn.execute("SELECT count(*) FROM negocio_fuentes").fetchone()[0],
        }


def production_probe(db, binary, output):
    db = db.resolve(strict=True)
    binary = binary.resolve(strict=True)
    snapshot = output / "scan.sqlite"
    with sqlite3.connect(db.as_uri() + "?mode=ro", uri=True) as original:
        with sqlite3.connect(snapshot) as copy:
            original.backup(copy)
    pbf = pathlib.Path(str(db) + ".navarra.osm.pbf")
    if not pbf.exists():
        raise RuntimeError("Falta el extracto OSM de producción; se necesita para comparar el mismo contenido")
    cached = pathlib.Path(str(snapshot) + ".navarra.osm.pbf")
    shutil.copyfile(pbf, cached)
    result = {"binario_sha256": fingerprint(binary), "pbf_sha256": fingerprint(pbf), "antes": inventory(snapshot), "ejecuciones": []}
    for i in (1, 2):
        command = [str(binary), "--db", str(snapshot), "scan", "--navarra", "--sector", "todos"]
        print(f"OSM: ejecución {i}/2 con el binario desplegado", flush=True)
        with (output / f"osm-{i}.json").open("w") as stdout, (output / f"osm-{i}.log").open("w") as stderr:
            subprocess.run(command, stdout=stdout, stderr=stderr, check=True)
        result["ejecuciones"].append({"resumen": json.loads((output / f"osm-{i}.json").read_text()), "inventario": inventory(snapshot)})
    result["repeticion_sin_duplicados"] = result["ejecuciones"][0]["inventario"] == result["ejecuciones"][1]["inventario"]
    if fingerprint(cached) != result["pbf_sha256"]:
        raise RuntimeError("El extracto cambió durante la prueba; los resultados no son comparables")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--db", type=pathlib.Path, help="Base de producción de la que crear una copia mediante SQLite backup")
    parser.add_argument("--radar", type=pathlib.Path, default=pathlib.Path("/usr/local/bin/radar"))
    parser.add_argument("--output", type=pathlib.Path)
    parser.add_argument("--year", type=int, help="Año DENA; por defecto, el último publicado")
    parser.add_argument("--nombre", action="append", default=[], help="Buscar un nombre normalizado en DENA; repetible")
    args = parser.parse_args()
    output = args.output or pathlib.Path(tempfile.mkdtemp(prefix="radar-cobertura-"))
    output.mkdir(mode=0o700, parents=True, exist_ok=True)
    if (output / "scan.sqlite").exists():
        parser.error("La carpeta ya contiene una copia de una prueba anterior")
    report = {"consultado_utc": datetime.datetime.now(datetime.timezone.utc).isoformat()}
    if args.db:
        report["produccion"] = production_probe(args.db, args.radar, output)
    with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
        report["fuentes"] = list(pool.map(source, DATASETS))
    dena = next(s for s in report["fuentes"] if s["dataset"] == DENA)
    report["dena"] = dena_profile(dena["recurso"], args.year, args.nombre)
    path = output / "resultado.json"
    path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(f"DENA {report['dena']['anio']}: {report['dena']['registros']} registros. Resultado: {path}")
    if args.db and not report["produccion"]["repeticion_sin_duplicados"]:
        raise SystemExit("La repetición modificó el número de negocios o vínculos; revisar resultado.json")


if __name__ == "__main__":
    main()
