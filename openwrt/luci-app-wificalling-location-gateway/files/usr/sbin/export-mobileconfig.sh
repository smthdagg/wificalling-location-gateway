#!/bin/sh
# Export the wloc-service root CA as an iOS configuration profile
# (.mobileconfig) served by uhttpd, so a test iPhone can install the CA by
# opening a Safari link - the same mechanism proxy tools use.
#
# After installation the CA must ALSO be enabled under
# Settings > General > About > Certificate Trust Settings, otherwise iOS will
# not trust MITM leaf certificates signed by it.

set -eu
umask 077

CA_PEM=/etc/wloc-service/ca.pem
OUT=/www/wloc-ca.mobileconfig

# The router's own LAN IPv4, used for the profile download URL. UCI is
# authoritative; fall back to the LAN bridge address for non-standard
# layouts.
lan_ip() {
    ip=$(uci -q get network.lan.ipaddr) || ip=
    case "$ip" in
        ''|*[!0-9.]*)
            ip=$(ip -4 addr show br-lan 2>/dev/null | sed -n 's/^[[:space:]]*inet \([0-9.]*\)\/.*/\1/p' | head -1)
            ;;
    esac
    printf '%s' "$ip"
}

ROUTER_IP=$(lan_ip)
[ -n "$ROUTER_IP" ] || {
    echo "export-mobileconfig: cannot determine the router LAN IP" >&2
    exit 1
}
CERT_URL="http://$ROUTER_IP/wloc-ca.mobileconfig"

[ -f "$CA_PEM" ] || {
    echo "export-mobileconfig: no CA at $CA_PEM (start wloc-service first)" >&2
    exit 1
}
[ -d /www ] || {
    echo "export-mobileconfig: /www (uhttpd) is not present" >&2
    exit 1
}

# Keep the intermediate certificate body private and unique. A fixed /tmp path
# would permit concurrent exports to overwrite one another or follow a symlink.
CA_B64=$(mktemp /tmp/wloc-ca.XXXXXX)
trap 'rm -f "$OUT.unsigned" "$CA_B64"' EXIT HUP INT TERM

# The PEM body is the DER certificate base64-encoded; strip the armor.
sed -n '/BEGIN CERTIFICATE/,/END CERTIFICATE/p' "$CA_PEM" \
    | grep -v 'CERTIFICATE' | tr -d '\r\n' >"$CA_B64"

uuid1=$(cat /proc/sys/kernel/random/uuid)
uuid2=$(cat /proc/sys/kernel/random/uuid)

{
    printf '%s\n' '<?xml version="1.0" encoding="UTF-8"?>'
    printf '%s\n' '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">'
    printf '%s\n' '<plist version="1.0">'
    printf '%s\n' '<dict>'
    printf '%s\n' '  <key>PayloadContent</key>'
    printf '%s\n' '  <array>'
    printf '%s\n' '    <dict>'
    printf '%s\n' '      <key>PayloadCertificateFileName</key>'
    printf '%s\n' '      <string>wloc-service-ca.pem</string>'
    printf '%s\n' '      <key>PayloadContent</key>'
    printf '%s\n' '      <data>'
    cat "$CA_B64"
    printf '%s\n' '      </data>'
    printf '%s\n' '      <key>PayloadDescription</key>'
    printf '%s\n' '      <string>wloc-service root CA</string>'
    printf '%s\n' '      <key>PayloadDisplayName</key>'
    printf '%s\n' '      <string>wloc-service root CA</string>'
    printf '%s\n' '      <key>PayloadIdentifier</key>'
    printf '%s\n' '      <string>com.wloc-service.ca</string>'
    printf '%s\n' '      <key>PayloadType</key>'
    printf '%s\n' '      <string>com.apple.security.root</string>'
    printf '%s\n' '      <key>PayloadUUID</key>'
    printf '      <string>%s</string>\n' "$uuid1"
    printf '%s\n' '      <key>PayloadVersion</key>'
    printf '%s\n' '      <integer>1</integer>'
    printf '%s\n' '    </dict>'
    printf '%s\n' '  </array>'
    printf '%s\n' '  <key>PayloadDescription</key>'
    printf '%s\n' '  <string>Install the wloc-service root CA for standalone WLOC location</string>'
    printf '%s\n' '  <key>PayloadDisplayName</key>'
    printf '%s\n' '  <string>wloc-service root CA</string>'
    printf '%s\n' '  <key>PayloadIdentifier</key>'
    printf '%s\n' '  <string>com.wloc-service.profile</string>'
    printf '%s\n' '  <key>PayloadType</key>'
    printf '%s\n' '  <string>Configuration</string>'
    printf '%s\n' '  <key>PayloadUUID</key>'
    printf '      <string>%s</string>\n' "$uuid2"
    printf '%s\n' '  <key>PayloadVersion</key>'
    printf '%s\n' '  <integer>1</integer>'
    printf '%s\n' '</dict>'
    printf '%s\n' '</plist>'
} >"$OUT.unsigned"

# CMS-sign the profile with the wloc-service CA itself. iOS trusts the CA
# from a signed profile in its system trust store, so background processes
# like locationd accept the MITM leaf certificates too (an unsigned profile
# works for Safari but locationd rejects the CA with CertificateUnknown).
if command -v openssl >/dev/null 2>&1 && [ -s "$CA_PEM" ] && [ -s "${CA_PEM%.pem}.key" ]; then
    openssl cms -sign -binary -nosmimecap -nodetach \
        -signer "$CA_PEM" -inkey "${CA_PEM%.pem}.key" \
        -in "$OUT.unsigned" -outform DER -out "$OUT" -md sha256 2>/dev/null \
        && SIGNED=1
fi
if [ "${SIGNED:-0}" != "1" ]; then
    mv "$OUT.unsigned" "$OUT"
fi
rm -f "$OUT.unsigned" "$CA_B64"
trap - EXIT HUP INT TERM
echo "export-mobileconfig: profile written to $OUT (signed=${SIGNED:-0})"
echo "export-mobileconfig: open on the test iPhone: $CERT_URL"
