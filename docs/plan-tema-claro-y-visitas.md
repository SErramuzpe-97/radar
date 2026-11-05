# Plan: identidad clara y diagnóstico para visitas comerciales

Estado: propuesta de aplicación; este documento no cambia ni despliega la interfaz.
Referencia inspeccionada: https://www.estudiomaire.com/es/ y su CSS público
https://www.estudiomaire.com/_astro/_doc_.CkGNRwU7.css.

## Objetivo

Aplicar a Radar los colores y la tipografía de Estudio Maire sobre una interfaz
clara. Preparar una experiencia táctil que permita seleccionar evidencias antes
de una visita y enseñar al negocio una explicación breve, visual y verificable.

El recorrido principal será:

**Auditoría automática → biblioteca de capturas → revisión manual → preparar
visita → presentar → guardar próximos pasos.**

## 1. Identidad visual

### Valores comprobados y asignación propuesta

El CSS de referencia declara estos colores por función; no identifica dos
variables llamadas «primario» y «secundario». Se propone asignar carbón como
primario y verde grisáceo como secundario de Radar.

| Función en Radar | Color de referencia | Valor |
| --- | --- | --- |
| Primario: textos, acciones principales, iconos | ink / dark | `#1E2428` |
| Secundario: selección y acentos suaves | maire-300 | `#9CA59E` |
| Fondo general claro | paper | `#F6F5EC` |
| Tarjetas, formularios y paneles | surface | `#FBFAF3` |
| Fondos secundarios | maire-100 | `#E3E2D6` |
| Bordes y separadores | line / chip | `#D2D3C3` |
| Texto secundario legible | ink-muted | `#414D4D` |
| Detalles decorativos y estados inactivos | ink-faint | `#758080` |

La fuente comprobada es **Albert Sans**, con pesos 400–600. Se utilizarán 400
para lectura, 500 para controles y 600 para títulos y datos destacados, cargados
localmente desde una distribución autorizada con su licencia.

Aplicación:

- Fondo marfil continuo, paneles claros y texto carbón. Los botones principales
  serán carbón con texto marfil; los secundarios, claros con borde.
- Tipografía de cuerpo de 16 px en móvil/tablet, controles cómodos y títulos
  proporcionados al contenido. El panel de datos de escritorio puede conservar
  densidad con una escala específica.
- Centralizar los colores en variables y sustituir los valores oscuros escritos
  directamente en tablas, mapa, login, fichas, estados, carrusel y exportaciones.
  Corregir también los usos inconsistentes de `--border` frente a `--borde`.
- Usar los tonos semánticos presentes en el CSS de referencia para avisos,
  acompañados de texto e icono. El verde grisáceo de marca no indicará gravedad.
- Revisar contraste, foco visible y estados seleccionado/deshabilitado. Carbón
  sobre marfil ofrece aproximadamente 14,34:1; texto secundario sobre marfil,
  8,02:1. `#758080` sobre marfil da 3,72:1 y no se usará para texto normal.
- Aplicar la misma identidad al modo visita y a los documentos exportados.

## 2. Experiencia de visita

### Preparar y presentar

Añadir dos acciones al diagnóstico: **Preparar visita** y **Presentar**.

Preparar muestra la información de trabajo y la biblioteca completa. Presentar
abre una vista dedicada a pantalla completa dentro de la aplicación, con el
nombre y la web del negocio, las pruebas elegidas y navegación grande. La API
Fullscreen será una mejora opcional; la vista funcionará sin ella en iOS.

Las métricas e informes técnicos quedarán disponibles detrás de «Ver detalle».
La primera pantalla de la presentación explicará qué se ha revisado, cuándo y
qué páginas se han podido observar. Cada prueba tendrá tres textos editables:

1. **Qué vemos.** Una frase concreta junto a la captura.
2. **Qué puede dificultar.** Una consecuencia prudente y ligada a la evidencia.
3. **Qué proponemos.** Una acción que pueda discutirse en la reunión.

