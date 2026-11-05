# Plan de implementación: rastreo, diagnóstico y ficha

Fecha del plan: 5 de noviembre de 2026. Estado: E1 y la base abierta de E2–E3 desplegadas; E4 y E5 parciales; E6 condicionado.

Objetivo: ampliar la cartera de negocios y entidades de Navarra, confirmar sus
webs de forma inequívoca y preparar propuestas de mejora con evidencias. La ficha
debe aprovechar mejor el espacio y mostrar con claridad qué información está
confirmada.

Este documento reúne y concreta las investigaciones de [fuentes de captación](investigacion-fuentes-negocios.md)
y [diagnóstico e informes](diagnostico-web-y-propuesta-comercial.md), junto a los
cambios de interfaz solicitados. La confirmación manual definida aquí sustituye
la propuesta anterior de confirmar automáticamente una web por coincidencia técnica.

## 1. Alcance y decisiones

- Toda Navarra; negocios, asociaciones, ONGs, fundaciones vinculadas a estas
  actividades, entidades de voluntariado y cooperativas de distintos sectores.
- Conservar los cinco sectores generales, el rubro detallado y la categoría de
  origen. Distinguir naturaleza jurídica y perfiles de actividad: una cooperativa
  puede ser una bodega y una ONG puede ser una asociación.
- Incorporar primero las fuentes abiertas verificadas. Google y fuentes sin
  exportación/reutilización comprobadas tendrán un alcance separado y explícito.
- La ausencia de una URL en una fuente no demuestra que una entidad carezca de web.
- Una URL encontrada o técnicamente contrastada sigue siendo sugerida hasta una
  confirmación explícita del usuario. Auditar y presentar propuestas sobre la URL
  confirmada, manteniendo la trazabilidad de cada medición.
- Las migraciones conservarán cartera, descartes, seguimiento, actividades,
  correcciones manuales y auditorías históricas. No se plantea reconstruir ni
  vaciar la base para añadir estas funciones.

## 2. Orden de entregas

| Entrega | Contenido | Dependencias | Resultado revisable |
| --- | --- | --- | --- |
| E1 | Ficha compacta y confirmación de webs | Base actual | Caso de la captura corregido, con migración y pruebas del estado web |
| E2 | Modelo de entidades, admisión y captación DENA/cooperativas | E1 para URLs y auditorías | Primera ampliación de cartera con altas únicas y filtros coherentes |
| E3 | Industria, turismo, entidades sociales, búsqueda web y objetivos | E2 | Captación por varias fuentes desde el trabajo de producción y cobertura por rubro |
| E4 | Colector de evidencias y pestaña Diagnóstico web | E1; aprovecha cartera E2/E3 | Fallos por página, tamaño y estado, con capturas y revisión |
| E5 | Dossier HTML/PDF, calibración y validación completa | E3 y E4 | Informe para la visita y pruebas del flujo completo sobre fuentes reales |
| E6 | Overture, directorios locales adicionales y complemento Google | Base de ingesta E2/E3 | Pilotos medidos; activación de fuentes según cobertura y compatibilidad verificadas |

Cada entrega tendrá una migración compatible, pruebas de su alcance y un
despliegue verificable antes de continuar la siguiente. El trabajo de diagnóstico
se podrá desarrollar cuando E1 esté estable, sin esperar a medir todos los pilotos E6.

## 3. E1 — Ficha compacta y estado web correcto

### 3.1 Cambios visuales exactos

| Elemento actual | Cambio previsto |
| --- | --- |
| `Procedencia: overpass` u otra procedencia | Retirar de la ficha; conservar la procedencia en los datos y en Fuentes |
| `Datos: © OpenStreetMap contributors · ODbL` | Retirar de la ficha y centralizar atribuciones en Fuentes con acceso global; mantener la atribución del mapa |
| `Encaje estimado a partir de la información disponible.` | Retirar de la ficha |
| `Candidata · OSM · 5/11/2026`, posición o consulta técnica de esa sugerencia | Retirar esos metadatos de la tarjeta sugerida; conservar su evidencia interna |
| Categoría, rubro, potencial y estado en párrafos separados | Fila de etiquetas con ajuste de línea; nombres comprensibles en español y sin repetir categoría/rubro equivalentes |
| Dirección aislada con `📍` | Integrarla en la línea de ubicación: `Doneztebe/Santesteban · Ameztia zeharkarrika 3` |
| Teléfono y email en bloques independientes | Fila de contacto con ajuste de línea, enlaces utilizables y sin columnas vacías |
| Última auditoría y fecha | Una fila compacta cuando el ancho lo permita |
| Servicios recomendados | Etiquetas horizontales con salto de línea y separación consistente |
| Scrollbar desplazado hacia dentro | Un contenedor de scroll que ocupe todo el ancho interior de la tarjeta; padding en una envoltura de contenido |

