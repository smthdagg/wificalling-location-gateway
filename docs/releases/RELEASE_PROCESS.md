# Release process and feed signing policy

How a new release is cut, and how the opkg feed signing key is managed.

## Signing key policy (stable since 1.0.6)

The feed signing key is **long-lived**: the same key is reused for every
release, so users import the public key exactly once. Per-release key
rotation is **not** performed anymore (it forced users to re-import the
key on every upgrade without a meaningful security gain - the private key
was already never committed or shared).

- Key ID: `f7050198aa77cf15`
- Private key: `~/.zcode/keys/wloc-signing.key` on the release machine
  (mode `0600`, never committed to git, never copied elsewhere)
- Public key: `~/.zcode/keys/wloc-signing.pub`; published as `wloc.pub`
  in the feed repository (`smthdagg/wificalling-location-gateway-feed`,
  `gh-pages` branch serves the package source, `main` branch serves the
  key file for `wget`)
- The public key is also referenced in the project README installation
  instructions. It must not change between releases.

If the private key is ever lost or compromised, rotate it **once**:
generate a new keypair (see below), publish the new `wloc.pub`, update the
README key ID, and document the rotation in the release notes.

## Signing a feed index

The index files (`Packages`, `Packages.gz`) must be signed with the
long-lived key. `usign` is not available on macOS; run it through the
pinned OpenWrt rootfs image:

```sh
# $FEED_DIR is a checkout of the feed repository on the gh-pages branch
# containing the release .ipk/.apk files and the generated index.
docker run --rm \
  -v "$HOME/.zcode/keys:/keys:ro" \
  -v "$FEED_DIR:/feed" \
  --entrypoint /bin/sh \
  ghcr.io/openwrt/rootfs:x86_64-24.10.8@sha256:9972a4b4747cd136abd597475d7b88c51a49fd849d0d53f069a2f4bf446061b9 \
  -c 'cd /feed && \
      usign -S -s /keys/wloc-signing.key -m Packages -x Packages.sig && \
      usign -S -s /keys/wloc-signing.key -m Packages.gz -x Packages.gz.sig && \
      usign -V -q -p /keys/wloc-signing.pub -m Packages -P Packages.sig && \
      usign -V -q -p /keys/wloc-signing.pub -m Packages.gz -P Packages.gz.sig'
```

This is also implemented as `scripts/openwrt/sign-feed.sh`.

## Release checklist

1. **Audit and test** before cutting the release:
   - `./scripts/ci/verify.sh` (full gate: Rust tests, Python tests, JS
     regression tests, packaging tests)
   - `cargo clippy --all-targets` and `cargo fmt --check`
   - privacy sweep: no real device IPs, credentials, or keys in the repo
2. **Use the V2 major version consistently**: the release branch uses
   `2.0.0` across `VERSION`, `Cargo.toml`/`Cargo.lock`, both OpenWrt Makefiles,
   package builders, tests, README install examples, and the V2 changelog.
3. **Build runtimes** only if `src/` changed (aarch64 via
   `verify-rust-openwrt.sh` with `OPENWRT_BIN_NAME`, x86_64 via
   `build-x86_64-runtime.sh`); otherwise reuse the existing
   `dist/runtime/` binaries.
4. **Build packages**: `build-luci-ipk.sh <ver>-1 ax6s-standalone`
   (aarch64) and `build-release-packages.sh` (x86_64 ipk + apk), then
   write `SHA256SUMS` in `dist/openwrt-release/`.
   Set `WLOC_UPDATE_SIGNING_KEY` to the protected release key and
   `WLOC_UPDATE_USIGN` to the approved `usign` binary when invoking the
   release builder; it refuses to emit an unsigned release. Rebuilding an
   unsigned manifest removes any old detached signature first.
5. **Install test**: `verify-docker-matrix.sh --dist-dir
   dist/openwrt-release` (four environments). On the space-constrained AX6S,
   back up UCI/CA, stop and remove the old Wificalling/WLOC application
   packages first, retain the selected tiny/lite/PassWall sing-box provider,
   then perform the live install/upgrade, resource measurement and rollback.
6. **Feed**: swap the release files in the feed repo `gh-pages` branch,
   regenerate `Packages`/`Packages.gz`
   (`scripts/gen-feed-index.sh`), sign with
   `scripts/openwrt/sign-feed.sh` (same key as always), push `gh-pages`
   and `main`. The `wloc.pub` does **not** change.
7. **GitHub**: only after Issue #41 acceptance is green and the final version
   is `2.0.0`, tag `v<version>`, create the Release with the three
   packages, `SHA256SUMS`, and the signed `Packages`/`Packages.gz`(+`.sig`)
   assets, bilingual notes (English first, Chinese after).
8. **Verify** the feed signature on the AX6S (`opkg update` must print
   `Signature check passed` with the existing key) and confirm the upgrade.
