# Calidad de datos y reconstrucción

Radar admite actividad comercial de Navarra antes de guardar negocios. Un nombre y un
punto en el mapa no demuestran actividad comercial. La política se aplica a OSM regional,
Overpass municipal, páginas estructuradas y constituciones/CSV del BORME.

## Método por evidencias

1. Comprobar categoría comercial, actividad vigente y pertenencia a Navarra. Excluir
   infraestructura, barrios, accesos, equipamientos públicos, locales vacíos y cierres.
2. Conservar marca, operador, identificadores de organización y relaciones de pertenencia.
   Consultar datos públicos de Wikidata por identidad, con caché de 30 días, consultas
   secuenciales y una pausa de un segundo entre peticiones. Los nombres sin resolver
   se guardan 24 horas y nunca demuestran independencia. Las coincidencias por nombre
   requieren contrastar el dominio oficial antes de utilizarse para una exclusión.
3. Considerar gran red con gestión digital corporativa cuando coinciden evidencia de
   cadena/red, tamaño documentado (al menos 250 empleados o 20 sucursales) y web oficial.
   También se contrasta la titularidad del dominio oficial compartido. La dimensión procede de cantidades estructuradas y se toma la observación fechada más
   reciente. También se recorren organizaciones matrices hasta dos niveles.
4. Un grupo explícito se admite como pequeño cuando hay evidencia documentada de menos
   de 20 sucursales y menos de 250 empleados. Si faltan datos, queda pendiente fuera de
   los resultados principales. Las actividades locales sin vínculos corporativos detectados
   se admiten, con esa limitación expresada en el motivo.

No hay nombres de empresas en las reglas ni excepciones por marca. Los umbrales son una
política operativa de prospección, no una estimación de ventas ni una certificación del tamaño
real de una empresa. Los datos incompletos o incorrectos pueden exigir un nuevo contraste.
Una marca o una web por sí solas no provocan exclusión por gran cadena. Las coincidencias de identidad con organizaciones grandes descubiertas durante el análisis quedan pendientes cuando falta corroboración, para resolver posibles homónimos sin darlos por cadenas.

Las decisiones conservan fuente, identidad, fecha, versión de política y evidencias en
`clasificacion_ingesta`. Los vínculos con entidades externas mantienen su URL. La pantalla
Fuentes muestra admitidos, excluidos, pendientes y motivos; estos candidatos no contaminan
la tabla de negocios. Las futuras reevaluaciones también desactivan registros ya admitidos.

## Reconstrucción local

Esta operación borra resultados, auditorías, actividades, seguimiento y borradores ligados a
negocios. Conserva fuentes, plantillas, accesos, sesiones y la clave de firma de sesión.

```sh
sudo systemctl stop radar
sudo -u radar /usr/local/bin/radar --db /var/lib/radar/radar.db rebuild --reset-results --audit
sudo systemctl start radar
```

El comando se niega a operar si el servidor nuevo tiene bloqueada la base. Crea una copia
completa independiente y comprueba `PRAGMA integrity_check` antes del borrado. La limpieza y
la creación del trabajo se confirman juntas. Los IDs de negocios no se reciclan: un navegador
con una ficha anterior no puede modificar por accidente otro negocio recién descubierto.

El servidor recupera la reconstrucción en el siguiente arranque: procesa OSM Navarra, BORME
y semillas/buscadores habilitados con importador compatible; después audita los admitidos.
Las fuentes sin importador quedan identificadas en el resumen. El fallo de una fuente
secundaria queda visible mientras la auditoría de los datos disponibles puede continuar.
Si falla OSM, la reconstrucción se detiene con error. Reintentar conserva las fases terminadas,
reanuda la cola y audita únicamente registros sin auditoría. Nunca repite el borrado.

Los valores de auditoría siguen siendo 100 negocios por lote, 8 simultáneos y 2 segundos entre
lotes; se configuran con `RADAR_AUDIT_BATCH_SIZE`, `RADAR_AUDIT_CONCURRENCY` y
`RADAR_AUDIT_PAUSE_MS`. Desde la CLI se puede validar solo el escaneo con `scan --navarra`.

## Recuperación

Si hace falta volver al estado anterior, detener el servicio, conservar la base actual y
restaurar el archivo `backups/radar-before-rebuild-*.sqlite`. Restaurar también el binario
anterior si corresponde a un esquema previo; comprobar integridad y reiniciar. Copiar las
copias a otra máquina para protegerlas frente a pérdida del servidor.

## Interfaz

Tabla y mapa usan el espacio disponible del navegador. Los filtros, encabezados y paginación
no se desplazan con las filas. La ficha tiene scroll interior independiente y se superpone en
pantallas menores de 900 px. Escape o Cerrar devuelve a la lista/mapa.

Los servicios son badges informativos con puntuación a partir de 45/100. Seguimiento permite
cambiar estado y registrar actividades; no incluye planificación ni borradores dentro de la
ficha. El color completo de cada marcador expresa potencial: verde ≥70, ámbar ≥50 y gris
para valores inferiores o desconocidos. El icono identifica el sector.
