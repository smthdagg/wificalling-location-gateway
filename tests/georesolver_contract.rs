use std::net::{IpAddr, Ipv4Addr};

use wificalling_location_gateway::georesolver::{
    resolve_candidates, select_cached, GeoCacheEntry, GeoCandidate, GeoResolution,
    GeoResolverError, ProviderRef,
};
use wificalling_location_gateway::service::GeoRecord;

fn exit_ip() -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))
}

fn london() -> GeoRecord {
    GeoRecord {
        country_code: "GB".to_owned(),
        latitude: 51.5074,
        longitude: -0.1278,
        timezone: "Europe/London".to_owned(),
        expires_at_unix: 2_000,
    }
}

fn candidate(provider: &str, record: GeoRecord) -> GeoCandidate {
    GeoCandidate::new(
        ProviderRef::new(provider).expect("safe provider reference"),
        exit_ip(),
        record,
    )
}

#[test]
fn one_valid_provider_result_is_usable_without_inventing_a_fallback() {
    assert_eq!(
        resolve_candidates(exit_ip(), &[candidate("primary", london())], 1_000)
            .expect("bounded candidate set"),
        GeoResolution::Fresh(london())
    );

    assert_eq!(
        resolve_candidates(exit_ip(), &[], 1_000).expect("empty set is bounded"),
        GeoResolution::Unavailable
    );
}

#[test]
fn invalid_or_wrong_exit_results_are_ignored_and_never_become_default_geo() {
    let wrong_exit = GeoCandidate::new(
        ProviderRef::new("primary").expect("safe provider reference"),
        IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
        london(),
    );
    let invalid_geo = candidate(
        "backup",
        GeoRecord {
            latitude: f64::NAN,
            ..london()
        },
    );

    assert_eq!(
        resolve_candidates(exit_ip(), &[wrong_exit, invalid_geo], 1_000)
            .expect("bounded candidate set"),
        GeoResolution::Unavailable
    );
}

#[test]
fn conflicting_valid_providers_are_marked_uncertain() {
    let conflict = GeoRecord {
        country_code: "US".to_owned(),
        latitude: 37.7749,
        longitude: -122.4194,
        timezone: "America/Los_Angeles".to_owned(),
        expires_at_unix: 2_000,
    };

    assert_eq!(
        resolve_candidates(
            exit_ip(),
            &[
                candidate("primary", london()),
                candidate("backup", conflict)
            ],
            1_000,
        )
        .expect("bounded candidate set"),
        GeoResolution::Uncertain
    );
}

#[test]
fn cache_is_bound_to_the_exit_ip_and_expiry() {
    let cache = GeoCacheEntry::new(exit_ip(), london());
    assert_eq!(select_cached(&cache, exit_ip(), 1_000), Some(london()));
    assert_eq!(
        select_cached(&cache, IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 1_000,),
        None
    );
    assert_eq!(select_cached(&cache, exit_ip(), 2_000), None);
}

#[test]
fn provider_and_candidate_count_limits_are_strict() {
    for value in ["", "provider secret", "https://provider.invalid"] {
        assert_eq!(
            ProviderRef::new(value).unwrap_err(),
            GeoResolverError::InvalidProviderRef
        );
    }

    let candidates = [
        candidate("one", london()),
        candidate("two", london()),
        candidate("three", london()),
    ];
    assert_eq!(
        resolve_candidates(exit_ip(), &candidates, 1_000).unwrap_err(),
        GeoResolverError::TooManyCandidates
    );
}

#[test]
fn semantic_placeholders_and_excessive_ttl_are_unavailable() {
    for record in [
        GeoRecord {
            country_code: "ZZ".to_owned(),
            ..london()
        },
        GeoRecord {
            timezone: "+".to_owned(),
            ..london()
        },
        GeoRecord {
            expires_at_unix: 10_000,
            ..london()
        },
    ] {
        assert_eq!(
            resolve_candidates(exit_ip(), &[candidate("primary", record)], 1_000)
                .expect("bounded candidate set"),
            GeoResolution::Unavailable
        );
    }
}

#[test]
fn valid_iso_boundary_codes_are_not_lost_by_the_compact_allowlist() {
    for country_code in ["AM", "BL", "CG", "LY", "VE"] {
        let record = GeoRecord {
            country_code: country_code.to_owned(),
            ..london()
        };
        assert_eq!(
            resolve_candidates(exit_ip(), &[candidate("primary", record.clone())], 1_000)
                .expect("bounded candidate set"),
            GeoResolution::Fresh(record),
            "rejected valid ISO code {country_code}"
        );
    }
}

#[test]
fn timezone_shape_rejects_empty_path_segments() {
    for timezone in ["/", "A/", "/A", "A//B"] {
        let record = GeoRecord {
            timezone: timezone.to_owned(),
            ..london()
        };
        assert_eq!(
            resolve_candidates(exit_ip(), &[candidate("primary", record)], 1_000)
                .expect("bounded candidate set"),
            GeoResolution::Unavailable,
            "accepted invalid timezone {timezone}"
        );
    }
}
