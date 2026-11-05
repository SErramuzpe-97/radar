# Webs y auditoría técnica

La tarjeta comparte Ficha y Auditoría entre tabla y mapa. La cabecera y las pestañas permanecen visibles; cada tarjeta tiene su propio scroll. «Web no localizada» expresa una falta de información, no demuestra que el negocio carezca de web.

La propuesta de una nueva pestaña con diagnóstico transversal, capturas, revisión
de hallazgos y dossier comercial está en [Diagnóstico web para propuestas](diagnostico-web-y-propuesta-comercial.md).
Es una investigación y especificación; todavía no está integrada en la interfaz.

## Localización y confirmación

Radar conserva las URLs de las fuentes y protege las correcciones confirmadas frente a nuevas importaciones. El Registro de Turismo de Navarra puede aportar domicilio y localidad. Los campos que no permiten publicar sus datos se ignoran. Fuente: Gobierno de Navarra, Datos Abiertos, CC BY 4.0. Registro: https://datosabiertos.navarra.es/dataset/restaurantes-inscritos-en-el-registro-de-turismo-de-navarra

La consulta usa nombre y domicilio, con localidad como alternativa. La primera respuesta orgánica de Tavily se guarda como «Web sugerida · Sin confirmar», con consulta, proveedor, posición y fecha. Se excluyen resultados identificados como publicidad. No se confía en la posición para identificar al negocio. Las redes sociales y los directorios se señalan como tales. Solo una identidad estructurada del negocio, nombre y contacto/domicilio coincidentes permiten confirmación automática. Las sugerencias ambiguas no activan auditorías atribuidas al negocio.

En Ficha se puede abrir, confirmar o descartar una candidata e introducir una URL propia. La confirmación registra procedencia y fecha, recalcula el encaje y programa auditoría. Sin buscador configurado funcionan las fuentes públicas, la corrección manual y «Abrir búsqueda».

Para habilitar Tavily, el administrador añade `TAVILY_API_KEY` a `/etc/radar.env` (permisos 600) y reinicia radar. No introducir claves en el navegador, Git o mensajes. Se usa búsqueda básica, sin respuestas de IA, máximo 1.000 créditos por mes UTC y caché local de 30 días. Se reserva el crédito antes de consultar, se detiene al agotarlo y no se contrata un plan de pago. No reutilizar la misma cuenta para otros consumidores si se quiere que el límite local refleje el total de la cuenta.

## Rubros y objetivos comerciales

Los cinco sectores amplios se conservan. Cada negocio puede tener un rubro normalizado adicional para filtrar bodegas, restaurantes/carta, automoción, funerarias, floristerías, granjas escuela, guarderías, veterinarias, cuidado de mascotas, oficios, desatascos, plagas y turismo cultural. El valor `categoria` sigue guardando la etiqueta original de la fuente. En la búsqueda regional se informa el resultado desglosado por sector y rubro.

En lugares de visita, Radar conserva las etiquetas OSM de visitas, horario, tarifa y web registrada. Un precio explícito es una señal; una tarifa ausente significa “sin dato” y que OSM no tenga web no demuestra que el negocio carezca de ella. Tras una búsqueda web completada sin localizar sitio y con una tarifa declarada, Radar prioriza una posible landing de visitas, precio y reservas. En restaurantes se sugiere una página de carta/menú.

La lista inicial de nombres está en `data/objetivos.csv`. Tras un escaneo de Navarra, importar o actualizar sus coincidencias exactas con `radar --db /var/lib/radar/radar.db targets --csv data/objetivos.csv`; listar el estado con `radar --db /var/lib/radar/radar.db targets`. Los nombres sin coincidencia permanecen como objetivos pendientes, fuera de la cartera. Los casos ambiguos se resuelven solo después de revisar las fichas, usando la clave devuelta por el listado: `radar --db /var/lib/radar/radar.db target-resolve --clave CLAVE --negocio-id ID`. La asignación conserva la procedencia de la lista y no sobrescribe un rubro ya obtenido de una fuente.

El encaje provisional de creación web y landing es como máximo 40 mientras no se complete la búsqueda; puede llegar a 60 tras una búsqueda sin resultados. Siempre requiere confirmación. Constructor, copyright antiguo y fallos temporales no prueban abandono. La lentitud de laboratorio no atribuye por sí sola un problema al hosting.

## Motores y perfiles

- Lighthouse 13.6.0: cuatro categorías, LCP, CLS, FCP, Speed Index y TBT. Datos de laboratorio de una página, sin confundirlos con métricas de usuarios reales.
- SiteOne Crawler 2.5.1: SEO técnico por URL, títulos, descripciones, H1/encabezados, indexabilidad, canonical disponible, enlaces rotos, redirecciones y otras comprobaciones. Radar añade validación de canonical, hreflang y JSON-LD. Las directivas noindex pueden ser intencionadas y se presentan para revisión.

Móvil (390×844) y escritorio (1366×768) tienen ejecuciones independientes para cada motor. Lighthouse mantiene el perfil de carga correspondiente y SiteOne su agente de usuario. Se registran versiones, entorno, viewport y fecha. El SEO comercial se apoya principalmente en SiteOne; sus puntuaciones son diagnósticas, no probabilidades de venta. Search Console no se integra en esta auditoría pública: requiere acceso autorizado a cada propiedad.

