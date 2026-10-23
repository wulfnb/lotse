#!/bin/sh
set -eu
tls=/config/tls
fail() { echo "$*" >&2; exit 1; }

# Settings of the generated certificate. A change regenerates it.
tls_settings() {
    sans='DNS:localhost,IP:127.0.0.1'
    set -f
    old_ifs=$IFS; IFS=,
    for name in ${BROWSER_TLS_SANS:-}; do
        name=$(printf '%s' "$name" | tr -d ' \t')
        case $name in
            '') continue ;;
            DNS:*|IP:*) : ;;
            *:*) name=IP:$name ;;
            *[!0-9.]*) name=DNS:$name ;;
            *) name=IP:$name ;;
        esac
        case ${name#*:} in
            ''|*[!A-Za-z0-9.:*-]*) IFS=$old_ifs; fail "BROWSER_TLS_SANS: invalid entry '$name'" ;;
        esac
        sans=$sans,$name
    done
    IFS=$old_ifs
    set +f
    echo "v2 ec-p256 365d $sans"
}

setup_tls() {
    mkdir -p "$tls"
    chown browser:browser "$tls"
    chmod 700 "$tls"
    # User-supplied certificate files (for example a mounted secret or a
    # certificate renewed on the host) are copied on every start and hourly.
    if [ -n "${WEGA_TLS_CERT_FILE:-}${WEGA_TLS_KEY_FILE:-}" ]; then
        [ -r "${WEGA_TLS_CERT_FILE:-}" ] && [ -r "${WEGA_TLS_KEY_FILE:-}" ] ||
            fail 'WEGA_TLS_CERT_FILE and WEGA_TLS_KEY_FILE must both name readable files'
        cmp -s "$WEGA_TLS_CERT_FILE" "$tls/server.crt" && cmp -s "$WEGA_TLS_KEY_FILE" "$tls/server.key" && return 0
        install -m 600 -o browser -g browser "$WEGA_TLS_KEY_FILE" "$tls/server.key"
        install -m 644 -o browser -g browser "$WEGA_TLS_CERT_FILE" "$tls/server.crt"
        rm -f "$tls/generated.conf"
        return 0
    fi
    wanted=$(tls_settings)
    if [ -s "$tls/server.key" ] && [ -s "$tls/server.crt" ]; then
        if [ -f "$tls/generated.conf" ]; then
            current=$(cat "$tls/generated.conf")
        elif [ "$(openssl x509 -noout -subject -issuer -in "$tls/server.crt" 2>/dev/null)" = "$(printf 'subject=CN=localhost\nissuer=CN=localhost')" ]; then
            current=legacy  # Self-signed RSA certificate from earlier images.
        else
            return 0  # A certificate placed in the volume by the user is never replaced.
        fi
        # Keep it unless its settings changed or it expires within 30 days.
        [ "$current" = "$wanted" ] && openssl x509 -checkend 2592000 -noout -in "$tls/server.crt" >/dev/null && return 0
    fi
    echo 'Generating a self-signed TLS certificate' >&2
    ( umask 077
        openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes -days 365 \
            -subj /CN=Wega -addext "subjectAltName=${wanted##* }" \
            -keyout "$tls/.server.key.new" -out "$tls/.server.crt.new" 2>/dev/null
    )
    chown browser:browser "$tls/.server.key.new" "$tls/.server.crt.new"
    mv -f "$tls/.server.key.new" "$tls/server.key"
    mv -f "$tls/.server.crt.new" "$tls/server.crt"
    printf '%s\n' "$wanted" > "$tls/generated.conf"
}

if [ "${1:-}" = healthcheck ]; then
    curl -fkSs --max-time 3 https://127.0.0.1:7443/healthz >/dev/null
    [ "$(curl -kSs --max-time 3 -o /dev/null -w '%{http_code}' https://127.0.0.1:7443/auth/account)" = 401 ]
    supervisorctl -c /etc/wega-supervisord.conf status | awk '
        $2 != "RUNNING" {bad=1} END {exit bad || NR < 6}'
    exit $?
fi
if [ "${1:-}" = renew-tls ]; then
    setup_tls
    exit 0
fi
case "${BROWSER_RESOLUTION}" in
    *[!0-9x]*|*x*x*|x*|*x|'') echo 'BROWSER_RESOLUTION must be WIDTHxHEIGHT' >&2; exit 1 ;;
esac
width=${BROWSER_RESOLUTION%x*}; height=${BROWSER_RESOLUTION#*x}
[ "$width" != "$BROWSER_RESOLUTION" ] && [ "$width" -ge 800 ] && [ "$width" -le 3840 ] && [ "$height" -ge 600 ] && [ "$height" -le 2160 ] || {
    echo 'Resolution must be between 800x600 and 3840x2160' >&2; exit 1;
}
mkdir -p /config/browser /config/Downloads /config/.cache /config/uploads /run/wega/browser /run/wega/auth /run/wega/vnc
chown browser:browser /config /config/browser /config/Downloads /config/.cache /run/wega/browser
chmod 700 /run/wega/browser
# Xvfb (browser UID) cannot create its socket directory on a fresh /tmp tmpfs.
mkdir -p /tmp/.X11-unix && chmod 1777 /tmp/.X11-unix
# Unix sockets replace loopback TCP: rfb.sock (x11vnc, browser UID) is reachable
# by websockify; auth.sock and websocket.sock (wega-auth UID) by nginx.
chown root:root /run/wega && chmod 755 /run/wega
chown browser:wega-ipc /run/wega/vnc && chmod 2750 /run/wega/vnc
chown wega-auth:wega-ipc /run/wega/auth && chmod 2750 /run/wega/auth
setup_tls

# Optional reverse proxies whose X-Forwarded-For is trusted for rate limiting.
realip=/run/wega/browser/nginx-realip.conf
rm -f "$realip"
if [ -n "${WEGA_TRUSTED_PROXIES:-}" ]; then
    {
        for proxy in $(printf '%s' "$WEGA_TRUSTED_PROXIES" | tr ',' ' '); do
            case $proxy in
                unix:) : ;;
                ''|*[!0-9A-Fa-f.:/]*) fail "WEGA_TRUSTED_PROXIES: invalid address '$proxy'" ;;
            esac
            echo "set_real_ip_from $proxy;"
        done
        echo 'real_ip_header X-Forwarded-For;'
        echo 'real_ip_recursive on;'; } > "$realip.new"
    chmod 644 "$realip.new"
    mv -f "$realip.new" "$realip"
fi

# Uploads: written by the gateway, readable by the browser group, kept outside
# the private credential directory. Earlier images stored them in /config/auth.
if [ -d /config/auth/uploads ]; then
    for entry in /config/auth/uploads/*; do
        if [ -e "$entry" ] && [ ! -e "/config/uploads/${entry##*/}" ]; then mv "$entry" /config/uploads/; fi
    done
    rm -rf /config/auth/uploads
fi
chown -R wega-auth:browser /config/uploads
find /config/uploads -type d -exec chmod 2750 {} +
find /config/uploads -type f -exec chmod 640 {} +

python3 /opt/wega/auth-service.py initialize
chown -R wega-auth:wega-auth /config/auth
chmod 700 /config/auth
# Plain bootstrap credentials are never inherited by browser/HTTP worker processes.
unset BROWSER_USER BROWSER_PASSWORD BROWSER_PASSWORD_FILE
if [ "$#" -gt 0 ]; then exec gosu browser "$@"; fi
exec supervisord -n -c /etc/wega-supervisord.conf
