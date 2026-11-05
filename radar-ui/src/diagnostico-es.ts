type Hallazgo = {
  code?: string;
  title?: string;
  details?: string;
  recommendation?: string;
  category?: string;
  severity?: string;
};

export const CATEGORIAS_ES: Record<string, string> = {
  accessibility: "Accesibilidad",
  seo: "Visibilidad en buscadores",
  performance: "Rendimiento",
  "best-practices": "Buenas prácticas",
  responsive: "Diseño móvil",
  contact: "Contacto",
  security: "Seguridad",
  availability: "Disponibilidad",
  "meta-data": "Información de la página",
  "page-quality": "Calidad de la página",
};

export const GRAVEDAD_ES: Record<string, string> = {
  critical: "Crítica",
  high: "Alta",
  medium: "Media",
  low: "Baja",
  info: "Informativa",
  informative: "Informativa",
};

const AUDITORIAS: Record<string, [string, string, string]> = {
  "largest-contentful-paint-element": ["El contenido principal tarda en aparecer", "La parte más importante de la página puede tardar en mostrarse, sobre todo con una conexión lenta.", "Optimizar la imagen o el bloque principal y reducir el tiempo de respuesta del servidor."],
  "render-blocking-resources": ["Hay recursos que retrasan la página", "Algunos archivos hacen que el contenido tarde más en empezar a mostrarse.", "Cargar primero los estilos esenciales y aplazar los recursos que no se necesitan al inicio."],
  "uses-optimized-images": ["Hay imágenes que se pueden optimizar", "Algunas imágenes podrían ocupar menos espacio sin perder calidad visible.", "Reducir su peso y servirlas en formatos modernos y al tamaño necesario."],
  "uses-responsive-images": ["Las imágenes no se adaptan a cada pantalla", "En algunos dispositivos se podrían descargar imágenes mayores de lo necesario.", "Ofrecer tamaños de imagen adecuados para móvil, tablet y escritorio."],
  "image-delivery-insight": ["La carga de imágenes se puede mejorar", "La forma de entregar algunas imágenes puede ralentizar la página.", "Revisar el formato, el tamaño y la carga diferida de las imágenes."],
  "uses-text-compression": ["Falta compresión en algunos archivos", "Algunos recursos se transfieren con más datos de los necesarios.", "Activar compresión Brotli o Gzip en el servidor."],
  "uses-long-cache-ttl": ["Los archivos podrían aprovechar mejor la caché", "El navegador puede volver a descargar archivos que apenas cambian.", "Configurar tiempos de caché largos para archivos con nombre versionado."],
  "unused-javascript": ["Se descarga código JavaScript que no se utiliza", "Parte del código descargado no se necesita para mostrar esta página.", "Eliminar código sobrante o cargarlo solo en las páginas que lo usan."],
  "unused-css-rules": ["Hay estilos que no se utilizan", "La página descarga reglas de diseño que no necesita en esta vista.", "Dividir o depurar las hojas de estilo para cargar solo lo necesario."],
  "server-response-time": ["El servidor tarda en responder", "La página empieza a cargarse después de una espera del servidor.", "Revisar el alojamiento, la caché y las consultas que preparan la página."],
  "font-display": ["La tipografía puede retrasar el texto", "El texto podría tardar en aparecer mientras se descarga una fuente.", "Configurar font-display: swap y servir fuentes optimizadas."],
  "color-contrast": ["Hay texto con poco contraste", "Algunas personas pueden tener dificultad para leer ciertos textos.", "Aumentar la diferencia de contraste entre texto y fondo."],
  "image-alt": ["Hay imágenes sin descripción alternativa", "Los lectores de pantalla pueden no comunicar el contenido de algunas imágenes.", "Añadir texto alternativo útil a las imágenes informativas."],
  "button-name": ["Hay botones sin nombre accesible", "Algunas tecnologías de asistencia no pueden explicar qué hace un botón.", "Añadir una etiqueta visible o un nombre accesible que describa su acción."],
  "link-name": ["Hay enlaces sin texto descriptivo", "El propósito de algunos enlaces puede no quedar claro al recorrerlos con un lector de pantalla.", "Usar un texto de enlace que describa el destino o la acción."],
  "document-title": ["La página no tiene un título claro", "El título ayuda a identificar la página en el navegador y en los resultados de búsqueda.", "Añadir un título único y descriptivo."],
  "meta-description": ["Falta una descripción para buscadores", "El resultado de búsqueda puede mostrarse sin una descripción útil.", "Añadir una descripción breve y específica para esta página."],
  "html-has-lang": ["No se ha indicado el idioma de la página", "Los navegadores y lectores de pantalla pueden no identificar correctamente el idioma.", "Declarar el idioma principal en el elemento HTML."],
  "tap-targets": ["Algunos controles son difíciles de pulsar", "En pantallas táctiles puede costar seleccionar controles demasiado pequeños o juntos.", "Aumentar su tamaño y separación en móvil."],
  viewport: ["La vista móvil no está configurada correctamente", "La página podría no ajustarse al ancho del dispositivo.", "Configurar la etiqueta viewport para adaptar el diseño a la pantalla."],
  "is-on-https": ["La página no usa una conexión segura", "El navegador puede mostrar avisos y la conexión no protege adecuadamente los datos.", "Configurar HTTPS y redirigir el tráfico HTTP."],
  "crawlable-anchors": ["Hay enlaces que los buscadores no pueden seguir", "Algunos enlaces pueden no ayudar a descubrir otras páginas del sitio.", "Usar enlaces HTML con destinos claros y rastreables."],
  "title-missing": ["Falta el título de la página", "El título ayuda a visitantes y buscadores a entender el contenido.", "Añadir un título único que describa la página."],
  "description-missing": ["Falta la metadescripción", "El resultado de búsqueda puede perder contexto útil para quien lo ve.", "Escribir una descripción breve y específica."],
  "h1-missing": ["No se detectó el encabezado principal", "La página puede no tener un título principal claro para visitantes y buscadores.", "Añadir un encabezado principal que describa el contenido."],
  "indexing-review": ["Conviene revisar si la página se puede indexar", "Las directivas observadas podrían impedir que la página aparezca en buscadores.", "Comprobar robots.txt y las etiquetas de indexación antes de hacer cambios."],
  "canonical-external": ["La página señala una URL canónica externa", "Los buscadores podrían atribuir esta página a otra dirección.", "Confirmar que la URL canónica externa es intencional."],
  "canonical-invalid": ["La URL canónica no es válida", "La dirección preferida de la página puede no poder interpretarse correctamente.", "Corregir la URL canónica para que sea absoluta y válida."],
  "heading-structure": ["La jerarquía de encabezados necesita revisión", "Los encabezados pueden no reflejar bien la estructura del contenido.", "Ordenar los encabezados para que describan secciones y subsecciones."],
  "non-unique-titles": ["Hay títulos repetidos", "Varias páginas comparten el mismo título y pueden ser difíciles de distinguir.", "Escribir títulos únicos para cada página."],
  "non-unique-descriptions": ["Hay metadescripciones repetidas", "Varias páginas comparten la misma descripción en buscadores.", "Redactar una descripción propia para cada página importante."],
  "redirect-review": ["Se observó una redirección que conviene revisar", "La página envía a otra dirección; puede ser correcto o indicar un destino mejorable.", "Confirmar que el destino final es el esperado y evitar cadenas innecesarias."],
  "broken-link": ["Se encontró un enlace roto", "Un enlace lleva a una página que no responde correctamente.", "Actualizar el enlace o restaurar la página de destino."],
  "server-error": ["El servidor devolvió un error", "Al menos una página respondió con un error del servidor.", "Revisar la página y los registros del servidor para identificar la causa."],
  "hreflang-invalid": ["La etiqueta de idioma alternativo no es válida", "Los buscadores pueden no entender a qué idioma o región corresponde la versión indicada.", "Corregir el código de idioma y región de hreflang."],
  "jsonld-invalid": ["Los datos estructurados no son válidos", "Los buscadores pueden ignorar la información estructurada de esta página.", "Corregir el JSON-LD y validarlo antes de publicarlo."],
};

