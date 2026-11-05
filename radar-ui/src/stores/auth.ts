import { signal, computed } from "@preact/signals";
import { api, authHeaders } from "../api/client";

// Estado de autenticación OTP (ADR-005): email -> código -> sesión JWT.
export const token = signal<string | null>(localStorage.getItem("radar_token"));
export const email = signal<string>("");
export const step = signal<"email" | "code">("email");
export const cargando = signal(false);
export const error = signal<string | null>(null);
export const sesionEmail = signal<string | null>(null);

export const autenticado = computed(() => sesionEmail.value !== null);

export function setToken(t: string | null) {
  token.value = t;
  if (t) localStorage.setItem("radar_token", t);
  else localStorage.removeItem("radar_token");
}

export async function pedirCodigo() {
  cargando.value = true;
  error.value = null;
  try {
    const { error: e } = await api.POST("/api/auth/request-code", {
      body: { email: email.value.trim() },
    });
    if (e) throw new Error((e as { error?: string }).error ?? "Error solicitando código");
    step.value = "code";
  } catch (err) {
    error.value = String(err);
  } finally {
    cargando.value = false;
  }
}

export async function verificarCodigo(code: string) {
  cargando.value = true;
  error.value = null;
  try {
    const { data, error: e } = await api.POST("/api/auth/verify", {
      body: { email: email.value.trim(), code },
    });
    if (e || !data) throw new Error((e as { error?: string })?.error ?? "Código incorrecto");
    sesionEmail.value = data.email;
    step.value = "email";
  } catch (err) {
    error.value = String(err);
  } finally {
    cargando.value = false;
  }
}

export async function cargarSesion() {
  const { data, error: e } = await api.GET("/api/auth/me", { headers: authHeaders() });
  if (!e && data) sesionEmail.value = data.email;
}

export async function logout() {
  try {
    await api.POST("/api/auth/logout", { headers: authHeaders() });
  } finally {
    setToken(null);
    sesionEmail.value = null;
  }
}
