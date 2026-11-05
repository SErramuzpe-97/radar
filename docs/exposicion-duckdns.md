# Despliegue de Radar con DuckDNS y Caddy

Guía para publicar Radar en `https://radar-navarra.duckdns.org`. Radar escucha solo en
`127.0.0.1:7800`; Caddy recibe tráfico HTTPS y lo reenvía al proceso local. Hace falta un
servidor Linux con acceso de administrador, una cuenta DuckDNS y un router que permita
redirigir los puertos TCP 80 y 443.

## 1. Crear el dominio DuckDNS

1. Inicia sesión en [DuckDNS](https://www.duckdns.org/) y crea el subdominio `radar-navarra`.
2. Copia el token de la cuenta. La API de DuckDNS acepta el subdominio sin `.duckdns.org`,
   detecta la IPv4 si `ip` queda vacío y devuelve `OK` cuando la actualización funciona
   ([especificación](https://www.duckdns.org/spec.jsp)).
3. En el servidor, ejecuta una actualización manual sin guardar el token en el historial:

   ```bash
   cd /ruta/al/radar
   read -rsp "Token de DuckDNS: " RADAR_DUCKDNS_TOKEN
   export RADAR_DUCKDNS_TOKEN
   ./scripts/duckdns-update.sh
   unset RADAR_DUCKDNS_TOKEN
   ```

   La respuesta esperada acaba en `duckdns radar-navarra: OK`. El dominio predeterminado es
   `radar-navarra`; se puede cambiar con `RADAR_DUCKDNS_DOMAIN`.

## 2. Mantener la IP actualizada

Guarda el token en un archivo legible solo por `root`:

```bash
sudo install -o root -g root -m 600 /dev/null /etc/radar-duckdns.env
sudoedit /etc/radar-duckdns.env
```

Contenido del archivo:

```dotenv
RADAR_DUCKDNS_TOKEN=pega_aqui_el_token
RADAR_DUCKDNS_DOMAIN=radar-navarra
```

Entra en la tabla cron de `root` y añade la línea siguiente. Conserva las otras tareas que ya
haya en la tabla:

```bash
sudo crontab -e
```

```cron
*/5 * * * * . /etc/radar-duckdns.env; /ruta/al/radar/scripts/duckdns-update.sh >> /var/log/radar-duckdns.log 2>&1
```

Sustituye `/ruta/al/radar` por la ruta real del repositorio. Comprueba la ejecución con
`sudo tail -n 20 /var/log/radar-duckdns.log`. Para comparar la dirección pública y el DNS:

```bash
curl -4fsS https://api.ipify.org
dig +short radar-navarra.duckdns.org A
```

## 3. Redirigir puertos en el router

Asigna una IP local fija o una reserva DHCP al servidor. Configura estas reglas TCP hacia esa
IP:

| Puerto externo | Destino | Uso |
|---|---|---|
| 80 | servidor:80 | Validación y redirección HTTP de Caddy/Let's Encrypt |
| 443 | servidor:443 | HTTPS |

Si el proveedor usa CGNAT, el reenvío de puertos del router no será suficiente; solicita una
IPv4 pública al proveedor antes de continuar.

## 4. Instalar Caddy

Instala Caddy siguiendo las instrucciones oficiales para la distribución del servidor. Copia
el archivo de ejemplo del repositorio y valida su sintaxis:

```bash
sudo cp /ruta/al/radar/deploy/Caddyfile /etc/caddy/Caddyfile
sudo caddy validate --config /etc/caddy/Caddyfile
sudo systemctl enable --now caddy
```

El [Caddyfile de ejemplo](../deploy/Caddyfile) sirve `radar-navarra.duckdns.org`, habilita
compresión gzip y hace `reverse_proxy` a `127.0.0.1:7800`. Caddy obtiene y renueva el
certificado TLS automáticamente cuando el DNS y los puertos son accesibles.

## 5. Instalar Radar como servicio

Compila la UI y después el binario en un entorno con Rust, Node.js y npm. El frontend se
construye explícitamente antes de incrustarlo en el ejecutable:

```bash
./scripts/build-release.sh
```

Crea un usuario de sistema y copia el binario:

```bash
sudo useradd --system --home-dir /var/lib/radar --shell /usr/sbin/nologin radar
sudo install -d -o radar -g radar /var/lib/radar
sudo install -o root -g root -m 755 target/release/radar-cli /usr/local/bin/radar
```

Guarda la configuración de autenticación y SMTP en un archivo protegido:

```bash
sudo install -o root -g root -m 600 /dev/null /etc/radar.env
sudoedit /etc/radar.env
```

Ejemplo de contenido, reemplazando las direcciones y credenciales por las propias:

```dotenv
RADAR_AUTH_EMAILS=hello@carlotacarrillo.eu,sergio.erramuzpe@gmail.com,pezonetis@gmail.com,duarmeler@gmail.com
RADAR_SMTP_HOST=smtp-relay.brevo.com
RADAR_SMTP_PORT=587
RADAR_SMTP_USER=usuario_smtp
RADAR_SMTP_PASS=clave_smtp
RADAR_SMTP_FROM=radar@tu-dominio.eu
RADAR_SMTP_FROM_NAME=Radar Navarra
```

Crea `/etc/systemd/system/radar.service`:

```ini
[Unit]
Description=Radar Navarra
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=radar
Group=radar
WorkingDirectory=/var/lib/radar
EnvironmentFile=/etc/radar.env
ExecStart=/usr/local/bin/radar --db /var/lib/radar/radar.db serve --puerto 7800
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

Activa Radar y comprueba que escucha en loopback:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now radar
sudo systemctl status radar
ss -tlnp | grep ':7800'
```

La dirección local debe ser `127.0.0.1:7800`. No configures Radar para escuchar en
`0.0.0.0`.

## 6. Acceso, allowlist y rotación de secretos

Abre `https://radar-navarra.duckdns.org`, introduce un email autorizado y el código de seis
dígitos recibido. La allowlist predeterminada contiene los cuatro emails del ejemplo anterior.
Para añadir o quitar personas, edita `RADAR_AUTH_EMAILS` en `/etc/radar.env`, separando las
direcciones con comas, y reinicia el servicio:

```bash
sudoedit /etc/radar.env
sudo systemctl restart radar
```

Para rotar el token DuckDNS, usa la opción de recrear token en el menú de cuenta de DuckDNS
([instrucciones oficiales](https://www.duckdns.org/faqs.jsp)). Actualiza
`RADAR_DUCKDNS_TOKEN` en `/etc/radar-duckdns.env`; el cron lo leerá en la siguiente ejecución.

Para rotar la contraseña SMTP, cámbiala primero en el proveedor, actualiza `RADAR_SMTP_PASS`
en `/etc/radar.env` y reinicia Radar. Para invalidar todas las sesiones JWT y regenerar la
clave local, detén el servicio y borra la clave y las sesiones de la base de datos:

```bash
sudo systemctl stop radar
sudo sqlite3 /var/lib/radar/radar.db "DELETE FROM sesiones; DELETE FROM meta WHERE key='jwt_secret';"
sudo systemctl start radar
```

## Checklist

- [ ] DuckDNS resuelve a la IPv4 pública actual y el cron registra `OK` cada cinco minutos.
- [ ] El router reenvía TCP 80 y 443 al servidor.
- [ ] Caddy está activo y obtiene el certificado para `radar-navarra.duckdns.org`.
- [ ] Radar escucha únicamente en `127.0.0.1:7800`.
- [ ] Los archivos `/etc/radar.env` y `/etc/radar-duckdns.env` son `root:root` con permisos `600`.
- [ ] La allowlist contiene solo las personas que deben acceder.
- [ ] El login envía códigos por SMTP y la cookie de sesión es `HttpOnly`, `Secure` y `SameSite=Lax`.
- [ ] El token DuckDNS y la contraseña SMTP no están guardados en el repositorio.