Ejemplo ilustrativo: «La carta se corta en esta pantalla» → «Cuesta consultar
los platos sin desplazarse lateralmente» → «Adaptar el contenedor de la carta».
No se atribuirán ventas perdidas ni una causa técnica que la evidencia no pruebe.

### Adaptación a cada dispositivo

| Dispositivo | Presentación |
| --- | --- |
| Tablet horizontal | Captura grande a la izquierda y explicación a la derecha; tira de pruebas elegidas abajo |
| Tablet vertical | Imagen protagonista y explicación breve debajo; notas ampliables |
| Móvil vertical | Una prueba por pantalla, título arriba y controles fijos abajo; acceso inmediato a ampliar |
| Móvil horizontal | Imagen ocupa casi todo el espacio; explicación plegable |

Las cuatro combinaciones son requisitos de la entrega: móvil vertical, móvil
horizontal, tablet vertical y tablet horizontal. La distribución responderá al
ancho y alto útiles, sin depender únicamente del modelo de dispositivo. En
pantallas bajas, los controles no deberán tapar la captura ni ocupar toda la
altura; la explicación podrá plegarse y el contenido seguirá siendo desplazable.
No se exigirá girar el dispositivo para usar ninguna función.

Al cambiar de orientación se conservarán la prueba seleccionada, las anotaciones
y el contexto de ampliación; se recalculará el encuadre para mantener visible la
zona señalada. Los diálogos y herramientas de edición también deberán caber en
las cuatro combinaciones, respetando las áreas seguras y las barras del navegador.

Elementos que harán útil la experiencia durante la conversación:

- **Recorrido de tres minutos:** preparar entre tres y siete pruebas, con orden
  manual y portada breve. La presentación puede terminar en una pantalla de
  próximos pasos acordados, sin generar automáticamente una propuesta comercial.
- **Señalar el fallo:** dibujar rectángulos, círculos, flechas y notas sobre la
  evidencia. Un control permite alternar original/marcada sin modificar el PNG.
- **Lupa de detalle:** abrir directamente el recorte del elemento problemático
  y volver al contexto de la página con un toque.
- **Comparar anchos:** alternar móvil/tablet/escritorio de la misma página y
  estado. En tablet horizontal, dos imágenes; en móvil, un selector alterna las
  imágenes. Ambas estarán etiquetadas con ancho y fecha.
- **Recorrido de cliente:** ordenar las pruebas por inicio → servicio/carta →
  contacto/reserva, además de poder ordenarlas por importancia.
- **Notas para quien presenta:** visibles solo al desplegarlas en preparación;
  el modo mostrado al cliente contiene exclusivamente los textos seleccionados.
- **Ampliación táctil:** pellizcar, doble toque y arrastrar. Deslizar cambia de
  prueba cuando la imagen está ajustada; al ampliarla, arrastrar mueve la imagen.
  Siempre habrá botones equivalentes y soporte de teclado.
- Controles con objetivos cómodos de al menos 44–48 px, navegación por progreso
  y restauración de la prueba actual al girar la pantalla. Soportar áreas seguras
  y altura dinámica del navegador.

## 3. Más páginas y mejores capturas

### Situación actual comprobada

`responsiveAudit()` captura como máximo cinco páginas en cuatro anchos: 320,
390, 768 y 1366 px. Son capturas del viewport, no de la página completa. La matriz
se ejecuta dentro de SiteOne móvil y comparte su tiempo restante. La interfaz
carga las imágenes juntas y el dossier exporta todas las capturas disponibles.

### Captura en dos niveles

**Auditoría habitual:** ampliar la muestra hasta diez páginas distintas del mismo
sitio, priorizando inicio, contacto, servicio/producto, carta/precios, reserva o
cita y otras páginas relevantes para el negocio. Evitar gastar la muestra en
URLs equivalentes, parámetros repetidos y páginas legales duplicadas.

