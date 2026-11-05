# Solución para recuperar y encontrar webs de negocios

Fecha de diagnóstico: 5 de noviembre de 2026.

## Qué ha pasado

La web no se perdió por completo. La migración a confirmación manual introdujo
una regla en `radar_core::web::init`: las URLs que no tenían una confirmación
explícita de usuario se copiaban a `web_candidatas`, se cambiaba su estado a
pendiente y se ponía `negocios.web` a `NULL`. Esto afectó a las URLs que antes
constaban como `confirmada_fuente`.

La copia de las 11:32 contiene 5.214 negocios y 1.046 valores de `web`. De ellos,
1.040 estaban clasificados como `confirmada_fuente` de Overpass, tres tenían
confirmación manual y tres estaban pendientes. Los 1.046 registros enlazan con
el conjunto actual por la clave estable `(fuente, fuente_id)`. La base actual
tiene 55.604 negocios y solo tres confirmaciones manuales; los otros 1.043
registros heredados figuran como pendientes de migración. Sus URLs reaparecen en
la tabla de candidatas, junto con otros resultados de fuentes.

La aplicación muestra como **confirmada** únicamente una web aceptada por una
persona. Por tanto, un enlace de OpenStreetMap o de un registro oficial no debe
presentarse como confirmación manual ni como prueba de que el sitio pertenece al
negocio. Se debe recuperar la procedencia y visibilidad de los enlaces, sin
falsear quién los validó.

Estado observado: 55.604 negocios; 3 webs manualmente confirmadas; 1.946
candidaturas en total. Hay 1.044 candidaturas cuyo proveedor es OSM, 498 de
industria y 136 de turismo, además de candidaturas de registros regionales y
otras fuentes. Son candidaturas, no 1.946 dominios necesariamente únicos ni
1.946 empresas con identidad ya verificada.

## Implementación desplegada

La aplicación ejecuta ahora una cola automática y reanudable: primero vuelve a
contrastar candidatas existentes (incluidos los enlaces OSM recuperados como
candidatos) y después busca negocios que aún no tengan URL. Las búsquedas Tavily
se limitan a 35 créditos diarios y 1.000 mensuales, con reserva transaccional y
caché. La clave se configura en el entorno del servicio y no se guarda en este
repositorio. Un enlace solo pasa a «verificada automáticamente» tras superar
comprobaciones de acceso, nombre e identidad; directorios y redes sociales se
excluyen. Las URLs automáticas de alta confianza pueden iniciar una auditoría
técnica. La interfaz distingue ese estado del confirmado por una persona.

El servicio también agenda la sincronización periódica de fuentes regionales y
OSM, y recorre fuentes web previamente revisadas. El ritmo externo está limitado
por la cuota gratuita; por ello, no se presenta como una promesa de encontrar
sitio para cada empresa. La métrica de candidatas tampoco equivale a webs propias
verificadas.

## Objetivo

Encontrar una web para la mayor parte de los negocios admisibles de Navarra,
manteniendo estas distinciones visibles:

1. **Confirmada por una persona:** el usuario revisó el negocio y aceptó la URL.
2. **Verificada por fuente:** una fuente identificable publica el enlace y se
   ha superado un contraste de identidad; conserva fuente, fecha y evidencia.
3. **Candidata:** hay una URL posible, pero faltan pruebas para atribuírsela al
   negocio.
4. **Sin localizar / revisión necesaria:** no hay resultado fiable o hay
   candidatos en conflicto.

«Verificada por fuente» no debe reescribirse como confirmación del usuario. La
interfaz debe permitir usarla para priorizar, filtrarla y revisarla en lotes.

## Diseño por etapas

### 1. Recuperar y hacer visibles los enlaces ya conocidos

- Tratar `web_candidatas` y las copias de seguridad como fuentes de recuperación;
  no volver a importar desde una fuente sin deduplicar por negocio, URL y
  procedencia.
- Reasociar datos heredados mediante `(fuente, fuente_id)`, nunca solo por nombre.
- Recuperar las 1.040 URLs de Overpass como **verificadas por fuente: OSM**,
  conservando el estado histórico y la fecha de importación. Mantener las tres
  confirmaciones manuales como tales. No elevar las tres filas heredadas
  pendientes ni perfiles de redes sociales.
- Migrar de manera idempotente: ejecutar dos veces produce el mismo estado; no
  reemplaza una confirmación manual nueva ni una URL corregida recientemente.
- Mostrar en la ficha y en la tabla el número de enlaces encontrados, su
  procedencia y el botón «Revisar». Añadir acciones de selección múltiple para
  aceptar o descartar resultados después de mostrar nombre, dominio y evidencia.

### 2. Exprimir las fuentes ya disponibles

Antes de pagar por búsquedas, generar un inventario de fuentes con cobertura por
municipio, rubro, fecha y número de identificadores enlazables:

- **OpenStreetMap:** `website`, `contact:website` y variantes de contacto; los
  campos sociales se guardan como redes sociales, no como web propia.
- **Registros de Navarra:** Turismo, alojamientos, industria y otros registros
  abiertos que tengan una URL publicable. La coincidencia debe usar identificador
  oficial cuando exista; si no, nombre normalizado más municipio y otro dato.
- **Fuentes ya ingeridas:** revisar atributos originales y relaciones
  `negocio_fuentes` para recuperar enlaces que se hayan omitido en la importación.
