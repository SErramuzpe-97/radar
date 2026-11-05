# Plan de mejora de Radar Navarra

El plan consolidado actualizado de rastreo, entidades, confirmación de webs,
diagnóstico/informes y cambios estéticos de ficha está en
[Plan de implementación del 5/11/2026](plan-implementacion-rastreo-diagnostico-ficha.md).
Ese documento concreta el orden de entregas y los criterios de aceptación de la siguiente ampliación.

**Fecha:** 04/11/2026  
**Objetivo de producto:** encontrar el mayor número posible de negocios navarros a los que ofrecer creación web, landing pages, SEO on-page y hosting; detectar qué servicio encaja, priorizar el contacto y hacer seguimiento en el CRM.

## Barrido inicial del proyecto

Este es un repaso de orientación, no una auditoría de código exhaustiva.

- El alcance descrito en el diseño ya dice “todos los municipios de Navarra”, pero la pantalla de búsqueda enviaba una lista de municipios concreta. La consulta OSM también estaba construida alrededor de `admin_level=8` (municipio).
- La auditoría de todos los registros llegó a mostrar exactamente 5.000 porque la consulta SQLite tenía un límite fijo. Además, los resultados se persistían al final, así que un fallo durante una ejecución larga podía perder el trabajo ya hecho.
- El proyecto ya dispone de ficha, mapa, filtros, priorización, pipeline comercial, registro de actividad, BORME y fuentes OSM. El siguiente salto de valor es cobertura y calidad de datos, trabajo reanudable y convertir las señales técnicas en oportunidades comerciales accionables.
- Se ha comenzado a implementar búsqueda regional, lotes controlados de auditoría y descarte reversible de negocios. El descarte conserva el registro para restaurarlo; no equivale a borrar datos.

## Fase 0 — Alcance de esta mejora

### Búsqueda de cobertura regional

- Una acción “Buscar en toda Navarra”, sin pedir al usuario escoger municipios.
- Consulta del área administrativa de Navarra y filtros por sector. Los municipios mencionados (Estella, Cizur, Noáin, Barañáin, Gares, Aoiz, Irurtzun y Pamplona) quedan incluidos por cobertura geográfica, junto con el resto de la comunidad.
- Para “todos los sectores”, ejecutar consultas sectoriales secuenciales con pausas para moderar carga de Overpass y evitar una única respuesta demasiado grande.
- Mantener el escaneo municipal interno para compatibilidad, pero no como flujo principal del usuario.

### Auditoría completa, gradual y recuperable

- Eliminar el límite implícito de 5.000; los límites que el usuario indique siguen siendo opcionales y explícitos.
- Auditar lotes de 100 registros con un máximo de 8 conexiones web simultáneas y 2 segundos entre lotes.
- Guardar cada lote y actualizar el progreso en SQLite antes de continuar, de forma que un reinicio no borre el trabajo ya completado.
- Excluir negocios descartados de las auditorías masivas. Permitir restaurarlos.

### Limpieza visual de la cartera

- Selección de varias filas, selección por página, descarte reversible y filtro para revisar descartados.
- Conservar los negocios descartados en la base de datos para evitar volver a importarlos como registros nuevos.

## Roadmap priorizado

### P0 — Completar y endurecer la cobertura de Navarra

1. Validar en OSM que la relación administrativa regional se resuelve correctamente y revisar la cobertura por sector.
2. Añadir informe de ejecución con negocios nuevos, ya existentes, descartados y fallos por sector.
3. Incorporar reintentos con backoff ante límites/errores temporales de Overpass; persistir cursores o checkpoints entre sectores.
4. Evitar trabajos regionales simultáneos que compitan por el mismo servicio externo.

**Aceptación:** una búsqueda regional llega a todos los sectores seleccionados, informa su resultado, y una interrupción permite continuar sin perder sectores completados.

### P1 — Calidad y amplitud de los leads