Un sitio cada vez, pausa de 60 segundos entre sitios (`RADAR_TECH_PAUSE_SECONDS`), máximo 100 URLs, profundidad 3, un worker HTTP y una petición por segundo. El rastreo HTML tiene un presupuesto de 120 segundos por perfil; cuando faltan señales de HTML o solo hay una página, se comprueban hasta cinco páginas relevantes con JavaScript durante hasta 60 segundos (20 segundos por página). Lighthouse dispone de hasta 75 segundos por perfil. Cada fase tiene su propio margen de espera: 3 minutos para Lighthouse y 8 minutos para SiteOne. En SiteOne móvil se intentan capturas de hasta diez páginas a 390, 768 y 1366 px; si la página desborda a 390 px se añade la vista de 320 px. Una matriz interrumpida queda marcada como parcial. La cola revisa cada minuto las webs propias confirmadas y programa las que no se han auditado en las últimas 24 horas; puede ajustarse con `RADAR_TECH_REFRESH_HOURS` (mínimo seis horas). La ejecución sigue siendo de un sitio a la vez. Los informes indican cobertura, bloqueos y límites. Un informe parcial no equivale a un fallo de ejecución. Las fases con resultados completos, parciales o no disponibles se conservan al reintentar errores.

## Instalación y aislamiento

`scripts/install-audit-worker.sh` instala las dependencias fijadas y los servicios. Requiere Node 24, una copia instalada de Chrome for Testing y la descarga oficial de SiteOne. Consultar las variables y la comprobación SHA-256 del script. No usa servicios de pago ni funciones `--ai-*`, envío de emails o publicación de informes de SiteOne.

`radar-audit.service` se ejecuta como `radar-audit`, sin acceso a la base de datos ni a `/root`, con sistema de archivos protegido y un límite duro de 1 GB sin swap. El grupo compartido solo permite intercambiar solicitudes e informes en `/var/lib/radar-audits`. El proxy valida y fija IP públicas en cada conexión y rechaza credenciales, puertos alternativos, métodos de escritura y destinos locales. Chromium conserva su sandbox. El servicio restringe además el acceso a redes privadas; loopback IPv4 queda disponible para el controlador y el proxy. Los motores consultan páginas públicas sin autenticación y respetan robots; los sitios cuya política no puede comprobarse quedan no disponibles.

El servicio principal necesita el drop-in `deploy/radar-audit-spool.conf`. Ningún navegador recibe la clave Tavily, la sesión del usuario ni la base de datos.

## Operación y recuperación

```
systemctl status radar radar-audit
journalctl -u radar-audit -n 50
radar --db /var/lib/radar/radar.db web-confirm --id ID --url https://dominio.example
radar --db /var/lib/radar/radar.db audit-queue --id ID
radar --db /var/lib/radar/radar.db audit-queue --all
radar --db /var/lib/radar/radar.db discover-webs
```

Los comandos de programación CLI persisten trabajos. Auditorías técnicas se recogen continuamente; para un descubrimiento programado por CLI reiniciar el servicio principal para reanudarlo. La búsqueda individual desde la app comienza directamente.

Tras reinicio se recuperan solicitudes del worker y trabajos pendientes. Las cuatro fases se guardan en SQLite por negocio, URL, motor y dispositivo. Cambiar la web impide atribuir resultados antiguos a la nueva URL. El historial permanece consultable; los errores de ejecución pueden reintentarse desde Buscar empresas. Los hallazgos SEO no marcan el trabajo como error.

La cola técnica comprueba que el negocio siga siendo admisible y que la URL del trabajo
coincida con su web propia confirmada. Los trabajos pendientes de URLs antiguas o sin
confirmación pasan a error con un motivo explícito, conservando sus informes y progreso.
Se depuran en bloque: no consumen la pausa de un minuto entre sitios. La selección toma
el trabajo vigente más antiguo y valida de nuevo su URL antes de consultar robots y de
cada fase. Para una URL cambiada hay que confirmar la web actual y programar un trabajo
nuevo; reintentar el antiguo no lo convierte en una auditoría de la URL nueva.

Los originales HTML/JSON y las capturas sin seleccionar se comprimen, duran siete días y comparten un máximo global de 2 GB. Las capturas elegidas para una visita se protegen hasta 90 días mediante marcadores que respeta la limpieza automática; las siete fotos máximas de cada presentación descargada se incluyen en un HTML autónomo. Las capturas protegidas cuentan dentro de la cuota y pueden hacer que el directorio rebase el límite si se conservan muchas visitas. La descarga exige la sesión de Radar y usa adjuntos. Los resultados normalizados y la preparación de visita permanecen en SQLite. Respaldar SQLite con su API de backup (no copiar una DB activa a ciegas) y, si se desean originales, copiar también `/var/lib/radar-audits/artifacts` fuera del servidor. Un temporizador systemd crea una copia diaria SQLite en línea a las 03:15; conserva 14 días y no sobrescribe una copia del mismo día. Comprueba `systemctl list-timers radar-backup.timer` y `journalctl -u radar-backup.service`. Antes de restaurar: detener ambos servicios, conservar la DB actual y restaurar propietario/permisos; después arrancarlos y revisar los jobs.

Fuentes técnicas: https://github.com/janreges/siteone-crawler · https://developer.chrome.com/docs/lighthouse/overview · https://docs.tavily.com/documentation/api-reference/endpoint/search