La fila de ubicación omitirá los separadores cuando falte un dato. Las etiquetas
desconocidas se omitirán o identificarán como desconocidas; no se inventará un tipo
de entidad. La dirección seguirá disponible en los datos y exportaciones.

Estructura orientativa de la ficha:

```text
Belarra                                  [Reset tamaño]
Ficha | Diagnóstico web | Auditoría | informes disponibles

[Restaurantes y cartas] [Potencial …] [Estado de seguimiento]
Doneztebe/Santesteban · Ameztia zeharkarrika 3
Teléfono · Email

Presencia web
[Web sugerida · Sin confirmar]
https://…
[Confirmar web propia] [Descartar sugerencia] [Añadir web]

Última auditoría · estado y fecha, si corresponde a la web actual
Servicios: [SEO] [Landing] [Creación web] [Hosting]
Seguimiento…
```

La cabecera y pestañas seguirán fijas. Tipo, potencial, contactos y controles
compactos usarán filas con `flex-wrap`, sin forzar una sola línea en tarjetas
estrechas. Formularios y notas mantendrán espacio suficiente para leer y escribir.

Para el scrollbar se retirará la reserva de gutter del contenedor exterior con
`overflow:hidden` y se evitará duplicarla con la del contenido. El scroll quedará
junto al borde derecho interior, sin superponerse al texto. Tabla y mapa compartirán
la misma ficha; se conservará el ajuste de ancho y la apertura en móvil.

### 3.2 Confirmación: regla de producto y backend

La interfaz actual deduce «Web confirmada» de `negocio.web`. Además, el arranque
marca URLs de fuentes como `confirmada_fuente`, y la búsqueda puede confirmar
automáticamente una coincidencia. Estos tres caminos deben ajustarse juntos.

| Estado real | Presentación y acciones |
| --- | --- |
| Sin URL conocida ni candidata | Web no localizada; búsqueda o introducción de URL |
| Una o más sugerencias sin confirmación manual | Solo tarjetas sugeridas, con URL y botones. No mostrar «Web confirmada» ni otra copia de la misma URL |
| El usuario confirma una sugerencia o introduce y confirma una URL propia | Mostrar una única web confirmada; retirar esa URL de sugerencias y habilitar la auditoría correspondiente |
| Existe una web A confirmada manualmente y aparece una alternativa B | A conserva su confirmación; B se presenta como alternativa sin confirmar. Una sugerencia nunca sustituye A por sí sola |
| Se descarta una sugerencia | Se oculta esa sugerencia; no se convierte en prueba de ausencia de web |

Pasos de implementación:

1. Establecer una única función de elegibilidad de URL confirmada para ficha,
   tabla, mapa, auditoría rápida, auditoría técnica, diagnóstico y trabajos masivos.
2. Guardar URLs de OSM, registros, directorios y buscadores como candidatas con
   procedencia. La validación automática puede añadir evidencia/confianza y ordenar
   candidatas, pero no ejecutar la confirmación del usuario.
3. Exponer en el contrato el estado y la URL confirmada de forma explícita. No usar
   la mera presencia del campo legado `web` como confirmación.
4. Migrar `confirmada_fuente`, confirmaciones automáticas y URLs heredadas sin
   constancia de acción del usuario a sugerencias. Conservar URL y evidencia;
   respetar confirmaciones con origen `correccion_usuario` u otro registro manual
   equivalente verificado.
5. Normalizar URLs para evitar duplicados por barra final, host y variantes
   equivalentes comprobadas. No unir dominios distintos por parecido.
