use wificalling_location_gateway::{
    build_tls_stack, inspect_candidate, roundtrip_synthetic_probe, run_h2_prior_knowledge_smoke,
    CandidateMeta, GateDecision, APPROVED_WLOC_HOSTS, MAX_WLOC_BODY_BYTES,
};

#[test]
fn exact_wloc_hosts_are_the_only_candidates() {
    let valid = CandidateMeta {
        source_ip: "192.0.2.10".to_string(),
        hostname: "gs-loc.apple.com".to_string(),
        content_length: Some(128),
    };

    assert_eq!(GateDecision::Candidate, inspect_candidate(&valid));

    let wrong_host = CandidateMeta {
        hostname: "evil.gs-loc.apple.com".to_string(),
        ..valid.clone()
    };
    assert_eq!(GateDecision::PassThrough, inspect_candidate(&wrong_host));

    let secondary_host = CandidateMeta {
        source_ip: "192.0.2.10".to_string(),
        hostname: "gs-loc-cn.apple.com".to_string(),
        content_length: Some(128),
    };
    assert_eq!(GateDecision::Candidate, inspect_candidate(&secondary_host));

    for hostname in APPROVED_WLOC_HOSTS {
        let candidate = CandidateMeta {
            source_ip: "192.0.2.10".to_string(),
            hostname: hostname.to_string(),
            content_length: Some(128),
        };
        assert_eq!(
            GateDecision::Candidate,
            inspect_candidate(&candidate),
            "approved host {hostname} must remain a candidate",
        );
    }
}

#[test]
fn oversized_or_unknown_bodies_are_not_parsed() {
    let oversized = CandidateMeta {
        source_ip: "192.0.2.10".to_string(),
        hostname: "gs-loc.apple.com".to_string(),
        content_length: Some(MAX_WLOC_BODY_BYTES + 1),
    };
    assert_eq!(GateDecision::PassThrough, inspect_candidate(&oversized));

    let unknown = CandidateMeta {
        content_length: None,
        ..oversized
    };
    assert_eq!(GateDecision::PassThrough, inspect_candidate(&unknown));
}

#[test]
fn prost_roundtrip_has_no_private_wloc_fields() {
    let envelope = roundtrip_synthetic_probe().expect("synthetic protobuf roundtrip");
    assert_eq!(vec![1, 2, 3, 4, 5], envelope.payload);
}

#[test]
fn rustls_builds_both_proxy_sides_with_h2_alpn() {
    let stack = build_tls_stack().expect("TLS client and server configuration");
    let report = stack.report;
    assert!(report.server_configured);
    assert!(report.upstream_verification_configured);
    assert_eq!(1, report.server_alpn_protocols);
    assert_eq!(1, report.client_alpn_protocols);
}

#[test]
fn h2_prior_knowledge_smoke_runs_with_explicit_limits() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");

    runtime
        .block_on(run_h2_prior_knowledge_smoke())
        .expect("h2 smoke test");
}