export function categoriaEs(code?: string): string {
  return CATEGORIAS_ES[code ?? ""] ?? "Revisión técnica";
}

export function gravedadEs(severity?: string): string {
  return GRAVEDAD_ES[severity ?? ""] ?? "Informativa";
}

export function traducirHallazgo(issue: Hallazgo) {
  const translated = AUDITORIAS[issue.code ?? ""];
  return {
    titulo: translated?.[0] ?? (translatedTitle(issue.title) ?? `Revisión técnica: ${categoriaEs(issue.category).toLowerCase()}`),
    explicacion: translated?.[1] ?? "El análisis automático detectó un punto que conviene revisar en esta página.",
    siguientePaso: translated?.[2] ?? "Contrastar el detalle técnico y decidir si requiere un cambio.",
    original: issue.title || issue.details || issue.recommendation || issue.code || "",
  };
}

function translatedTitle(title?: string): string | undefined {
  if (!title) return undefined;
  // Lighthouse ya genera HTML localizado; algunos títulos normalizados llegan en inglés.
  const exact: Record<string, string> = {
    "Document has a <title> element": "La página tiene un título",
    "Links do not have descriptive text": "Hay enlaces sin texto descriptivo",
    "Image elements do not have [alt] attributes": "Hay imágenes sin descripción alternativa",
    "Background and foreground colors do not have a sufficient contrast ratio.": "Hay texto con poco contraste",
  };
  return exact[title];
}