- **Candidaturas existentes:** deduplicar dominios por negocio, preservar
  `proveedor`, `consulta`, posición, fecha y `evidencias`.

Cada adaptador guardará el identificador del registro externo y la fecha de la
última sincronización. Las fuentes con cambios incrementales solo volverán a
procesar altas y modificaciones.

### 3. Buscar en la web los negocios restantes

La búsqueda actual está diseñada para un negocio por trabajo y su límite es de
1.000 créditos Tavily al mes. No puede cubrir 55.604 negocios: aunque cada
consulta consumiera un crédito, necesitaría más de 55 meses de cuota actual. No
se debe lanzar una búsqueda masiva sin priorización.

Crear una cola de descubrimiento reanudable y separada de la cola de auditoría
técnica:

- Agrupar trabajos por municipio y rubro, con persistencia de avance por negocio.
- Priorizar negocios admisibles con señales comerciales, sin web y con mejor
  calidad de identidad. Permitir también preparar una lista para visita.
- Consultar primero registros y cachés locales; reservar el buscador externo para
  los casos sin resultado y con identidad suficiente.
- Aplicar un límite diario configurable, cuota mensual visible, caché por consulta
  y backoff al recibir límites o errores. Un trabajo interrumpido reanuda en el
  primer negocio pendiente.
- Probar consultas con nombre, municipio y, si existe, dirección o teléfono. No
  elegir automáticamente el primer resultado del buscador.
- Guardar varias candidatas y su evidencia para resolver homónimos; no sobrescribir
  una URL manualmente confirmada.

Con el presupuesto vigente, la búsqueda externa servirá para obtener un conjunto
priorizado, mientras que la cobertura mayoritaria debe venir de registros y
fuentes de datos de mayor rendimiento por lote. La cuota y su agotamiento deben
ser visibles en la interfaz.

### 4. Resolver identidad con puntuación explicable

Evaluar cada candidatura usando señales observables:

- coincidencia exacta de identificador de registro o de `fuente_id`;
- nombre normalizado y municipio;
- teléfono o dirección coincidente;
- el dominio enlaza con el negocio desde una fuente de confianza;
- la página de destino contiene entidad estructurada coherente;
- el resultado es un directorio, red social, aparcamiento de dominio o una web
  compartida por negocios no relacionados.

Clasificación inicial propuesta:

- **Alta, verificable por fuente:** identificador de registro exacto o dos señales
  independientes fuertes, sin conflicto. Puede pasar a «verificada por fuente».
- **Media:** nombre y municipio coinciden, pero falta una segunda señal. Queda en
  revisión.
- **Baja o conflicto:** homónimo, directorio, red social o dominio no relacionado.
  No se asigna como web del negocio.

La puntuación y las señales deben poder explicarse en la ficha. Umbrales exactos
se calibrarán sobre una muestra revisada por una persona, midiendo precisión y
cobertura por fuente y rubro.

### 5. Auditorías técnicas sin bloquear la recuperación

Una URL encontrada no implica permiso para presentarla como web propia ni que
deba entrar inmediatamente en Lighthouse/SiteOne. Mantener el trabajo separado:

- Las confirmadas por una persona siguen siendo auditables automáticamente.
- Las verificadas por fuente solo entran en la cola técnica cuando la evidencia
  identifica una web propia con alta confianza, no es directorio ni red social,
  robots permite el acceso y se respeta el presupuesto por dominio.
- Las candidatas medias necesitan revisión humana antes de auditar.
- Concurrencia global baja, límite por dominio, reintentos con espera y captura de
  incidencias parciales. Una fase fallida no debe borrar el resultado de las otras.

Así la búsqueda puede avanzar en miles de registros sin fingir que todas esas
URLs ya cuentan con un diagnóstico completo.

## Métricas de salida

Panel diario con:

- negocios totales, sin URL, con candidatas, verificadas por fuente y confirmadas
  por usuario;
- tasa de coincidencia y falsos positivos revisados por fuente/rubro;
- candidatas nuevas por día, trabajos en cola y antigüedad de la cola;
- créditos externos consumidos y disponibles;
- URLs que no responden, redirigen, son redes/directorios o están bloqueadas por
  robots;
- auditorías completas, parciales y fallidas, separadas del estado de búsqueda.

No usar «negocios con web» para mezclar URL posible, fuente verificada y
confirmación manual. Mostrar cada cifra con su definición.

## Orden de implementación

1. Corregir la regresión de clasificación y migrar las 1.040 webs heredadas como
   verificadas por fuente; validar enlaces con IDs estables y hacer la migración
   reversible e idempotente.
2. Incorporar la fuente/procedencia/estado en tabla, ficha, filtros y revisión por
   lotes. Auditar solamente confirmaciones manuales durante esta primera etapa.
3. Añadir sincronización incremental de registros oficiales y enriquecer OSM con
   todas las variantes de campos web.
4. Implementar cola de descubrimiento reanudable, presupuesto, caché y
   priorización por municipio/rubro.
5. Calibrar identidad sobre una muestra revisada; activar auditoría automática
   para candidatas de alta confianza en una expansión gradual.

Antes del despliegue, crear copia SQLite en línea, exportar el conjunto de
`(fuente, fuente_id, url, estado, origen)` y comparar conteos. Después comprobar
que las confirmaciones manuales no cambian, que reaparecen los enlaces de fuente,
que la cola no agenda directorios y que el reinicio no vuelve a degradarlos.
