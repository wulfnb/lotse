#!/bin/sh
set -eu
if [ "${1:-}" = healthcheck ]; then
    curl -fkSs --max-time 3 https://127.0.0.1:7443/healthz >/dev/null
    [ "$(curl -kSs --max-time 3 -o /dev/null -w '%{http_code}' https://127.0.0.1:7443/auth/account)" = 401 ]
    supervisorctl -c /etc/servo-supervisord.conf status | awk '
        $2 != "RUNNING" {bad=1} END {exit bad || NR != 6}'
    exit $?
fi
case "${BROWSER_RESOLUTION}" in
    *[!0-9x]*|*x*x*|x*|*x|'') echo 'BROWSER_RESOLUTION must be WIDTHxHEIGHT' >&2; exit 1 ;;
esac
width=${BROWSER_RESOLUTION%x*}; height=${BROWSER_RESOLUTION#*x}
[ "$width" != "$BROWSER_RESOLUTION" ] && [ "$width" -ge 800 ] && [ "$width" -le 3840 ] && [ "$height" -ge 600 ] && [ "$height" -le 2160 ] || {
    echo 'Resolution must be between 800x600 and 3840x2160' >&2; exit 1;
}
mkdir -p /config/browser /config/Downloads /config/tls /config/.cache /run/servo
chown browser:browser /config /config/browser /config/Downloads /config/tls /config/.cache /run/servo
chmod 700 /run/servo /config/tls
if [ ! -s /config/tls/server.key ] || [ ! -s /config/tls/server.crt ]; then
    umask 077
    openssl req -x509 -newkey rsa:2048 -nodes -days 365 \
        -subj /CN=localhost -addext 'subjectAltName=DNS:localhost,IP:127.0.0.1' \
        -keyout /config/tls/server.key -out /config/tls/server.crt 2>/dev/null
    chown browser:browser /config/tls/server.key /config/tls/server.crt
fi
python3 /opt/servo/auth-service.py initialize
chown -R browser:browser /config/auth
# Plain bootstrap credentials are never inherited by browser/HTTP worker processes.
unset BROWSER_USER BROWSER_PASSWORD BROWSER_PASSWORD_FILE
if [ "$#" -gt 0 ]; then exec gosu browser "$@"; fi
exec supervisord -n -c /etc/servo-supervisord.conf
