# Diagnóstico web para preparar propuestas comerciales

Investigación del 5 de noviembre de 2026. Se ha revisado el código de Radar, la
ayuda del SiteOne instalado, las dependencias locales y documentación primaria.
La pestaña descrita aquí todavía no está integrada ni desplegada. La demostración
usa HTML ficticio y no representa una auditoría de un cliente.

## Recomendación

Crear **Diagnóstico web**, una pestaña en la ficha con síntesis en español,
pruebas visuales, hallazgos revisables y un dossier exportable para la visita.
Reutilizar Lighthouse y SiteOne, y añadir un colector propio con Puppeteer y
axe-core para medir páginas renderizadas y sus estados.

La parte propia será la selección de páginas, las comprobaciones de adaptación
y uso, la conservación de evidencias, la prioridad y la explicación comercial.
Las reglas establecidas de accesibilidad y las mediciones de rendimiento se
apoyarán en motores mantenidos. El primer alcance no necesita contratar un SaaS.

## Qué hay ya y qué falta

| Área | Situación comprobada | Cambio necesario |
| --- | --- | --- |
| Interfaz | Ficha, Auditoría e informes HTML de Lighthouse y SiteOne | Una síntesis por problema y propuesta |
| Lighthouse 13.6.0 | Rendimiento, accesibilidad, buenas prácticas y SEO; una URL por perfil | Conservar nodos y detalles; ampliar muestra y estados |
| SiteOne 2.5.1 | Rastreo HTML hasta 100 URLs; renderizado condicionado, hasta cinco páginas | Renderizar expresamente páginas relevantes y conservar pruebas |
| Responsive | Sin análisis propio de geometría, solapamientos o menú | Medir en varios anchos y estados, con capturas |
| Normalización | Título, explicación, categoría, severidad y URL; Lighthouse se simplifica a severidad media/baja | Guardar impacto, selector, nodo, medición, referencias y confianza |
| Archivos | HTML; la limpieza elimina otros formatos; caducidad actual de siete días | Admitir capturas y dossier en almacenamiento y descargas autenticadas |

La ayuda del binario instalado indica que `--device` selecciona el **User-Agent**.
Para medir el diseño hace falta navegador y viewport explícito. El worker actual
solo usa navegador en ciertos casos y no activa `--screenshots`. Dos informes de
rastreo no demuestran por sí solos que la web funcione bien en móvil y escritorio.
La versión instalada admite `--screenshot-viewport=WxH` por invocación; no debemos
asumir opciones de versiones posteriores de GitHub.

En el histórico se comprobó una misma ejecución móvil con accesibilidad 10/10 en
SiteOne y 77/100 en Lighthouse, y distinta cobertura de páginas. Sus puntuaciones
no deben promediarse como si midieran lo mismo.

## Herramientas comparadas