6. Confirmar dentro de una operación consistente: actualizar estado, retirar la
   candidata coincidente, recalcular encaje y programar una sola auditoría.
7. Al cambiar URL o retirar una confirmación heredada, actualizar generación y
   detener o diferir trabajos que ya no correspondan. Conservar resultados como
   históricos, sin presentarlos como diagnóstico vigente de una URL sin confirmar.
8. Recalcular estados y prioridades sin interpretar «pendiente de confirmar» como
   «el negocio no tiene web». Una importación posterior no sobrescribirá la elección.

**Aceptación E1:** el caso de Belarra de la captura muestra una sola sugerencia
hasta confirmar; al confirmar queda una única web confirmada y un único trabajo.
Los textos retirados no aparecen, la ubicación ocupa una línea/grupo y el scrollbar
queda al borde. Se verifica en tabla y mapa, en tarjeta estrecha/ancha y móvil,
incluyendo URL larga, datos ausentes, varias candidatas, teclado y cambio de ficha.

## 4. E2 — Ingesta fiable, tipos de entidad y primeras fuentes

### Modelo y admisión

- Añadir naturaleza jurídica opcional y perfiles de actividad no excluyentes, sin
  sustituir sector ni rubro. Incorporar filtros en tabla y mapa.
- Admitir entidades no lucrativas con actividad e implantación en Navarra
  verificadas. Reevaluar asociaciones/ONGs previamente excluidas por no demostrar
  actividad mercantil. Comprobar cierres, inactividad y autonomía de delegaciones
  sin aplicarles ciegamente reglas pensadas para cadenas comerciales.
- Mantener nombre comercial, razón social y variantes con evidencia. Usar una
  clasificación explícita y versionada de CNAE; conservar código y descripción
  originales y señalar discrepancias o categorías desconocidas.
- Unificar la procedencia por atributo, la observación y la corrección manual.
  Cambiar datos de una fuente no borrará seguimiento, descartes ni decisiones humanas.

### Arquitectura de fuentes

Crear adaptadores con un flujo común: descargar/paginar, normalizar, comprobar
ubicación y vigencia, clasificar, resolver identidad y persistir. El trabajo de
«Buscar en toda Navarra» y la CLI ejecutarán esta misma orquestación.

Cada adaptador mantendrá fuente, licencia/atribución, identificador externo estable,
fecha de datos y captura, versión, cursor, límites y errores. Evitar usar solo
el número de fila o año como identidad de un establecimiento que se repite entre
ediciones. Guardar checkpoints por fuente/página y reanudar tras fallos.

Resolver identidad con varias evidencias: registro, nombre/variantes, dirección,
contacto, dominio y coordenadas. Un NIF o dominio compartido no fusionará todas
las sucursales. Las coincidencias dudosas quedarán revisables.

Corregir el contador de altas: el upsert devolverá creación real, actualización o
vinculación a un negocio existente. Los registros admitidos y los vínculos de
fuentes no se contarán como nuevos negocios.

### Primeros adaptadores

1. **DENA, último año publicado:** paginar solo esa edición; conservar referencia
   temporal y contrastar actividad actual cuando haga falta. No importar todas
   las filas históricas como negocios distintos.
2. **Cooperativas vigentes:** clase, actividad, inscripción y domicilio; cubrir
   cooperativas agrarias y de servicios además de industria.
3. **OSM/BORME:** mantener su cobertura dentro del flujo común y corregir la
   exclusión de tipos de entidad que ahora interesan. BORME aporta señales de
   constitución, no prueba universal de apertura o falta de web.

La ubicación se resolverá con códigos municipales, nombres bilingües y direcciones,
usando recursos abiertos compatibles para georreferenciar. Un negocio con municipio
confirmado y sin coordenadas podrá entrar en tabla; no se inventará un pin exacto.

**Aceptación E2:** datos anteriores conservados; cooperativa filtrable también por
su actividad; ONG constituida como asociación localizable por ambos perfiles;
repetición de fuente/edición sin duplicados; confirmación manual protegida; informe
de altas reales y pendientes por fuente, rubro y municipio.

