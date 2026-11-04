# ADR-005 — Exposición en duckdns (radar-navarra.duckdns.org) con acceso por código enviado por email

**Estado:** aceptado · **Fecha:** 04/11/2026

## Contexto

Otros compañeros tienen que entrar a la herramienta. Presupuesto 0 €: se usa **duckdns**
(DNS dinámico gratuito) con el subdominio `radar-navarra.duckdns.org`, y el acceso se
controla con **email + código de un solo uso (OTP)** que llega por email.

## Decisión

### DNS y transporte
- `radar-navarra.duckdns.org` apunta a la IP pública del equipo; `scripts/duckdns-update.sh`
  la actualiza cada 5 min vía cron usando `RADAR_DUCKDNS_TOKEN`.
- **Caddy** (gratis) termina TLS con certificado Let's Encrypt automático y hace
  `reverse_proxy 127.0.0.1:7800` (ver `deploy/Caddyfile`). Port-forwarding 80/443 en el router.
- El binario `radar serve` **solo escucha en 127.0.0.1**, nunca en 0.0.0.0.

### Autenticación OTP por email
- Allowlist de emails autorizados (ampliable con `RADAR_AUTH_EMAILS`):
  - `hello@carlotacarrillo.eu`
  - `sergio.erramuzpe@gmail.com`
  - `pezonetis@gmail.com`
  - `duarmeler@gmail.com`
- Flujo: `POST /api/auth/request-code {email}` → si el email está autorizado, código de
  6 dígitos (hash SHA-256 en `auth_codes`, caduca en 10 min, máx 5 códigos/hora por email,
  máx 5 intentos de verificación) enviado por SMTP → `POST /api/auth/verify {email, code}`
  → sesión JWT (HS256, 7 días) en cookie `radar_session` HttpOnly (+`Secure` tras proxy)
  con `jti` registrado en la tabla `sesiones` para poder revocar (`/api/auth/logout`).
- `request-code` devuelve siempre 200 genérico: **no se revela la allowlist**.
- Envío de email vía SMTP configurable con `RADAR_SMTP_HOST/PORT/USER/PASS/FROM`
  (cualquier relay gratuito tipo Brevo/SMTP2GO/Gmail con contraseña de aplicación).
  Sin configurar, modo dev: el código se imprime en el log del servidor.
- Middleware axum protege todo `/api/*` salvo `/api/auth/*`; `GET /openapi.json` queda público.

## Consecuencias

- Acceso HTTPS gratuito y renovado solo para el equipo autorizado.
- Sin contraseñas que guardar: cada acceso es un código temporal.
- El coste de la exposición es 0 € (duckdns + Let's Encrypt + Caddy + relay SMTP free tier).
- Riesgos: si se compromete el SMTP o un email, el canal de códigos; mitigación con
  rate limit, expiración corta y revocación de sesiones.