| Herramienta | Aportación y decisión |
| --- | --- |
| [Lighthouse](https://developer.chrome.com/docs/lighthouse/overview/) | Conservar para laboratorio, métricas y diagnóstico de carga |
| [SiteOne](https://github.com/janreges/siteone-crawler) | Conservar para rastreo, SEO técnico e informes; también puede capturar con navegador |
| [Puppeteer](https://pptr.dev/guides/getting-started) | Primera opción para el colector propio: tamaños, DOM, acciones, capturas y PDF |
| [axe-core](https://github.com/dequelabs/axe-core) | Reglas de accesibilidad con elementos afectados y referencias; ejecutar en varias páginas y estados |
| [Playwright](https://playwright.dev/docs/accessibility-testing) | Alternativa a Puppeteer para emulación y flujos entre navegadores; elegir un controlador para el primer alcance |
| [Pa11y](https://github.com/pa11y/pa11y) | CLI con axe o HTML_CodeSniffer, acciones y capturas; útil como contraste. Usando axe no constituye una evaluación independiente de axe |
| [Screaming Frog](https://www.screamingfrog.co.uk/seo-spider/faq/) | Alternativa SEO especializada: edición gratuita hasta 500 URLs y renderizado JavaScript en la de pago; no imprescindible para comenzar |
| [Accessibility Insights](https://accessibilityinsights.io/docs/web/getstarted/fastpass/) | Revisión asistida de teclado, foco y comprobaciones manuales antes de la visita |
| [CrUX API](https://developer.chrome.com/docs/crux/api) | Datos agregados de usuarios reales cuando haya muestra; complemento opcional con clave |

Lighthouse ya incorpora axe. Las dependencias locales incluyen **axe-core 4.14.0**
y **puppeteer-core 25.16.0**. Usarlos directamente amplía páginas, estados y
evidencias, sin sumar dos veces las mismas incidencias. Al integrarlos se deben
declarar como dependencias directas y fijar versiones compatibles con Chrome.
`puppeteer-core` permite gestionar nuestro navegador sin descargar otro por
defecto. [Gestión del navegador](https://pptr.dev/next/guides/installation).

## Contenido de la nueva pestaña

La pestaña estará disponible aunque todavía no haya mediciones. Mostrará el
requisito de web confirmada y estados sin analizar, en cola, en curso, completo,
parcial y no disponible.

1. **Resumen para la visita:** tres a cinco problemas relevantes, fortalezas,
   alcance y recomendación provisional de arreglar, mejorar una parte o revisar
   la estructura. La gravedad técnica no determina por sí sola rehacer.
2. **Móvil y escritorio:** capturas con ancho y fecha, y observaciones por estado.
   Un diseño diferente en móvil puede ser correcto.
3. **Problemas y pruebas:** filtros por adaptación, accesibilidad, rendimiento,
   SEO y contacto; tarjetas con observación, URL, elemento, evidencia,
   consecuencia posible y solución.
4. **Qué mejorar:** acciones ordenadas por utilidad y alcance; esfuerzo y
   presupuesto revisados por una persona que conozca la base técnica.
5. **Dossier e histórico:** seleccionar hallazgos, revisar textos, exportar
   HTML/PDF y comparar mediciones compatibles.

Ejemplo **ilustrativo**, no resultado de un negocio auditado:

> **La carta se corta en pantallas pequeñas.** A 390 px, su contenedor ocupa
> 1.040 px y requiere desplazamiento lateral. La captura y el selector permiten
> reproducirlo. Esto dificulta consultar el menú desde el móvil. Propuesta:
> adaptar el contenedor y revisar tablas y tipografías de la carta.

La proporción de páginas afectadas siempre se refiere a la muestra: “3 de 5
páginas analizadas”, sin extrapolar al sitio entero o a todos los visitantes.

## Comprobaciones propuestas

Seleccionar inicialmente hasta cinco páginas del mismo sitio: inicio, contacto,
servicio principal y páginas relevantes, como carta, reserva, actividad, proyecto,
donaciones o inscripción de socios. Registrar URLs y estados que no se pudieron
medir. Permitir corregir la selección.

### Adaptación y uso

- Anchos de 320, 390, 768 y 1366 píxeles CSS, con altura, escala, User-Agent y
  emulación táctil registrados. Son perfiles en Chromium, no pruebas en un móvil
  físico ni garantías sobre Safari/iOS.
- Medir desbordamiento horizontal, contenido recortado y elemento causante;
  contrastar con captura. Comparar con `documentElement.clientWidth`, conservando
  también el viewport solicitado y `innerWidth`, que puede expandirse en móvil
  cuando el contenido se desborda.
- Comprobar menú y acciones de contacto en estados cerrado/abierto cuando se
  puedan identificar con seguridad. Que un botón esté bajo el primer pliegue no
  demuestra que sea inaccesible.
- Contrastar superposiciones mediante geometría y elemento que recibe la
  interacción; un carrusel, un mapa o un menú cerrado fuera de pantalla pueden
  ser intencionales y requieren tratar excepciones.
- Separar legibilidad y comodidad de infracciones normativas. “Texto menor de
  16 px” no constituye por sí solo una infracción WCAG.
- Revisar ampliación y reflow. Los 320 px son una referencia de reflow con
  excepciones, no una prohibición ciega de cualquier contenido ancho.
  [W3C: Reflow](https://www.w3.org/WAI/WCAG22/Understanding/reflow.html).

### Accesibilidad

Ejecutar axe sobre el DOM renderizado, con un conjunto declarado de reglas WCAG
2.2 A/AA y versiones registradas. Conservar violaciones, resultados incompletos,
reglas aplicables/no aplicables y revisión pendiente. Obtener etiquetas de
formulario, nombres accesibles, texto alternativo, contraste calculable, idioma
y estructura. Una imagen decorativa puede llevar `alt=""` correctamente.

Añadir revisión asistida de teclado, foco, trampas, menú, diálogos y lectura. Los
resultados automáticos no certifican conformidad completa ni permiten deducir
incumplimientos legales. [W3C: límites de las herramientas](https://www.w3.org/WAI/test-evaluate/tools/selecting/).

WCAG 2.2 AA contempla objetivos de 24×24 píxeles CSS y excepciones, como
separación y enlaces en línea. Un objetivo menor de 44 px no es automáticamente
una infracción AA. [W3C: tamaño mínimo](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum).

### Rendimiento

Usar Lighthouse con condiciones registradas. Para conclusiones relevantes del
dossier, repetir mediciones y mostrar mediana, rango y número de muestras de cada
métrica, respetando el presupuesto de ejecución. Una sola ejecución es exploratoria.
No promediar móvil y escritorio. [Variabilidad de Lighthouse](https://developer.chrome.com/docs/lighthouse/performance/performance-scoring/).

CrUX, si se configura, mostrará por separado percentil 75, periodo y granularidad
URL/origen. La falta de muestra significa “sin datos”. No sustituir INP por TBT ni
calcular pérdidas de ventas a partir de una puntuación. [CrUX: muestra y datos](https://developer.chrome.com/docs/crux/guides/crux-api).

Las acciones pueden abordar imágenes, recursos bloqueantes, terceros y estabilidad
según las pruebas. La lentitud por sí sola no atribuye el problema al alojamiento.

### SEO y acciones de negocio

Rastrear enlaces, HTTP, títulos, metadatos, directivas y canonical; contrastar el
HTML renderizado cuando JavaScript pueda cambiarlo. Distinguir “no lo detectamos”
de “no existe”. Un `noindex`, canonical externo o redirección puede ser intencional
y requiere contrastar su objetivo. JSON-LD bien formado no garantiza validez de
un resultado enriquecido.

Valorar claridad de servicios, zona, contacto, horarios, precios y reservas según
la actividad. Sin acceso autorizado a Search Console no se conoce completamente
indexación, consultas, clics y evolución. Las puntuaciones técnicas no garantizan
posiciones. [Google: experiencia y posicionamiento](https://developers.google.com/search/docs/appearance/page-experience).

Comprobar enlaces y campos de contacto no demuestra que un formulario entregue
mensajes. Las pruebas públicas no deben enviar correos, crear reservas, publicar
contenido o aceptar compras.

## Ajuste de calidad y prioridad

Conservar **observación, gravedad, confianza y revisión** por separado. La gravedad
del motor no se convierte automáticamente en urgencia comercial. Un enlace roto
de reserva puede ser más relevante que muchos avisos de metadatos.

- Versionar reglas, umbrales, páginas, perfiles y tiempos. No reescribir informes
  antiguos al cambiar una política.
- Agrupar por causa y contexto: una etiqueta ausente repetida en 20 páginas puede
  ser un defecto de plantilla con 20 apariciones. Mantener sus ocurrencias.
- Permitir confirmado, pendiente, descartado con motivo y resuelto. Cambios de
  dominio, elemento o condiciones pueden requerir nueva validación.
- Los bloqueos, errores y límites no cuentan como aprobados. Indicar si un tope
  recorta resultados.
- Comparar antes/después con la misma página, ancho, estado y entorno. Las
  diferencias entre móvil y escritorio no son una prueba de regresión.

Calibrar las reglas con una muestra revisada por una persona: medir falsos
positivos, problemas omitidos, reproducibilidad y utilidad para la visita. El
score no expresa probabilidad de venta.

## Dossier y recomendación comercial

Preparar resumen y tres a cinco hallazgos seleccionados con captura, fecha,
página, condiciones, explicación y siguiente acción. Incluir alcance y anexo
técnico. Diferenciar hallazgos revisados y sugerencias pendientes.

| Propuesta | Evidencia que puede sostenerla |
| --- | --- |
| Arreglar | Defectos concretos sobre una estructura útil: enlace, etiqueta, contraste o componente |
| Mejorar una parte | Problemas concentrados en móvil, carga, contacto, carta o plantilla |
| Rehacer | Problemas estructurales extendidos, necesidades no cubiertas y revisión técnica de la base existente |

Rehacer exige valorar alcance y alternativas con una persona. Copyright antiguo,
WordPress/Wix o puntuación baja no demuestran por sí solos la necesidad de sustituir
todo. Presupuesto y esfuerzo requieren conocer acceso, tecnología y necesidades.
La redacción puede usar plantillas; una IA sería opcional para explicar hallazgos
sustentados, sin inventar medidas, capturas o promesas de resultados.

## Integración por fases

1. Mejorar normalización y síntesis, y añadir la pestaña con estados y cobertura.
   Los informes anteriores sin evidencia se muestran con esa limitación; completarla
   requiere nueva medición.
2. Añadir colector Puppeteer + axe en el worker, con dependencias directas, perfiles,
   estados y capturas. Mantener proxy público, sandbox y un navegador/sitio cada vez.
3. Persistir selección y revisión; generar HTML autocontenido y PDF con capturas
   locales. Puppeteer permite [capturas](https://pptr.dev/guides/screenshots) y
   [PDF](https://pptr.dev/guides/pdf-generation).
4. Probar el flujo de producción en una muestra variada y contrastar manualmente.
   Añadir CrUX o varios navegadores cuando aporten información útil.

Añadir diagnóstico ligado a negocio, URL y generación, con esquema versionado.
Cada hallazgo necesita regla/origen, categoría, gravedad, confianza, revisión,
URL, perfil/estado, selector, DOM, captura, observado/esperado, fecha, recomendación
y ocurrencias. Registrar entorno, versiones, límites y cobertura. No mezclar
fechas y versiones incompatibles sin indicarlo.

Conservar compatibilidad con los cuatro informes actuales. Un trabajo nuevo
necesita progreso, reintento y presupuesto propios; hoy hay listas que esperan
exactamente dos motores y dos dispositivos.

Presupuesto inicial propuesto: hasta diez minutos por diagnóstico, configurable,
con resultados parciales persistidos. Reutilizar informes recientes compatibles
y priorizar páginas/repeticiones; no ejecutar todas las combinaciones sin medir
el coste. Validarlo con el límite actual de 1 GB del worker y su CPU asignada.

Ampliar explícitamente formatos admitidos, limpieza y rutas autenticadas para
capturas/PDF. El dossier debe conservar una instantánea para la visita; no puede
depender de imágenes que caduquen a los siete días en el servidor.

## Demostración local y aceptación

[`scripts/demo-diagnostico-web.mjs`](../scripts/demo-diagnostico-web.mjs) usa Chrome
con sandbox, Puppeteer y axe del worker. Bloquea peticiones externas y compara
dos versiones de HTML ficticio a 390 y 1366 px. Comprueba que solo la inicial
móvil desborda y que `label` y `button-name` desaparecen al corregir los controles.
Genera entorno/resultados JSON, cuatro capturas y HTML/PDF identificados como
demostración. No verifica webs de clientes ni certifica accesibilidad completa.

Resultado observado al ejecutar la demostración con el runtime instalado:

| Versión y ancho | Desbordamiento horizontal | Reglas con infracciones detectadas |
| --- | --- | --- |
| Inicial, 390 px | Sí | `button-name`, `label` |
| Inicial, 1366 px | No | `button-name`, `label` |
| Corregida, 390 px | No | Ninguna en el conjunto automático analizado |
| Corregida, 1366 px | No | Ninguna en el conjunto automático analizado |

Artefactos de esta ejecución: `/tmp/radar-diagnostico-demo-20261105/`, incluyendo
`resultado.json`, cuatro PNG, `diagnostico-demo.html` y `diagnostico-demo.pdf`.
El HTML incorpora las imágenes y no necesita acceso a webs externas. En el
primer ensayo se comprobó que `innerWidth` puede expandirse cuando una página
se desborda en móvil; el colector usa `clientWidth` y conserva ambas medidas.

Para repetirla, ejecutar el script desde un directorio accesible al usuario sin
privilegios que pueda usar Chrome. `RADAR_DEMO_RUNTIME` selecciona el runtime
(por defecto `/opt/radar-audit`), `RADAR_DEMO_CHROME` permite fijar el ejecutable
y `--output` la carpeta de resultados. No requiere cambiar servicios ni base de datos.

Aceptación de la futura pestaña:

- Incidencias sin duplicados entre motores, con evidencias y revisión persistente.
- Excepciones de carrusel, mapa y menú cerrado no generan fallos automáticos ciegos.
- Un resultado incompleto no equivale a aprobado y una falta de muestra no produce
  métricas inventadas.
- Inicio y contacto se comprueban aunque la portada pase Lighthouse; se informa
  del resto de la muestra.
- Exportación con capturas incorporadas; distingue demostración de auditoría real
  y funciona sin depender de una sesión pública de Radar.
- Histórico y revisión se conservan; cambiar la web impide atribuirle mediciones
  del dominio anterior.
- La recomendación de arreglar, mejorar o rehacer es revisable y explica su evidencia.