## 5. E3 — Nuevas fuentes, búsqueda web y objetivos

### Fuentes siguientes

- Registro industrial A/B/C: deduplicar habilitaciones y verificar ubicación en
  Navarra antes de incorporar.
- Turismo: restaurantes, alojamientos y empresas de turismo activo/cultural como
  captación, además del enriquecimiento actual. Conservar las señales de visitas
  y precio como evidencias; no deducir una tarifa por categoría.
- Entidades deportivas/federaciones y registros de asociaciones con datos
  reutilizables: verificar fecha y estado. El conjunto juvenil 2018–2021 sirve
  como pista histórica y requiere corroboración reciente.
- Navarra + Voluntaria, Coordinadora de ONGD y otros registros: implementar
  adaptador al verificar acceso, exportación y reutilización; mostrar explícitamente
  las fuentes aún pendientes, sin prometer cobertura que no se ha medido.

### Búsqueda web reproducible

Separar consultas de descubrimiento por rubro/localidad de las consultas para
identificar una entidad conocida. Construir una matriz versionada de municipios,
rubros, sinónimos y variantes de nombres; priorizar huecos de cobertura y distribuir
el trabajo para no agotar cuotas en combinaciones repetidas.

Reutilizar el adaptador de buscador con caché, reserva de presupuesto, deduplicación
de consultas y límites. El despliegue actual no tiene clave Tavily: las fuentes
abiertas funcionarán sin ella, y las consultas que necesiten proveedor quedarán
identificadas como no ejecutadas. Mantener el límite gratuito configurado, sin
activar facturación. Una búsqueda nunca sustituye evidencia por una respuesta
generada ni confirma una web por su posición.

Los resultados registrarán consulta exacta, proveedor, fecha, URLs y contraste de
identidad. Una URL sugerida entrará en el flujo manual definido en E1.

### Objetivos y casos reales

- Separar Ontañón y San Raimundo, contrastando el rubro vigente de la cooperativa.
- Incorporar variantes verificadas para Lainez Automoción y Rincón del Torero.
  Rincón del Torero de Buñuel ya está en cartera y debe vincularse sin crear otra ficha.
- Revisar SANCO de Fitero sin asociarlo automáticamente a otra empresa homónima.
- Mantener pendientes los objetivos de ubicación o identidad incierta, incluida
  La Taska de Barillas frente a otras Taska y Castillo de GaskonciKo.
- Contrastar visitas/actividad del Castillo Palacio de Monteagudo con una fuente
  del operador o registro adecuado antes de asignarle una oportunidad turística.

**Aceptación E3:** pruebas con consultas reales de la matriz y adaptadores que
usará producción; listado de objetivos resueltos/pendientes con motivos; cobertura
incremental medible y sin altas forzadas por parecido de nombre.

## 6. E4 — Diagnóstico web con evidencias

### Motores y colector propio

- Lighthouse para laboratorio y diagnóstico de carga; SiteOne para rastreo y SEO
  técnico. Mantener acceso a los informes originales y su descarga HTML.
- Añadir Puppeteer y axe-core como dependencias directas fijadas, compatibles con
  Chrome. Reutilizar el worker, sandbox y proxy público.
- Colector propio de responsive, interacción segura, DOM y capturas. Playwright y
  Pa11y quedan como alternativas; no se instalan motores duplicados sin una necesidad
  medida. Screaming Frog no es requisito de producción.

Mejorar el esquema normalizado: regla y versión, motor, URL, estado de página,
viewport, selector, evidencia DOM, datos observados, gravedad, confianza,
referencia de accesibilidad, ocurrencias, captura y recomendación.

Agrupar duplicados entre motores y entre páginas de una plantilla. Preservar
resultados incompletos, excepciones y topes; ausencia de medición no equivale a aprobado.

### Alcance inicial configurable

- Hasta cinco páginas relevantes: inicio, contacto, servicio principal y, según
  actividad, carta, reserva, proyecto, donación o alta de socios.
- Anchos 320, 390, 768 y 1366 px CSS, con estado y entorno registrados.
- Desbordamiento, recortes y superposiciones contrastadas; menús y acciones de
  contacto, sin enviar formularios ni crear reservas.