- Base propuesta: 390, 768 y 1366 px para cada página: hasta 30 vistas.
- Añadir 320 px cuando haya señales de desbordamiento o se solicite expresamente.
- Capturar el primer viewport de todas las páginas medidas y guardar dimensiones,
  URL final, fecha, estado de carga y relación con los hallazgos.
- Priorizar una primera pasada de las diez páginas en móvil antes de completar
  los otros anchos, para que un límite de tiempo no deje toda la cobertura en
  una sola URL.
- Incorporar hasta una vista completa acotada por página y recortes de elementos
  relevantes, con límites de altura y tamaño. Generar miniaturas para la biblioteca.

**Ampliación para visita:** el usuario marca URLs adicionales o elige «Capturar
esta página». Proponer lotes de hasta veinte páginas en total por preparación,
con recortes o estados extra donde aporten evidencia. Publicar avance por página
y conservar las capturas terminadas si se agota el tiempo.

El colector de evidencias tendrá tareas reanudables separadas del tiempo restante
de SiteOne; añadirá trabajos a la misma infraestructura de cola, con prioridad
para preparaciones solicitadas por el usuario y capacidad reservada a auditorías
habituales para evitar que una carga deje a la otra esperando indefinidamente.
Mantener un solo navegador pesado activo en el servidor actual y medir consumo
antes de ampliar concurrencia.

Esperar a fuentes e imágenes con límites explícitos y registrar cargas
incompletas. Para capturas largas, hacer desplazamiento progresivo limitado para
recursos diferidos. Si la espera no termina, guardar la evidencia como incompleta.
Los estados de menú solo se abrirán mediante acciones inequívocas o una selección
manual. Formularios, reservas y compras no se enviarán.

No todos los fallos son visibles en una foto: para SEO, rendimiento y otras
mediciones, presentar una tarjeta de evidencia con dato, URL, fecha y alcance,
en vez de adjudicar el problema a una captura sin relación.

## 4. Selección manual de fotografías

Crear una **biblioteca por negocio**, con filtros por página, dispositivo,
auditoría y estado de revisión.

Cada captura podrá quedar en:

- **Pendiente:** todavía no revisada.
- **Para enseñar:** elegida manualmente para la visita.
- **Descartada:** apartada de la biblioteca principal y de la presentación.

La revisión tendrá botones claros «Conservar» y «Descartar», selección múltiple y
atajos táctiles opcionales. Descartar será reversible, con «Deshacer» y una vista
de descartadas. La eliminación física definitiva será una acción diferente.

La aplicación podrá destacar posibles problemas, pero ninguna imagen entrará en
la presentación sin selección expresa. Anotar, titular, relacionar con un
hallazgo y ordenar las elegidas; ofrecer botones de mover además de arrastrar.

Guardar selección y anotaciones en el servidor para continuar en tablet lo que
se preparó en ordenador. Persistir identificador del negocio, auditoría y captura,
estado, orden, texto, coordenadas normalizadas y autor/fecha de la edición.

La presentación será una versión guardada del dossier. Las auditorías automáticas
nuevas no cambiarán sus fotografías ni el orden. «Actualizar evidencias» permitirá
compararlas y elegir cuáles sustituir. Una captura descartada de una auditoría
anterior no implicará descartar automáticamente una captura nueva.

## 5. Conservación y preparación sin conexión

Actualmente los originales caducan a los siete días y existe una cuota global
de 2 GB. Ampliar capturas sin cambiar esta política podría dejar una presentación
sin imágenes al llegar al negocio.

- Mantener la limpieza habitual para archivos no seleccionados.
- Conservar en un espacio protegido las imágenes incluidas en una versión de
  visita durante 90 días como propuesta inicial, con renovación explícita.
- Usar referencias por dossier para que eliminar un archivo de una biblioteca
  no rompa otra presentación que lo utiliza. Mantener cuota y tamaño visibles.
- Añadir «Preparar para llevar»: descargar solo la versión elegida y sus pruebas,
  comprobar integridad y mostrar «Lista sin conexión», tamaño y fecha.
