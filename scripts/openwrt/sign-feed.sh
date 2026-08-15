#!/bin/sh
# Sign the opkg feed index (Packages / Packages.gz) with the long-lived
# feed signing key.
#
# Key policy: one key is reused for every release - users import the
# public key once. The private key lives on the release machine only
# (default ~/.zcode/keys/wloc-signing.key, mode 0600) and is never
# committed to git. See docs/releases/RELEASE_PROCESS.md.
#
# Usage: sign-feed.sh <feed-directory>
# The directory must contain Packages and Packages.gz (the gh-pages
# checkout of the feed repository). Writes Packages.sig and
# Packages.gz.sig, then verifies both signatures.

set -eu

OPENWRT_ROOTFS='ghcr.io/openwrt/rootfs:x86_64-24.10.8@sha256:9972a4b4747cd136abd597475d7b88c51a49fd849d0d53f069a2f4bf446061b9'
KEY_DIR=${WLOC_SIGN_KEY_DIR:-"$HOME/.zcode/keys"}
SIGNING_KEY="$KEY_DIR/wloc-signing.key"
PUBLIC="$KEY_DIR/wloc-signing.pub"

feed_dir=${1:?usage: sign-feed.sh <feed-directory>}
[ -d "$feed_dir" ] || { echo "sign-feed: not a directory: $feed_dir" >&2; exit 2; }
[ -f "$feed_dir/Packages" ] || { echo "sign-feed: missing $feed_dir/Packages" >&2; exit 2; }
[ -f "$feed_dir/Packages.gz" ] || { echo "sign-feed: missing $feed_dir/Packages.gz" >&2; exit 2; }
[ -f "$SIGNING_KEY" ] || { echo "sign-feed: missing signing key $SIGNING_KEY" >&2; exit 2; }
[ -f "$PUBLIC" ] || { echo "sign-feed: missing public key $PUBLIC" >&2; exit 2; }

case "$feed_dir" in
	/*) ;;
	*) echo "sign-feed: feed directory must be absolute" >&2; exit 2 ;;
esac

docker run --rm \
	-v "$KEY_DIR:/keys:ro" \
	-v "$feed_dir:/feed" \
	--entrypoint /bin/sh \
	"$OPENWRT_ROOTFS" \
	-c 'cd /feed && \
	    usign -S -s /keys/wloc-signing.key -m Packages -x Packages.sig && \
	    usign -S -s /keys/wloc-signing.key -m Packages.gz -x Packages.gz.sig && \
	    usign -V -q -p /keys/wloc-signing.pub -m Packages -P Packages.sig && \
	    usign -V -q -p /keys/wloc-signing.pub -m Packages.gz -P Packages.gz.sig'

echo "sign-feed: signed and verified $feed_dir/Packages{,.gz}.sig"
echo "sign-feed: key $(sed -n 's/.*key //p' "$PUBLIC")"