- axe sobre DOM renderizado y estados relevantes; revisión asistida de teclado,
  foco y casos no automatizables. No duplicar los mismos avisos de Lighthouse.
- Rastreo SEO con cobertura declarada y contraste de JavaScript. Revisar intención
  de `noindex`, canonical y redirecciones antes de calificarlos como errores.
- Rendimiento exploratorio con una medición; para conclusiones del dossier,
  repeticiones compatibles con mediana, rango y número de muestras.
- CrUX opcional, con clave y muestra disponible; datos de campo separados de los
  de laboratorio. INP no se sustituye por TBT.

Conservar los límites operativos iniciales: rastreo hasta 100 URLs/profundidad 3,
un sitio y navegador a la vez, presupuesto de hasta diez minutos con resultados
parciales y el límite actual de memoria del worker. Medir coste antes de ampliar
la matriz o concurrencia. El nuevo trabajo tendrá progreso y reintento propios,
compatibles con los cuatro informes técnicos actuales.

### Nueva pestaña

Añadir **Diagnóstico web** junto a Ficha y Auditoría, visible también sin resultados.
Incluir resumen para la visita, capturas, hallazgos por categoría, revisión,
acciones propuestas e histórico. Mostrar estados pendientes, parciales y bloqueos
sin puntuaciones inventadas. Los resultados se vincularán a URL, generación y ejecución.

**Aceptación E4:** fallos reproducibles con evidencia; carouseles/mapas y estados
intencionales no producen avisos ciegos; reglas accesibles válidas no se señalan
por duplicado; cambio de dominio no reutiliza un diagnóstico previo como vigente;
el trabajo recupera fases y respeta los límites del worker.

## 7. E5 — Informe comercial y calibración

- Seleccionar tres a cinco hallazgos prioritarios. Cada uno tendrá prueba, efecto
  posible en el uso, solución y alcance. Mostrar fortalezas y límites de la muestra.
- Estados revisables: pendiente, confirmado, descartado con motivo y resuelto.
  Separar gravedad técnica, confianza y prioridad de la propuesta.
- Preparar recomendaciones de arreglar, mejorar una parte o rehacer; esta última
  requiere revisar problemas estructurales, necesidades y alternativas. Las
  puntuaciones no son una probabilidad de venta ni una promesa de posicionamiento.
- Generar HTML autocontenido y PDF con capturas incorporadas, fecha, páginas,
  perfiles, condiciones y versión del diagnóstico; revisión de textos antes de exportar.
- Ampliar explícitamente formatos admitidos en archivos, limpieza y descarga
  autenticada. La exportación debe conservar sus pruebas aunque caduquen los
  temporales del servidor. Retención/cuotas configurables y visibles.
- Comparar antes/después con la misma página, estado, perfil y condiciones.
  El dossier no se enviará ni publicará automáticamente.