export function etiquetaMetricaEs(code: string, label?: string): string {
  const labels: Record<string, string> = {
    "largest-contentful-paint": "Elemento principal visible (LCP)",
    "cumulative-layout-shift": "Estabilidad visual (CLS)",
    "first-contentful-paint": "Primer contenido visible (FCP)",
    "speed-index": "Velocidad de carga visual",
    "total-blocking-time": "Tiempo de bloqueo total",
    seo: "Visibilidad en buscadores",
    performance: "Rendimiento",
    accessibility: "Accesibilidad",
    "best-practices": "Buenas prácticas",
  };
  return labels[code] ?? (label && !/[a-z]{3,}/i.test(code) ? label : "Indicador técnico");
}

export function estadoAuditoriaEs(state?: string): string {
  const labels: Record<string, string> = {
    completo: "Completo", parcial: "Parcial", pendiente: "En cola", en_curso: "En curso",
    error: "Con incidencias", no_disponible: "No disponible", no_lanzada: "No iniciada",
  };
  return labels[state ?? ""] ?? "Estado no disponible";
}

export function errorAuditoriaEs(error: unknown): string {
  const message = String(error ?? "").replace(/^Error:\s*/, "").trim();
  if (/^(Selecciona|El navegador bloqueó|Conserva|Una presentación|Captura|No se pudo|El informe original|El HTML original)/i.test(message)) return message;
  if (/401|403|unauthorized|forbidden/i.test(message)) return "Tu sesión no permite cargar este informe. Vuelve a iniciar sesión e inténtalo de nuevo.";
  if (/404|not found/i.test(message)) return "El informe solicitado ya no está disponible.";
  if (/timed?\s*out|timeout|presupuesto del sitio|did not finish within/i.test(message)) return "El motor agotó el tiempo disponible para revisar esta web. Puedes volver a intentarlo más tarde.";
  if (/failed to fetch|networkerror|connection/i.test(message)) return "No se pudo conectar con el servicio de auditoría. Comprueba la conexión e inténtalo de nuevo.";
  return "No se pudo cargar el informe de auditoría. Inténtalo de nuevo.";
}
