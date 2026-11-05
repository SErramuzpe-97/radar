import { useId, useState } from "preact/hooks";
import type { ComponentChildren } from "preact";

export function Chevron() {
  return <svg class="desplegable-chevron" viewBox="0 0 24 24" aria-hidden="true"><path d="m6 9 6 6 6-6"/></svg>;
}

export function Desplegable({
  titulo,
  children,
  className = "",
  contenidoClass = "",
  inicialAbierto = false,
  indicador,
}: {
  titulo: string;
  children: ComponentChildren;
  className?: string;
  contenidoClass?: string;
  inicialAbierto?: boolean;
  indicador?: ComponentChildren;
}) {
  const [abierto, setAbierto] = useState(inicialAbierto);
  const contenidoId = `desplegable-${useId()}`;
  return <section class={`desplegable ${className}`}>
    <button type="button" class="desplegable-titulo" aria-expanded={abierto} aria-controls={contenidoId} onClick={() => setAbierto(value => !value)}>
      <span>{titulo}</span>{indicador}<Chevron />
    </button>
    <div id={contenidoId} class={`desplegable-contenido ${abierto ? "abierto" : ""}`} aria-hidden={!abierto} inert={!abierto}>
      <div class={`desplegable-interior ${contenidoClass}`}>{children}</div>
    </div>
  </section>;
}