La demostración local de [diagnóstico](diagnostico-web-y-propuesta-comercial.md#demostración-local-y-aceptación)
valida la viabilidad, pero no sustituye las pruebas en webs reales del flujo de producción.

**Aceptación E5:** exportación utilizable durante la visita sin una sesión de
Radar ni imágenes remotas; hallazgos seleccionados con revisión y trazabilidad;
comparaciones compatibles y recomendaciones justificadas.

## 8. E6 — Ampliaciones condicionadas a resultados

1. Medir Overture Places dentro del límite de Navarra, vigencia y aportación única
   respecto a la cartera. Incorporarlo al mismo flujo si el piloto demuestra utilidad.
2. Añadir directorios municipales y sectoriales con adaptadores acotados. Fitero
   es un primer caso de contraste para los objetivos del usuario.
3. Evaluar Google Places como consulta complementaria, con caché, presentación,
   atribución y uso de datos compatibles con sus condiciones. No volcar sus
   contenidos como un censo libre a SQLite. No activar gasto ni convertir la
   integración en requisito de la captación abierta.
4. Mantener Google Business Profile para ubicaciones administradas; no usarlo como
   catálogo público. No basar una integración nueva en Custom Search JSON API.

**Aceptación E6:** cada piloto informa costes/cuotas, restricciones, consultas
realizadas, cobertura y aporte único antes de su activación general.

## 9. Plan de pruebas y puesta en producción

### Datos y rastreo

- Copia de producción mediante SQLite backup, con la línea base de 5.214 negocios
  únicos observada en la investigación. Registrar la línea base real al comenzar
  cada ensayo; no asumir que permanece fija.
- Ejecutar la misma orquestación, parámetros, consultas y adaptadores que usará
  la web; no un importador especial que evite filtros o deduplicación.
- Probar fuente vacía, paginación, cambios de esquema, edición anual, errores,
  reintentos y recuperación tras reinicio.
- Repetir el mismo conjunto de entrada: cero nuevos negocios y cero vínculos
  duplicados en la repetición, salvo cambios reales registrados por la fuente.
- Comparar por fuente, rubro, tipo de entidad y municipio: filas, admitidos,
  nuevos únicos, actualizados, vínculos, excluidos, pendientes y errores.
- Verificar manualmente una muestra de altas, casos conocidos y posibles
  duplicados. No fijar una promesa de altas a partir de sumar filas de registros.

### Estado web y ficha

- Cubrir las cinco situaciones de la tabla de confirmación, migración de URLs
  heredadas y protección de correcciones manuales.
- Confirmación repetida y reescaneo no duplican URL ni auditoría; cambiar URL
  invalida atribución de trabajos antiguos sin borrar su histórico.
- Comparación visual con la captura del usuario: textos retirados, ubicación unida,
  etiquetas horizontales y scrollbar al borde.
- Probar la ficha a distintos anchos y en móvil, con texto largo, falta de campos,
  teclado, pestañas y cambio entre tabla y mapa.

### Diagnóstico e informe

- Casos controlados con y sin fallos, más una muestra real que incluya web
  tradicional, web con JavaScript, restaurante, servicios y entidad no lucrativa.
- Contrastar hallazgos con revisión manual y Accessibility Insights cuando proceda;
  registrar falsos positivos, omisiones y reproducibilidad.
- Probar timeouts, bloqueos, falta de datos CrUX, capturas ausentes, límites de
  memoria y reinicio, sin presentar resultados parciales como cobertura completa.
- Verificar PDF/HTML con evidencias incorporadas, revisión persistente y fechas
  correctas; no declarar conformidad completa de accesibilidad con un score automático.

### Despliegue por entrega

1. Backup, ensayo de migración sobre copia y verificación de columnas reales;
   coordinar el versionado de módulos para evitar colisiones de migraciones.
2. Pruebas Rust/worker apropiadas, contrato OpenAPI generado y cliente TypeScript
   regenerado, compilación y comprobación de la interfaz.
3. Despliegue con funciones nuevas activadas de forma controlada y posibilidad
   de desactivar fuentes/colector sin perder la cartera.
4. Comprobación de servicio, login, tabla/mapa, confirmación y primer trabajo real.
5. Captura regional progresiva con revisión de crecimiento único, errores, cuotas
   y recursos. Conservar procedimiento de recuperación y compatibilidad del binario.

## 10. Áreas principales del código

| Área | Piezas previstas |
| --- | --- |
| Ficha y presentación | `FichaNegocio.tsx`, `WebNegocio.tsx`, `styles.css`, atribuciones globales/Fuentes |
| Confirmación | `web.rs`, `web_api.rs`, modelo/API, jobs rápidos/técnicos y stores web |
| Ingesta | Adaptadores de `fuentes`, orquestación scan/CLI/jobs, deduplicación y store |
| Entidades y filtros | Modelo/migraciones, clasificación, API, tabla y mapa |
| Diagnóstico | Normalizador y colector del worker, cola/API, persistencia, componente de pestaña |
| Dossier | Plantilla, exportador, archivos, retención y descargas |
| Verificación | Prueba de cobertura, casos de confirmación, pruebas del colector y revisión de UI/PDF |

## 11. Estado de implementación y alcance entregado

### Implementado localmente

- **E1:** ficha compacta y confirmación manual inequívoca. URLs heredadas y de
  fuentes pasan a sugerencias; solo las confirmaciones explícitas del usuario se
  muestran y habilitan auditorías como web propia. La atribución de fuentes queda
  en Fuentes y la ficha compacta sus etiquetas, ubicación y contactos.
- **E2–E3 (fuentes abiertas):** el rastreo habitual de CLI y API incorpora OSM,
  DENA vigente, cooperativas, turismo y registros industriales de Navarra. Los
  adaptadores conservan fuente y atributos, clasifican sector/rubro/tipo de entidad,
  enlazan coincidencias y permiten filtrar rubros y naturaleza de entidad en tabla
  y mapa. La lista `data/objetivos.csv` importa cuatro objetivos verificados y
  deja cuatro ambiguos pendientes; no fuerza su alta.
- **E4:** nueva pestaña Diagnóstico web que reúne informes, incidencias, cobertura
  y capturas responsive de hasta cinco páginas a 320, 390, 768 y 1366 px. El worker
  suma axe-core y hallazgos de desbordamiento responsive sobre Chromium emulado.
  La revisión manual queda persistida por informe y hallazgo, con estados pendiente,
  confirmado, descartado y resuelto, y motivo de descarte.
- **E5 (parcial):** generación local de dossier HTML autocontenido y flujo de
  impresión/guardado PDF, con capturas incrustadas y límites del diagnóstico.
  La selección para el dossier sigue guardándose en el navegador; no hay flujo de
  aprobación multiusuario ni comparación antes/después.

### Ensayo de captura

Se ejecutó la misma orden de producción (`radar-cli scan --navarra --sector todos`)
sobre una copia aislada de SQLite y el PBF de Navarra. La primera pasada registró
55.604 negocios únicos y 61.291 vínculos de fuente; DENA y cooperativas aportaron
40.480 y 1.456 altas, respectivamente, y se enlazaron registros existentes.
Al repetir la consulta forzando la lectura de las mismas fuentes, ambas mantuvieron
sus altas sin duplicados y la cuenta de negocios y vínculos no creció. La lista de
objetivos dio cuatro resoluciones y cuatro pendientes de verificación. El ensayo
no modificó la base de producción.

### Pendiente o condicionado

- No se han activado Overture, Google Places, directorios locales adicionales ni
  nuevas consultas de búsqueda web: requieren medir cobertura, condiciones y cuotas;
  Google además no debe convertirse en una copia de censo local.
- Queda pendiente una muestra real de calibración para falsos positivos, rendimiento
  repetido y revisión manual del diagnóstico. No se informa CrUX y no se emite una
  certificación WCAG.
- El dossier ya se genera localmente, pero la selección aún es por navegador, no
  se sincroniza entre usuarios y la comparación antes/después no está hecha.
- La UI informa de los informes disponibles y cobertura guardada, pero el worker
  todavía no ofrece progreso separado y reintento por página/fase para el colector.
- La búsqueda regional de fuentes no se ha lanzado en producción; el ensayo con
  55.604 negocios se realizó únicamente en una copia aislada.

### Despliegue

Desplegado el 5 de noviembre de 2026 en `radar-navarra.duckdns.org`. Antes del
reinicio se creó `/var/lib/radar/backups/radar-predeploy-20261105T113255Z.sqlite`
y se verificó con `PRAGMA integrity_check`. También se guardó la instalación
anterior del worker en `/var/backups/radar-audit-predeploy-20261105T113450Z.tar.gz`.
Tras el arranque, Radar y `radar-audit` quedaron activos, la nueva tabla de revisión
existe, el contrato OpenAPI publica ese endpoint, la web y sus assets responden
200 por HTTPS y la base vuelve a pasar `integrity_check`. No se ejecutó una captura
regional ni se añadieron negocios durante el despliegue.

La instalación del worker informa de cinco avisos moderados transitivos de `glob`
y `minimatch` en la cadena de Lighthouse; la corrección sugerida por npm requiere
un cambio mayor/degradación de versión y queda para una actualización evaluada.

### Verificación local

Verificación ejecutada con éxito: `cargo test --workspace` (40 pruebas),
`npm run build` en `radar-ui`, y `node --check worker.mjs` junto con `npm test`
en `audit-worker` (5 pruebas).
Los recuentos anteriores proceden del ensayo aislado descrito arriba y no son una
promesa de altas futuras.
