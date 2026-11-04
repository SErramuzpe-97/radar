#!/usr/bin/env bash
# Actualiza la IP pública de radar-navarra.duckdns.org (DNS dinámico, 0 €).
# Requiere RADAR_DUCKDNS_TOKEN (token de la cuenta duckdns.org).
# Uso en cron (cada 5 min):
#   */5 * * * * RADAR_DUCKDNS_TOKEN=xxxx /ruta/radar/scripts/duckdns-update.sh >> /var/log/radar-duckdns.log 2>&1
set -euo pipefail
: "${RADAR_DUCKDNS_TOKEN:?falta RADAR_DUCKDNS_TOKEN (token de duckdns.org)}"
DOMAIN="${RADAR_DUCKDNS_DOMAIN:-radar-navarra}"
RESP=$(curl -s -m 15 "https://www.duckdns.org/update?domains=${DOMAIN}&token=${RADAR_DUCKDNS_TOKEN}&ip=")
echo "$(date -Is) duckdns ${DOMAIN}: ${RESP}"
[[ "$RESP" == *"OK"* ]] || exit 1
