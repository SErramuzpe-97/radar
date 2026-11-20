# Integración IA gratuita con OpenRouter

Estado: implementada y desplegada el 10 de noviembre de 2026. El servicio está
activo y la clave se autenticó correctamente; no se han enviado solicitudes de
inferencia.

## Capacidades

Radar usa OpenRouter tanto para buscar como para clasificar empresas. El endpoint
`parallel/decisions-fast:free` consulta la web actual y devuelve respuestas con
probabilidades por opción y citas. El borrador de diagnóstico usa `openrouter/free`
y solo recibe resultados de auditorías guardadas. No se usa Tavily ni el plugin
genérico de búsqueda web.

La ficha de cada negocio conserva categoría, web candidata, estado de actividad,
relación con cadenas y referencias. Una categoría automática solo completa un
rubro vacío o `otro`. El usuario puede persistir revisiones humanas para
categoría, actividad, organización y web; las correcciones de categoría prevalecen
y una revisión de actividad u organización impide que la IA vuelva a descartar el
negocio por ese dato.

La exclusión automática se limita a cierre permanente o cadena/franquicia. Requiere
probabilidad de la opción igual o mayor que 99 % y citas de al menos dos dominios
distintos. Un rechazo o corrección humana prevalece. El descarte conserva motivo y
respuesta de origen; la restauración manual queda protegida contra reevaluaciones.
El fallo HTTP, un cierre temporal, falta de información o conflicto no descartan.

Cada ejecución conserva etapa, huella de entrada, modelo solicitado y efectivo,
versiones de prompt/esquema/política, respuesta, referencias, uso, fecha y error.
Si cambian los datos de empresa, sus candidatas o revisiones mientras OpenRouter
procesa la petición, la respuesta se registra como obsoleta y no se aplica.

El borrador aparece en Diagnóstico web y en el dossier HTML/PDF. Se puede editar
y guardar una revisión humana; cada edición queda como versión separada y una
generación posterior no la sobrescribe.

## Política gratuita y credenciales

Búsqueda e informes comparten un límite local de 45 solicitudes diarias UTC. Las
peticiones reservan cuota antes de salir, incluso si luego fallan. La búsqueda
cacheada no consume una llamada nueva. Al agotar el límite, Radar pausa nuevas
llamadas hasta el día siguiente.

Las solicitudes fuerzan `provider.max_price` a cero en entrada, salida y precio
por solicitud, deshabilitan fallbacks y restringen proveedores con
`data_collection: deny`. La respuesta se rechaza si informa coste positivo o un
modelo distinto de la opción gratuita permitida. No hay compra, recarga ni ruta a
modelos de pago. La cuota local no consulta la cuota global de otras aplicaciones
de la misma cuenta.

La clave solicitada está instalada como `OPENROUTER_API_KEY` en `/etc/radar.env`
(propietario root, modo 600); no se guarda en el repositorio ni se envía al
navegador. La clave se validó con el endpoint de metadatos de OpenRouter, sin
realizar inferencia. Como se compartió en el chat, se puede rotar más adelante.
El saldo de cuenta no cambia el límite de precio de las peticiones.

## API e interfaz

- `GET /api/negocios/{id}/web`: candidatas, evaluación, citas, huella/estado y revisiones humanas.
- `POST /api/negocios/{id}/web/descubrir`: encola el descubrimiento individual.
- `POST /api/jobs/discovery`: encola descubrimiento por lotes. Usa la cola durable existente `jobs`/`job_items`.
- `PUT /api/negocios/{id}/ia-revisiones`: registra una corrección humana validada.
- `GET|POST|PUT /api/negocios/{id}/ia-informe`: consulta, genera y guarda una versión del informe.

Estas rutas quedan bajo la autenticación global de Radar. El contrato Rust/utoipa
genera `api/openapi.yaml` y el cliente TypeScript.

## Persistencia y límites

Las tablas `evaluaciones_ia`, `ia_ejecuciones`, `ia_revisiones`, `informes_ia` e
`informes_ia_revisiones` conservan el resultado actual, historial de llamadas,
revisiones humanas y versiones de texto. SQLite se actualiza con `CREATE TABLE IF
NOT EXISTS`; `web::init` agrega la columna de revisión humana cuando migra una base
anterior.

La evaluación de negocio corre dentro del trabajo durable de descubrimiento; no
se crea una cola paralela. La redacción de informe es una petición síncrona. Los
fallos quedan registrados, y los resultados que se vuelven obsoletos no modifican
la ficha.

La exclusión usa probabilidades y citas proporcionadas por el modelo. Dos dominios
distintos no demuestran por sí solos que las fuentes sean independientes; por eso
las decisiones se conservan con evidencia y pueden revertirse. Antes de usar el
flujo en producción, se debe medir precisión con una muestra etiquetada de casos
locales. No se ha realizado esa calibración ni una llamada real: la clave expuesta
requiere rotación y no se debe considerar validada la calidad empírica.

## Feature planificada: análisis SEO competitivo

**Estado:** implementación manual bajo demanda completada en `T-034`–`T-038`.
La calibración empírica `T-039` permanece en curso hasta revisar una muestra
real; el procedimiento está en [calibracion-seo-competidores.md](calibracion-seo-competidores.md).

### Objetivo y flujo

Contrastar una web confirmada con hasta cinco competidores locales comparables y
ampliar el diagnóstico SEO más allá de la puntuación OnPage. El proceso se lanza
bajo demanda desde Diagnóstico web. OpenRouter puede sugerir competidores, pero
la persona revisa, quita o añade dominios antes de medirlos.

1. Crear un análisis ligado al negocio, URL confirmada y huella de auditoría.
2. Proponer hasta cinco competidores por actividad, localidad y oferta; guardar
   nombre, dominio y citas actuales que justifican la comparación.
