# Cobertura de negocios: pruebas reales y fuentes adicionales

Fecha de consulta: 5 de noviembre de 2026. Ámbito: toda Navarra. Las consultas a
fuentes nuevas son una investigación; todavía no están integradas en el escaneo
de producción. No se han contratado servicios ni configurado facturación.

## Alcance ampliado: asociaciones, ONGs y cooperativas

Por indicación del usuario se incluyen también asociaciones culturales, vecinales,
juveniles, deportivas y profesionales; ONGs y entidades de voluntariado;
fundaciones vinculadas a estas actividades; y cooperativas agrarias, de trabajo,
consumo, servicios y otros ámbitos. La captación debe valorar entidades con
actividad y ubicación en Navarra que puedan necesitar servicios web, aunque su
actividad no persiga beneficio mercantil.

El tipo de entidad debe distinguirse de su rubro. Una cooperativa puede mantener
el rubro bodega, agricultura o servicios; una ONG puede estar constituida como
asociación o fundación. Se propone conservar naturaleza jurídica y perfiles de
actividad por separado, con evidencias, para permitir filtros que se solapen.
La ausencia de información se conserva como desconocida.

La revisión de la base de producción encuentra **nueve registros OSM de categoría
`association` y cinco de categoría `ngo` excluidos** como
`sin_actividad_comercial_demostrada`. El filtro de `sector_de` descarta expresamente
esas oficinas. Antes de la siguiente integración hay que ampliar ese criterio:
la falta de ánimo de lucro no debe ser causa de exclusión. Sigue siendo necesario
verificar identidad, actividad, ubicación y cierres; una inscripción antigua no
demuestra actividad actual. El reconocimiento de grandes redes también debe
revisarse para valorar la autonomía web de sus delegaciones.

Fuentes adicionales comprobadas el 5 de noviembre de 2026:

| Fuente | Filas de origen | Estado de la comprobación |
| --- | ---: | --- |
| [Cooperativas vigentes de Navarra](https://datosabiertos.navarra.es/dataset/datos-de-cooperativas-vigentes-inscritas-en-el-registro-de-cooperativas-de-navarra) | **1.578** | API CKAN consultada; CC BY 4.0; recurso actualizado el 5/11/2026; denominación, inscripción, domicilio, localidad, clase y CNAE |
| [Entidades deportivas y federaciones](https://datosabiertos.navarra.es/dataset/registro-de-entidades-deportivas-y-federaciones-en-navarra) | **2.209** | API CKAN consultada; CC BY 4.0; última actualización del recurso 6/8/2025; contiene dirección, localidad y web |
| [Asociaciones juveniles, 2018–2021](https://datosabiertos.navarra.es/dataset/registro-de-asociaciones-juveniles-en-navarra) | **122** | API CKAN consultada; CC BY 4.0; recurso de 2021. Es histórico, no un censo actual; contrastar bajas y actividad reciente |
| [Mapa de entidades de Navarra + Voluntaria](https://navarramasvoluntaria.navarra.es/es/mapa-de-entidades) | Sin recuento validado | Fuente oficial de entidades y programas de voluntariado; falta comprobar exportación/API y condiciones de reutilización |
| [Coordinadora de ONGD de Navarra](https://congdnavarra.org/) | Sin recuento validado | Fuente de sus entidades socias; no representa todas las ONGs de Navarra; falta comprobar exportación y reutilización |
| [Registro de Asociaciones de Navarra](https://www.navarra.es/es/tramites/on/-/line/registro-de-asociaciones-) | Sin recuento validado | Referencia oficial para verificar identidad e inscripción; no se ha validado un API o descarga completa para captación |

Los recuentos son filas previas a deduplicar; varias fuentes pueden describir la
misma entidad. El script de prueba incorpora los tres conjuntos CKAN e identifica
el de asociaciones juveniles como histórico. No se han incorporado sus filas a
producción ni se ha modificado todavía la política de admisión desplegada.

La prioridad propuesta para estas entidades se apoya en necesidades observables:
información sobre proyectos y servicios, agenda e inscripciones, altas de socios,
voluntariado, donaciones, accesibilidad, transparencia y mantenimiento de contenidos.
Ni recibir subvenciones ni tener una forma jurídica concreta demuestra por sí solo
presupuesto disponible o intención de contratar.

Aceptación adicional: una asociación o una ONG activa con ubicación confirmada
no se excluye por ser no lucrativa; una cooperativa conserva su actividad y su
tipo de entidad; una ONG constituida como asociación aparece en ambos filtros;
las entidades dadas de baja y los registros históricos sin corroborar quedan
pendientes o excluidos según la evidencia, y repetir la captura no los duplica.

## Prueba con el escaneo desplegado

Se ha ejecutado dos veces `/usr/local/bin/radar scan --navarra --sector todos`
sobre una copia obtenida mediante SQLite backup de la base de producción. Se ha
copiado también su extracto PBF y comprobado su SHA-256. Las consultas usan el
mismo binario y `scan_navarra_resumen` que invoca `POST /api/jobs/scan` desde la web.
La base original no se modifica.

| Medida | Antes | Primera búsqueda | Repetición |
| --- | ---: | ---: | ---: |
| Negocios únicos en cartera | 5.214 | 5.214 | 5.214 |
| Negocios admisibles | 5.214 | 5.214 | 5.214 |
| Vínculos con registros de la fuente | 5.222 | 5.222 | 5.222 |

Cada ejecución examina 14.239 candidatos OSM: admite 5.222 registros, excluye
8.447 sin actividad comercial demostrada y 329 por red comercial grande, y deja
241 pendientes. La repetición no duplica negocios ni vínculos.

**Corrección del recuento anterior:** 5.222 son registros admitidos de OSM, antes
de unir coincidencias. No representan altas nuevas ni negocios únicos. El
resumen actual declara ocho `nuevos` en cada repetición aunque la cartera no
crece: ese contador comprueba la identidad principal del negocio en lugar de
consultar el resultado de la vinculación de fuentes. Debe corregirse antes de
usarlo como indicador de crecimiento.

La búsqueda regional actual lee el extracto OSM de Geofabrik; los escaneos
municipales usan Overpass. BORME cubre constituciones, no un censo de negocios.
El registro de restaurantes se consulta para enriquecer negocios existentes,
sin importar sus establecimientos como candidatos nuevos. El buscador Tavily
está implementado para localizar webs de negocios conocidos, pero su clave no
está configurada en este despliegue.

## Fuentes verificadas con sus APIs reales

Los recuentos siguientes son filas de origen, antes de filtrar elegibilidad y
deduplicar establecimientos. No se deben sumar ni presentar como futuras altas.
Todos estos conjuntos del portal navarro declaran CC BY 4.0.

| Fuente | Registros consultados | Aportación |
| --- | ---: | --- |
| [DENA: establecimientos](https://datosabiertos.navarra.es/dataset/directorio-de-establecimientos-con-actividad-economica-en-navarra) | **45.978 del año 2026** | Base general de establecimientos, con denominación, municipio, dirección y actividad |
| [Registro industrial A](https://datosabiertos.navarra.es/dataset/registro-industrial-de-navarra-empresas-divisi-n-a) | 5.831 | Industria, talleres y producción |
| [Registro industrial B](https://datosabiertos.navarra.es/dataset/registro-industrial-de-navarra-empresas-divisi-n-b) | 7.052 | Instaladores y servicios industriales habilitados |
| [Registro industrial C](https://datosabiertos.navarra.es/dataset/registro-industrial-de-navarra-empresas-divisi-n-c) | 10 | Entidades de control y otras actividades específicas |
| [Restaurantes](https://datosabiertos.navarra.es/dataset/restaurantes-inscritos-en-el-registro-de-turismo-de-navarra) | 553 | Restauración registrada |
| [Alojamientos](https://datosabiertos.navarra.es/dataset/alojamientos-inscritos-en-el-registro-de-turismo-de-navarra) | 2.846 | Casas rurales, apartamentos, hoteles, albergues y campings |
| [Turismo activo y cultural](https://datosabiertos.navarra.es/dataset/empresas-de-actividades-inscritas-en-el-registro-de-turismo-de-navarra) | 195 | Empresas de actividades y visitas |

Para obtener estas cifras se han consultado `package_show` y `datastore_search`
del API CKAN oficial. En DENA se han paginado las 45.978 filas del año 2026,
comprobando el total y que no se repitan identificadores de fila. Las 393.215
filas del recurso completo incluyen 2018–2026: importarlas todas multiplicaría
observaciones históricas de los mismos establecimientos.

En DENA 2026 hay 705 filas con código original `4321` (instalaciones eléctricas),
721 con `4322` (fontanería y climatización), 87 con `7500` (veterinaria) y 124 con
`1102` (elaboración de vinos). El OSM actual solo aporta ocho negocios con rubro
electricidad y ocho con rubro fontanería. Las unidades y taxonomías son distintas,
pero la diferencia justifica el piloto de cobertura.

DENA tiene actualización anual, excluye algunas actividades —incluida la sección
agraria— y suele aportar la razón social en vez del nombre del local. Tiene
13.596 filas de 2026 sin descripción CNAE; algunos códigos y descripciones
requieren contrastar la versión de la clasificación. La fila de Lainez Automoción,
por ejemplo, figura con una descripción de reparación de efectos personales.
No debe asignarse un rubro definitivo a partir de esa descripción sin contraste.
Tampoco aporta coordenadas o datos de contacto web en este recurso.

El registro industrial puede repetir una empresa por habilitación. Hay que
verificar su ubicación y unir establecimientos coincidentes, conservando cada
habilitación como evidencia. Los registros de turismo pueden coincidir con DENA
y OSM. La inscripción turística no demuestra una tarifa concreta.

## Comprobación de los nombres aportados

| Objetivo | Resultado de la investigación |
| --- | --- |
| Lainez Automoción | DENA 2026: `LAINEZ AUTOMOCION SL`, Murchante. No aparece entre los candidatos OSM examinados con ese nombre. |
| Bodegas Ontañón | DENA 2026: `BODEGAS ONTANON SL`, Fitero. También consta en el directorio municipal. |
| San Raimundo | DENA 2026: `SOCIEDAD COOPERATIVA SAN RAIMUNDO ABAD DE FITERO`. El directorio municipal la lista por separado de Ontañón y describe actividad agrícola. Debe tratarse como una identidad separada y verificar su rubro actual. |
| M Sanco | El [directorio municipal de Fitero](https://fitero.es/empresas-locales/) publica `SANCO, S.L.`, dedicada a madera y derivados. Es una candidata; el nombre abreviado requiere confirmar la identidad. |
| El Rincón del Torero | Ya está en producción como `Rincón del Torero`, en Buñuel. El cruce exige igualdad del nombre y no lo vincula al objetivo con el artículo “El”. |
| Castillo de Monteagudo | OSM lo recoge como `Castillo Palacio de Monteagudo`, excluido por falta de actividad comercial demostrada. Hace falta una fuente del operador que confirme visitas o actividad comercial. |
| La Taska de Barillas | OSM contiene una `La Taska` en otra ubicación. Esa coincidencia no confirma el objetivo de Barillas. |
| Castillo de GaskonciKo | Sin identificación fiable en esta investigación; conservar pendiente. |

La lista inicial reúne Ontañón y San Raimundo en una fila. El siguiente cambio
debe corregir esa identificación y permitir candidatos por variantes del nombre,
manteniendo la comprobación de ubicación y evidencia antes de confirmar una unión.

## Otras fuentes adecuadas para un siguiente piloto

- [Overture Maps Places](https://docs.overturemaps.org/guides/places/): descarga de
  lugares por zona, con coordenadas y categoría; algunos tienen web y contacto.
  Sus licencias permiten reutilización y varían según el proveedor. No incluye
  OSM en su tema Places. Falta medir su aportación real dentro del límite de
  Navarra, comprobar cierres y deduplicar.
- [IDENA](https://geoportal.navarra.es/es/idena/descargar): recursos turísticos y
  localizaciones oficiales; útil para verificar geografía y resolver municipios.
- [Centros educativos](https://datosabiertos.navarra.es/dataset/centros-educativos):
  titularidad, enseñanzas y contacto, incluyendo educación infantil. Seleccionar
  el curso vigente y la actividad elegible; la existencia de un centro público
  no demuestra una oportunidad comercial.
- [Listados oficiales de servicios plaguicidas](https://www.navarra.es/NR/rdonlyres/73313760-91CD-4D6F-976E-8DB7E2DCFC2E/492649/ListadoRamaB.pdf):
  cubren control de plagas; incluyen operadores de otras provincias. Revisar
  fecha, ubicación y condiciones de reutilización antes de automatizarlos.
- Directorios de ayuntamientos y asociaciones: la página de Fitero demuestra
  cobertura útil para los nombres de la lista. Cada adaptador debe tener
  procedencia, licencia o autorización de reutilización y control de actualización.

## Google Maps y Google Search

**Sí se pueden buscar negocios en Google Maps mediante APIs oficiales.**

- [Places API Text Search](https://developers.google.com/maps/documentation/places/web-service/text-search)
  permite consultas como `fontaneros en Tudela` o un nombre concreto. La
  documentación limita actualmente la consulta a 60 resultados entre páginas y
  no garantiza resultados idénticos entre peticiones. Para cobertura se necesitan
  consultas por rubro y zona y registrar cuándo se agota la paginación.
- [Places Aggregate API](https://developers.google.com/maps/documentation/places-aggregate/overview)
  proporciona recuentos y devuelve identificadores cuando el recuento es 100 o
  inferior. Puede servir para medir cobertura, sujeto a sus condiciones de uso.
- [Places Insights](https://developers.google.com/maps/documentation/placesinsights/overview)
  permite análisis en BigQuery y requiere solicitar acceso. Tiene más complejidad
  de configuración que el piloto con datos abiertos.
- [Business Profile](https://developers.google.com/my-business/content/location-data)
  permite gestionar ubicaciones que el usuario posee o administra; no proporciona
  un listado general de todas las empresas registradas.

Google permite expresamente usar Places para gestionar contenido asociado a
clientes y oportunidades de un equipo comercial en sus [usos permitidos del EEE](https://cloud.google.com/terms/maps-platform/eea-places-api-permitted-uses).
La integración debe respetar las [restricciones de almacenamiento y extracción](https://cloud.google.com/terms/maps-platform/eea)
y las [condiciones específicas del EEE](https://cloud.google.com/terms/maps-platform/eea/maps-service-terms).
El identificador del lugar puede conservarse; otros contenidos tienen condiciones
distintas, y la visualización junto a mapas necesita un diseño compatible.
No puede tratarse como un CSV libre para copiar nombres, direcciones y reseñas
permanentemente a nuestra base.

La [tarifa oficial](https://developers.google.com/maps/billing-and-pricing/pricing)
incluye cuotas gratuitas y precios según campos y volumen. Obtener solo IDs tiene
un SKU sin coste; nombres, datos de contacto y otros detalles activan SKUs
distintos. No se han realizado consultas autenticadas a Places: no hay una clave
Google configurada en el despliegue. Esta investigación no mide su cobertura real.

Para el buscador general, la [Custom Search JSON API](https://developers.google.com/custom-search/v1/overview)
está cerrada a nuevos clientes y los existentes deben migrar antes del 1 de enero
de 2027. No es una base recomendable para una integración nueva. Radar puede
ampliar su adaptador de buscador con consultas por rubro y localidad; ahora solo
busca webs de empresas ya conocidas y necesita configurar el proveedor.

## Orden de implementación recomendado y aceptación

1. Corregir los contadores, la identificación de los objetivos conocidos y la
   exclusión automática de asociaciones y ONGs por falta de actividad mercantil.
2. Integrar DENA del último año, industria, turismo y cooperativas como fuentes de candidatos
   en el mismo trabajo regional de la web, con resultados por fuente y rubro.
   Incorporar registros de asociaciones y entidades sociales según la vigencia y
   reutilización comprobadas de cada fuente.
3. Mantener separados nombre comercial, razón social y variantes; resolver
   identidad por varias evidencias, sin unir sucursales por compartir dominio.
   Registrar observación, procedencia y fecha del dato por atributo.
4. Resolver dirección y municipio; una falta de coordenadas puede impedir un pin,
   pero no debe impedir que un negocio con ubicación confirmada entre en la tabla.
5. Usar el buscador para contrastar actividad, localizar web y descubrir negocios
   fuera de los registros; recorrer rubros, sinónimos y municipios con caché y cuota.
   La ausencia de web en un registro no confirma que el negocio carezca de ella.
6. Medir Overture y adaptar directorios locales. Evaluar Google como complemento
   de consulta, con presentación y conservación compatibles con sus condiciones.

La aceptación debe comparar, por fuente y rubro, filas descargadas, candidatos
elegibles, negocios nuevos reales, vínculos añadidos, duplicados, ubicaciones
pendientes y webs confirmadas. Repetir exactamente el mismo trabajo debe mantener
los negocios y vínculos. Los objetivos conocidos sirven como casos de cobertura:
coincidencias verificadas o motivos explícitos de revisión, nunca altas forzadas.
No se promete un número final de altas antes de medir esos cruces.

## Reproducir la prueba

```bash
python3 scripts/probar-cobertura.py \
  --db /var/lib/radar/radar.db \
  --radar /usr/local/bin/radar \
  --output /tmp/radar-cobertura-nueva \
  --nombre 'Lainez Automoción' \
  --nombre 'Bodegas Ontanon' \
  --nombre 'Sociedad Cooperativa San Raimundo'
```

El script crea su propia copia, ejecuta dos escaneos y consulta las fuentes
oficiales con paginación. Conserva resultados, hashes y registros de ejecución
en la carpeta indicada. El informe de esta consulta está en
`/tmp/radar-cobertura-20261105/resultado.json`; esa carpeta contiene una copia de
la base y no debe publicarse ni añadirse a Git. Para consultar únicamente fuentes
externas, omitir `--db`. `--year` permite fijar un año de DENA; por defecto usa el
último publicado. Overture y Google no se consultan desde este script.