1. Registrar fuente, fecha y señales originales por atributo, no solo una fuente global.
2. Deduplicar por identificador OSM, dominio normalizado y similitud de nombre/dirección; presentar posibles duplicados para revisión.
3. Añadir cobertura por comarca/municipio como indicador de huecos, sin convertir el municipio en requisito de búsqueda.
4. Ampliar progresivamente fuentes abiertas compatibles (datos de aperturas, licencias, asociaciones, ONGs y cooperativas), con atribución y fecha de actualización. Incluir entidades no lucrativas con actividad verificada; distinguir el tipo de entidad de su rubro y no excluirlas automáticamente por falta de actividad mercantil. Fuentes y pruebas en [la investigación de cobertura](investigacion-fuentes-negocios.md#alcance-ampliado-asociaciones-ongs-y-cooperativas).
5. Revisar categorías OSM de comercio, oficinas, hostelería, servicios y talleres para mejorar recall y reducir objetos no comerciales.

**Aceptación:** panel de cobertura muestra dónde faltan datos y la importación no multiplica fichas del mismo negocio.

### P2 — Prioridad alineada con los servicios ofertados

1. Separar oportunidad por servicio: web nueva/landing, mejora SEO on-page y hosting/mantenimiento.
2. Explicar cada puntuación con señales observadas y nivel de confianza; “sin web en OSM” debe permanecer como hipótesis, no como hecho.
3. Calibrar los pesos con resultados reales del CRM (contactado, reunión, propuesta, venta/no encaje), en vez de tratar la puntuación heurística como probabilidad estadística.
4. Permitir ajustar reglas y umbrales sin recompilar.
5. Incorporar la pestaña **Diagnóstico web** con revisión de hallazgos, capturas por tamaño y estado, pruebas de accesibilidad y dossier HTML/PDF para propuestas comerciales. La investigación, prueba de viabilidad y criterios de aceptación están en [Diagnóstico web para propuestas](diagnostico-web-y-propuesta-comercial.md). Distinguir gravedad, confianza y cobertura; recomendar arreglar, mejorar o rehacer según evidencia y revisión humana.

**Aceptación:** cada ficha explica por qué se recomienda un servicio y el usuario puede corregir el encaje.

### P3 — Flujo comercial y productividad

1. Añadir próxima acción y fecha de seguimiento, recordatorios y vistas de “pendientes hoy”.
2. Preparar listas por ruta/día y exportaciones de contactos filtradas por prioridad y estado.
3. Añadir plantillas de contacto y paquetes comerciales por servicio, guardando el consentimiento/preferencia de contacto cuando corresponda.
4. Medir conversión por sector, servicio, fuente y prioridad para enfocar el esfuerzo donde haya más resultados.

### P4 — Operación y recuperación

1. Recuperar o marcar como interrumpidos los jobs que quedaron `en_curso` al reiniciar el proceso.
2. Añadir reintento de job, errores por registro y límites configurables de concurrencia, pausa y tamaño de lote.
3. Crear copia de seguridad SQLite y procedimiento documentado de restauración.
4. Mejorar logs y métricas de duración, throughput, cuota/errores de fuentes y salud de la base de datos.
5. Separar la compilación del frontend de `build.rs` para que generar OpenAPI o compilar Rust no dependa de reconstruir la UI.

### P5 — UX y documentación

1. Mostrar cobertura y trabajos activos en el panel, con progreso por sector y botón de actualización manual.
2. Mantener la tabla y el mapa sincronizados al archivar/restaurar y mostrar claramente filtros activos.
3. Documentar el flujo real de buscar en Navarra, auditar por lotes y recuperar descartados.
4. Mantener README, diseño y ayuda de la app alineados con las funciones implementadas.

## Decisiones y límites

- “Toda Navarra” es el alcance de descubrimiento; la prioridad por densidad puede seguir orientando las rutas, sin limitar la captura a Pamplona.
- Ninguna puntuación actual debe presentarse como una probabilidad real de venta hasta calibrarla con resultados observados.
- La cobertura depende de la calidad y licencia de las fuentes abiertas; no equivale a un censo oficial de todos los negocios.
- Los límites de lote y concurrencia son protecciones operativas iniciales y deben ajustarse usando métricas del servidor y respuestas de las fuentes.