3. Mostrar la lista y pedir selección humana dentro del flujo, sin iniciar rastreos
   ni consumir más cuota hasta confirmar.
4. Reutilizar mediciones técnicas recientes del sitio propio. Para competidores,
   recopilar solo páginas públicas comparables y respetar robots, límites y las
   reglas de SSRF del cliente existente.
5. Calcular señales comparables con el mismo método y fecha; pedir a la IA una
   síntesis con diferencias, vacíos y acciones posibles, siempre enlazada a la
   evidencia guardada.
6. Mostrar resultados y fuentes en Diagnóstico y permitir incluirlos en el
   dossier como sección separada del diagnóstico técnico del negocio.

### Qué se comparará

- Cobertura de servicios, productos, preguntas y temas visibles en páginas
  públicas comparables; títulos, encabezados, metadatos, enlaces internos y datos
  estructurados observables.
- SEO local verificable en el propio sitio: identidad, localidad, datos de
  contacto, páginas de servicio/localidad y consistencia entre fuentes citadas.
- Rastreo e indexabilidad observable por URL pública: respuesta HTTP, robots,
  canonical, sitemap accesible, profundidad/enlaces detectados y cobertura del
  rastreo. Se informará qué páginas no se pudieron medir.
- Señales técnicas de la web propia y, cuando el coste/tiempo lo permita, del
  conjunto seleccionado: rendimiento, móvil y errores de auditoría ya cubiertos
  por Lighthouse/SiteOne.

Cada comparación distingue valores medidos, observaciones citadas y
recomendaciones IA. Usa la misma fecha de consulta y método entre dominios; si
no se puede medir un competidor, lo marca como incompleto en vez de completar con
una estimación.

### Límites de datos y afirmaciones

No se mostrarán rankings, tráfico, volumen de búsqueda, autoridad de dominio,
calidad/pérdida de backlinks ni posiciones históricas sin una fuente explícita
que los mida. Las menciones encontradas por búsqueda web no prueban enlaces
entrantes ni posición en Google. La IA no calculará esos valores a partir de la
calidad visual, Lighthouse, ni una respuesta del modelo.

OpenRouter Decisions Fast será el único servicio de investigación inicial y
seguirá limitado por el cupo gratuito compartido. El rastreo será local; no se
activarán plugins de búsqueda ni proveedores de pago. Las notas privadas del
CRM, contactos no públicos y credenciales nunca se envían.

### Frescura y relación con auditorías actuales

El despliegue de OpenRouter no invalida las auditorías existentes y no implica
volver a ejecutar todas las webs. El informe IA actual usa auditorías guardadas.
Para la nueva comparación, la auditoría propia se podrá reutilizar cuando
corresponda a la URL confirmada y tenga menos de 30 días; si no, solo se
reprogramará la medición del negocio seleccionado. El rastreo de competidores se
hará una vez por análisis y se guardará con fecha, huella y límites de cobertura.
Cambios de URL, selección o evidencia invalidan el análisis, no el historial.

### Aceptación

- La selección final incluye como máximo cinco competidores y requiere acción
  humana antes de iniciar su rastreo.
- Cada afirmación comparativa enlaza a una medición/captura guardada o a una
  fuente pública citada con fecha; no se inventan métricas externas.
- Se mide el mismo conjunto de señales y límites para todas las webs comparadas;
  se muestran bloqueos, omisiones, páginas no accesibles y cobertura desigual.
- La auditoría técnica actual no se borra ni se recalcula por defecto; solo se
  refresca para la web seleccionada si falta o excede la ventana de frescura.
- El análisis se cachea por selección, URLs, huellas y versiones de método/prompt;
  una edición de selección crea nueva versión y preserva el informe previo.
- Búsqueda y generación se cargan al mismo presupuesto local de 45 llamadas/día,
  con precio máximo cero y pausa segura al agotarlo.
- El dossier separa “Mediciones”, “Comparación pública” y “Recomendaciones IA”.

### Fases de trabajo

1. Contrato y persistencia de análisis, competidores, capturas/evidencias y
   estados de vigencia (`T-034`).
2. Descubrimiento y selección humana de competidores (`T-035`).
3. Recogida segura de páginas y mediciones homogéneas, con reutilización de
   auditorías recientes (`T-036`).
4. Comparativa SEO y resumen con referencias, límites y cobertura (`T-037`).
5. Flujo API, interfaz y exportación a dossier (`T-038`).
6. Calibración editorial, revisión de muestras, coste/cuota y criterios de
   activación (`T-039`).

## Comprobaciones realizadas

- `/root/.cargo/bin/cargo check --workspace`
- `cd radar-ui && npm run gen:api && npm run build`
- `./scripts/build-release.sh` compiló la interfaz; el binario Rust se compiló con `/root/.cargo/bin/cargo build --release -p radar-cli`.
- Reinicio de `radar.service`, HTTP local en puerto 7800 (`200`) y autenticación OpenRouter aceptada.
- `./scripts/tasks.sh validate`

No se ejecutaron pruebas ni solicitudes de inferencia. Se creó una copia previa de SQLite en `/var/lib/radar/backups/radar-predeploy-20261110T163649Z.sqlite` y del binario anterior en `/var/backups/radar-predeploy-20261110T163649Z`.

## Referencias técnicas

- [Decisions Fast gratuito](https://openrouter.ai/parallel/decisions-fast:free/)
- [API de Decisions](https://openrouter.ai/docs/api/api-reference/alphadecisions/submit-a-decisions-request)
- [Límites de precio y proveedores](https://openrouter.ai/docs/guides/routing/provider-selection)
