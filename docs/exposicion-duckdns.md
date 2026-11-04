# Exponer Radar en duckdns (radar-navarra.duckdns.org) con login por código

Guía de despliegue 0 € para que el equipo entre desde fuera (ADR-005).

## 1. Cuenta duckdns y dominio

1. Entra en https://www.duckdns.org y crea el subdominio **radar-navarra** (aparecerá como
   `radar-navarra.duckdns.org`).
2. Copia el **token** de tu cuenta (arriba a la derecha).
3. En el equipo servidor, cada 5 min se actualiza la IP pública:

   ```bash
   echo '*/5 * * * * RADAR_DUCKDNS_TOKEN=TU_TOKEN /ruta/radar/scripts/duckdns-update.sh >> /var/log/radar-duckdns.log 2>&1' | crontab -
   ```

## 2. Router

Abre (port-forward) los puertos **80 y 443** TCP hacia la IP local del equipo servidor.
Son necesarios para la validación HTTP-01 de Let's Encrypt y para HTTPS.

## 3. Caddy (TLS automático + proxy)

```bash
sudo apt install caddy   # o descarga de caddyserver.com
sudo cp deploy/Caddyfile /etc/caddy/Caddyfile
sudo systemctl enable --now caddy
```

Caddy obtiene y renueva solo el certificado Let's Encrypt de `radar-navarra.duckdns.org`
y redirige todo a `127.0.0.1:7800`.

## 4. Radar (solo localhost)

```bash
cargo build --release
./target/release/radar-cli --db radar.db serve --puerto 7800   # escucha solo en 127.0.0.1
```

## 5. Emails autorizados y SMTP

Allowlist por defecto (ADR-005): `hello@carlotacarrillo.eu`, `sergio.erramuzpe@gmail.com`,
`pezonetis@gmail.com`, `duarmeler@gmail.com`. Para cambiarla:

```bash
export RADAR_AUTH_EMAILS="hello@carlotacarrillo.eu,sergio.erramuzpe@gmail.com,pezonetis@gmail.com,duarmeler@gmail.com,otro@email.com"
```

Para enviar los códigos por email (relay SMTP, tier gratuito tipo Brevo / SMTP2GO / Gmail
con contraseña de aplicación):

```bash
export RADAR_SMTP_HOST=smtp-relay.brevo.com
export RADAR_SMTP_PORT=587
export RADAR_SMTP_USER=tu_usuario_smtp
export RADAR_SMTP_PASS=tu_clave_smtp
export RADAR_SMTP_FROM=radar@tu-dominio.eu
```

Sin `RADAR_SMTP_HOST`, el servidor imprime el código en su log (solo para pruebas local).

## 6. Acceso

1. Abrir https://radar-navarra.duckdns.org
2. Poner el email autorizado → llega un código de 6 dígitos (caduca en 10 min)
3. Introducirlo → sesión de 7 días (cookie HttpOnly Secure)

## Checklist de seguridad

- [ ] El binario solo escucha en 127.0.0.1 (comprobar con `ss -tlnp`)
- [ ] duckdns token en variable de entorno, no en el repo
- [ ] `RADAR_SMTP_PASS` en entorno/systemd, no commiteado
- [ ] Solo los 4 emails (o los que estén en RADAR_AUTH_EMAILS) pueden pedir código
- [ ] Los códigos caducan a los 10 min y a los 5 intentos fallidos
- [ ] Sesiones revocables (tabla `sesiones`) con `/api/auth/logout`
