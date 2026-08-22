use std::fs;

#[test]
fn cargo_manifest_pins_openwrt_compatible_rust_and_minimal_features() {
    let manifest = fs::read_to_string("Cargo.toml").expect("Cargo.toml exists");
    assert!(manifest.contains("rust-version = \"1.90\""));
    assert!(!manifest.contains("license = \"MIT OR Apache-2.0\""));
    assert!(manifest.contains("rustls = { version = \"=0.23.43\", default-features = false"));
    assert!(manifest.contains("features = [\"ring\", \"std\", \"tls12\"]"));
    assert!(manifest.contains("tokio = { version = \"=1.48.0\", default-features = false"));
    assert!(manifest.contains("tokio-rustls = { version = \"=0.26.4\", default-features = false"));
    assert!(!manifest.contains("\"full\""));
    assert!(!manifest.contains("prost-build"));
}

#[test]
fn rust_sources_do_not_embed_private_material_or_unsafe_blocks() {
    for path in ["src/lib.rs", "src/main.rs", "src/tls_h2.rs"] {
        let content = fs::read_to_string(path).expect("source exists");
        assert!(!content.contains("BEGIN PRIVATE KEY"));
        assert!(!content.contains("api_key"));
        assert!(!content.contains("unsafe"));
    }
}

#[test]
fn pull_requests_run_the_real_pinned_openwrt_cross_build() {
    let workflow = fs::read_to_string(".github/workflows/ci.yml").expect("CI workflow exists");

    assert!(workflow.contains("openwrt-cross-build:"));
    assert!(workflow.contains("docker pull --platform linux/amd64"));
    assert!(workflow
        .contains("rust@sha256:64232e656c058f4468e8d024e990acff04f0fd5a5c0a88a574dc37773d7325c9"));
    assert!(workflow.contains("./scripts/ci/verify-rust-openwrt.sh"));
}

#[test]
fn h2_errors_are_not_reclassified_as_success() {
    let source = fs::read_to_string("src/lib.rs").expect("source exists");
    assert!(!source.contains("contains(\"BrokenPipe\")"));
    assert!(!source.contains("contains(\"broken pipe\")"));
}
