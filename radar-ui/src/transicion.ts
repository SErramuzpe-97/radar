export function transicionar(cambio: () => void) {
  const doc = document as Document & { startViewTransition?: (update: () => void) => unknown };
  if (doc.startViewTransition && !window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
    try { doc.startViewTransition(cambio); }
    catch { cambio(); }
  } else cambio();
}
