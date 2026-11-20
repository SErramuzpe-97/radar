# Calibración de comparación SEO

La comparación ya está disponible como flujo manual y bajo demanda. No hay
descubrimiento ni rastreo masivo automático. El usuario debe confirmar cada
dominio antes de rastrearlo. La tarea T-039 sigue abierta hasta revisar evidencia
real etiquetada; compilar el código no demuestra precisión ni coste operacional.

## Muestra de revisión

Antes de ampliar el uso, revisar al menos 60 análisis completos, agrupados por
los casos siguientes (mínimo 10 por grupo; se pueden solapar):

1. Negocios locales pequeños y nombres ambiguos.
2. Cadenas y franquicias, incluidos negocios independientes que revenden marcas.
3. Negocios de localidades cercanas dentro del radio de candidatos.
4. Coincidencias por nombre/categoría que no compiten realmente.
5. Sitios bloqueados por robots, errores HTTP o cobertura desigual.
6. Fuentes contradictorias o identidad/actividad sin evidencia suficiente.

Para cada sugerencia, una persona registra si la identidad, localidad y
comparabilidad son correctas; si el dominio es independiente o de cadena; si
las citas respaldan la decisión; y si las mediciones enlazan a páginas
observadas. Revisar también todos los casos donde el modelo escogió una opción
distinta a “no comparable”, y una muestra de abstenciones.

## Indicadores y salida

Guardar número de casos revisados, precisión de sugerencias aceptadas,
competidores incorrectos, abstenciones, conflictos de fuentes, proporción de
sitios medidos/bloqueados, páginas visitadas por sitio, reintentos, uso y coste
devuelto por proveedor. Publicar la tabla junto a la fecha, versión del método y
la persona revisora. Los datos de tráfico, posiciones, backlinks y autoridad de
dominio no son parte de esta evaluación y no se deben inferir.

El límite de coste se fija a cero en cada petición; la cuota diaria compartida
se reserva antes de llamar al modelo. Verificar en ejecución agotamiento y
reanudación desde páginas ya guardadas. Si las etiquetas no alcanzan el umbral
de calidad acordado por el equipo, mantener o volver a desactivar el botón
manual con `RADAR_SEO_COMPARE_ENABLED=false` (el valor predeterminado es activo) hasta el siguiente despliegue.

## Estado actual

La revisión con muestra real, medición de abstenciones y coste real está
pendiente: no se han ejecutado llamadas con una clave de producción ni se han
revisado negocios reales para calibración. La clave proporcionada anteriormente
se debe rotar antes de usarse, porque se compartió en la conversación. La
implementación valida compilación Rust y empaquetado TypeScript; estas
comprobaciones no sustituyen esta revisión.
