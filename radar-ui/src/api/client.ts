import createClient from "openapi-fetch";
import type { paths } from "./schema";
import { token } from "../stores/auth";

// Cliente tipado generado desde el contrato OAS (ADR-001).
// En producción la sesión viaja en cookie HttpOnly; el header Authorization
// es un respaldo (útil en dev y para llamadas directas).
export const api = createClient<paths>({ baseUrl: "/" });

export function authHeaders(): Record<string, string> {
  const t = token.value;
  return t ? { Authorization: `Bearer ${t}` } : {};
}
