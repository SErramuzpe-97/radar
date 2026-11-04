import { useState } from "preact/hooks";
import { email, step, cargando, error, pedirCodigo, verificarCodigo } from "../stores/auth";

// Login OTP (ADR-005): email -> código de 6 dígitos enviado por email.
export function Login() {
  const [code, setCode] = useState("");

  return (
    <div class="login">
      <h1>Radar Navarra</h1>
      <p class="muted">Acceso restringido al equipo. Te enviaremos un código por email.</p>
      {step.value === "email" ? (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            pedirCodigo();
          }}
        >
          <label>
            Email
            <input
              type="email"
              required
              value={email.value}
              onInput={(e) => (email.value = (e.target as HTMLInputElement).value)}
              placeholder="tu@email.com"
            />
          </label>
          <button type="submit" disabled={cargando.value}>
            {cargando.value ? "Enviando…" : "Enviar código"}
          </button>
        </form>
      ) : (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            verificarCodigo(code);
          }}
        >
          <p class="muted">
            Código enviado a <strong>{email.value}</strong> (caduca en 10 min).
          </p>
          <label>
            Código de 6 dígitos
            <input
              type="text"
              inputMode="numeric"
              pattern="[0-9]{6}"
              maxlength={6}
              required
              value={code}
              onInput={(e) => setCode((e.target as HTMLInputElement).value)}
              placeholder="123456"
            />
          </label>
          <button type="submit" disabled={cargando.value}>
            {cargando.value ? "Verificando…" : "Entrar"}
          </button>
          <button type="button" class="secundario" onClick={() => (step.value = "email")}>
            Cambiar email
          </button>
        </form>
      )}
      {error.value && <p class="error">{error.value}</p>}
    </div>
  );
}
