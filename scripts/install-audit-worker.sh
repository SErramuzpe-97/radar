#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
: "${RADAR_CHROME_SOURCE:?Ruta a la carpeta chrome-linux64 de Chrome for Testing}"
node -e 'if(Number(process.versions.node.split(".")[0])!==24)process.exit(1)'
CHROME_VERSION="$("$RADAR_CHROME_SOURCE/chrome" --version)"
case "$CHROME_VERSION" in *'156.0.8078.4'*) ;; *) printf 'Versión Chrome no fijada: %s\n' "$CHROME_VERSION" >&2; exit 1 ;; esac
getent group radar-audit >/dev/null || groupadd --system radar-audit
id radar-audit >/dev/null 2>&1 || useradd --system --gid radar-audit --home-dir /var/lib/radar-audits --shell /usr/sbin/nologin radar-audit
install -d -m 2770 -o radar-audit -g radar-audit /var/lib/radar-audits
install -d -m 755 /opt/radar-audit/chrome
TMP_WORKER="$(mktemp -d)"
trap 'rm -rf "$TMP_WORKER"' EXIT
curl -fL --retry 2 -o "$TMP_WORKER/siteone.tar.gz" https://github.com/janreges/siteone-crawler/releases/download/v2.5.1/siteone-crawler-v2.5.1-linux-musl-x64.tar.gz
printf '%s  %s\n' bd96b9502563aea2581fc248625871e69b60d7f1bb68a484f226c551c73706fd "$TMP_WORKER/siteone.tar.gz" | sha256sum -c -
tar -xzf "$TMP_WORKER/siteone.tar.gz" -C "$TMP_WORKER"
install -m 755 "$TMP_WORKER/siteone-crawler/siteone-crawler" /opt/radar-audit/siteone-crawler
install -m 644 "$TMP_WORKER/siteone-crawler/LICENSE" /opt/radar-audit/SITEONE-LICENSE
install -d /opt/radar-audit/chrome/chrome-linux64
cp -a "$RADAR_CHROME_SOURCE/." /opt/radar-audit/chrome/chrome-linux64/
install -m 644 "$ROOT"/audit-worker/*.mjs "$ROOT"/audit-worker/package*.json /opt/radar-audit/
install -m 755 "$ROOT/audit-worker/chrome-wrapper" /opt/radar-audit/chrome-wrapper
(cd /opt/radar-audit && npm ci --omit=dev)
install -m 644 "$ROOT/deploy/radar-audit.service" /etc/systemd/system/radar-audit.service
install -m 644 "$ROOT/deploy/radar-backup.service" /etc/systemd/system/radar-backup.service
install -m 644 "$ROOT/deploy/radar-backup.timer" /etc/systemd/system/radar-backup.timer
install -d /etc/systemd/system/radar.service.d
install -m 644 "$ROOT/deploy/radar-audit-spool.conf" /etc/systemd/system/radar.service.d/audit.conf
systemctl daemon-reload
systemctl enable --now radar-backup.timer
systemctl enable --now radar-audit
# Aplicar el grupo/directorio al servidor en el siguiente reinicio de radar.
