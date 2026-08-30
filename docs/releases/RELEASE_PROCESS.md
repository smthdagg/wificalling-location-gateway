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
  in the feed repository (`smthdagg/Smthdagg-Repo-feeds`,
  `gh-pages` branch serves per-project package directories (the WLOC
  packages live in `wificalling-location-gateway/`), `main` branch serves
  the key file for `wget` and the index generator script)
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
2. **Bump the version** (e.g. `1.0.6 -> 1.0.7`): `VERSION`, `Cargo.toml`
   (+ `Cargo.lock` via `cargo update -p wificalling-location-gateway
   --offline`), `scripts/openwrt/build-release-packages.sh`,
   `scripts/build-luci-ipk.sh`, both `openwrt/*/Makefile` files, the
   version tests in `tests/scripts/` (mind the escaped regex line in
   `test-release-version.sh`), README install examples, and a new
   `CHANGELOG.md` entry.
3. **Build runtimes** only if `src/` changed (aarch64 via
   `verify-rust-openwrt.sh` with `OPENWRT_BIN_NAME`, x86_64 via
   `build-x86_64-runtime.sh`); otherwise reuse the existing
   `dist/runtime/` binaries.
4. **Build packages**: `build-luci-ipk.sh <ver>-1 ax6s-standalone`
   (aarch64) and `build-release-packages.sh` (x86_64 ipk + apk), then
   write `SHA256SUMS` in `dist/openwrt-release/`.
5. **Install test**: `verify-docker-matrix.sh --dist-dir
   dist/openwrt-release` (four environments) and a live upgrade on the
   AX6S test router.
5a. **Post-upgrade hygiene (AX6S, mandatory)**: remove every uploaded
   installer package from `/tmp` in the same session that installed it
   (`opkg install /tmp/x.ipk && rm -f /tmp/x.ipk`), delete any ad-hoc debug
   backup directories created during the session, then
   `sync; echo 3 > /proc/sys/vm/drop_caches` and re-read `MemAvailable`: it
   must stay above the computed cold-start requirement of
   `require_start_memory` (inflated Lite runtime + 8 MiB). Verify service
   health (`wloc-health.sh`), the package version, and a single sing-box
   process. Prefer the signed-feed upgrade
   (`opkg update && opkg upgrade <package>`), which creates no `/tmp`
   artifact at all.
6. **Feed (private signed package source — mandatory, before tagging)**:
   the index generator lives in the feed repository, not here. Standard
   sequence (use the repository's noreply git identity — the feed repo
   rejects pushes from personal-email commits):
   a. `git clone --branch main https://github.com/smthdagg/Smthdagg-Repo-feeds.git /tmp/wloc-feed-main`
      and `git clone --branch gh-pages https://github.com/smthdagg/Smthdagg-Repo-feeds.git /tmp/wloc-feed`.
   b. In the `gh-pages` checkout: work inside the project subdirectory
      (`wificalling-location-gateway/` — directory name must equal the
      project repository name): remove superseded packages, copy the new
      IPKs.
   c. `/tmp/wloc-feed-main/scripts/gen-feed-index.sh
      /tmp/wloc-feed/wificalling-location-gateway` (regenerates that
      project's `Packages`/`Packages.gz`), then
      `scripts/openwrt/sign-feed.sh /tmp/wloc-feed/wificalling-location-gateway`
      (same long-lived key as always) — signing without regenerating the
      index is forbidden.
   d. Append a row to `UPDATES.md` (master update log), run
      `/tmp/wloc-feed-main/scripts/feed-verify.sh /tmp/wloc-feed` (it must
      pass: index integrity, signatures, checksums, log coverage), then
      commit and push `gh-pages`. The feed `main` branch changes only if
      `wloc.pub` or its docs change — also align the feed `README.md`
      package table with the current release filenames.
   e. `wloc.pub` does **not** change between releases.
7. **GitHub**: tag `v<version>`, create the Release with the three
   packages, `SHA256SUMS`, and the signed `Packages`/`Packages.gz`(+`.sig`)
   assets, bilingual notes (English first, Chinese after).
8. **Verify** the feed signature on the AX6S (`opkg update` must print
   `Signature check passed` with the existing key) and confirm the upgrade.