- Usar almacenamiento local por cuenta para ese paquete; limpiar al cerrar sesión
  o eliminar la visita. Fuentes, interfaz mínima e imágenes deben estar incluidas.
- Comprobar disponibilidad real antes de entrar en presentación: no confundir
  metadatos guardados con fotografías descargadas. Si el navegador elimina su
  almacenamiento, retirar la indicación de disponibilidad.
- Durante la reunión, registrar decisiones y notas en local; sincronizar al
  volver la conexión sin sobrescribir silenciosamente otra edición.
- Exportar HTML autocontenido y PDF únicamente con las pruebas elegidas, con la
  misma identidad visual. El HTML será un respaldo portátil adicional.

## 6. Orden de aplicación

| Etapa | Entrega concreta | Condición para avanzar |
| --- | --- | --- |
| 1. Tema claro | Variables, Albert Sans, componentes y documentos adaptados | Legibilidad y estados comprobados en las pantallas principales |
| 2. Selección persistente | Biblioteca, conservar/descartar/deshacer, orden y notas | Una selección hecha en ordenador se recupera en tablet |
| 3. Evidencia ampliada | Diez páginas, cola reanudable, miniaturas y conservación protegida | Una interrupción conserva el avance y no rompe visitas guardadas |
| 4. Modo visita | Vistas tablet/móvil, ampliar, señalar, comparar y recorrido | Se puede enseñar una visita completa con una mano y sin interfaz administrativa |
| 5. Preparar para llevar | Paquete sin conexión, exportaciones y sincronización | La visita elegida abre y se recorre en modo avión |

La primera entrega utilizable en visita será la etapa 4 usando conexión. La
etapa 5 completa el uso en locales sin buena cobertura. La selección de la etapa
2 puede trabajar con las capturas actuales mientras se construye la ampliación.

## 7. Encaje técnico y comprobación

- `radar-ui/src/styles.css`: variables y sustitución de valores oscuros.
- `DiagnosticoWeb.tsx`: separar biblioteca, editor y presentación en componentes
  propios para evitar concentrar todos los estados en una sola vista.
- `FichaNegocio.tsx` y `app.tsx`: acceso al modo visita y vista dedicada que ocupe
  el área útil de tablet/móvil.
- `audit-worker/worker.mjs`: extraer colector de capturas, límites por tarea y
  escritura incremental; actualizar limpieza para respetar evidencias conservadas.
- Backend y SQLite: modelos de captura/revisión/dossier/versión; endpoints
  autenticados de selección, anotaciones, preparación y descargas; migraciones y
  actualización del contrato OpenAPI y su cliente generado.
- Rutas de archivos: ampliar la validación actual de `responsive-N-ancho` para
  representar miniaturas, vistas completas, estados y recortes con identificadores
  registrados. No aceptar rutas proporcionadas directamente por el navegador.
- Cliente sin conexión: service worker y almacenamiento IndexedDB acotado a las
  visitas descargadas, con eliminación de datos y gestión de cuota.

Comprobar el tema y la visita a 390, 768, 1024 y 1366 px; Safari en iPad/iPhone y
Chrome en Android; orientación vertical/horizontal, teclado, foco y zoom.
Incluir expresamente parejas de tamaños útiles de referencia: móvil 390×844 y
844×390, tablet 768×1024 y 1024×768, además de una tablet de mayor tamaño. Revisar
las cuatro combinaciones tanto en preparación como en presentación: sin controles
recortados, sin desplazamiento horizontal de la interfaz, con textos legibles y
sin pérdida de estado al girar. El desplazamiento dentro de una captura ampliada
será independiente del de la interfaz.
Verificar persistencia entre dispositivos, deshacer descartes, versiones que no
cambian con una auditoría automática, limpieza que no borra imágenes protegidas,
interrupciones del worker y apertura sin conexión. Medir memoria, tiempo por
sitio y crecimiento de almacenamiento con la matriz ampliada antes de activar
el nuevo volumen para toda la cola automática.
