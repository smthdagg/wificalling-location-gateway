//! Geo-resolver provider runtime contract.
//!
//! The pure [`resolve_candidates`] logic is driven by a provider adapter that
//! performs the real network lookups. Provider failures are fail-open: the
//! result degrades to `Unavailable`, never to a fabricated coordinate.

use std::net::{IpAddr, Ipv4Addr};

use wificalling_location_gateway::georesolver::runtime::{
    resolve_geo, GeoProviderRuntime, ProviderFailure,
};
use wificalling_location_gateway::georesolver::{GeoResolution, ProviderRef};
use wificalling_location_gateway::service::GeoRecord;

const EXIT_V4: IpAddr = IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8));
const OTHER_V4: IpAddr = IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1));

fn valid_record(now_unix: u64, latitude: f64, longitude: f64) -> GeoRecord {
    GeoRecord {
        country_code: "US".to_owned(),
        latitude,
        longitude,
        timezone: "America/Los_Angeles".to_owned(),
        expires_at_unix: now_unix + 3_600,
    }
}

struct StubProvider {
    results: Vec<Result<Option<(IpAddr, GeoRecord)>, ProviderFailure>>,
    index: usize,
}

impl StubProvider {
    fn single(result: Result<Option<(IpAddr, GeoRecord)>, ProviderFailure>) -> Self {
        Self {
            results: vec![result],
            index: 0,
        }
    }
    fn pair(
        first: Result<Option<(IpAddr, GeoRecord)>, ProviderFailure>,
        second: Result<Option<(IpAddr, GeoRecord)>, ProviderFailure>,
    ) -> Self {
        Self {
            results: vec![first, second],
            index: 0,
        }
    }
}

impl GeoProviderRuntime for StubProvider {
    fn lookup(
        &mut self,
        _provider: ProviderRef,
        _ip: IpAddr,
    ) -> Result<Option<(IpAddr, GeoRecord)>, ProviderFailure> {
        let result = self.results[self.index].clone();
        self.index += 1;
        result
    }
}

fn provider(name: &str) -> ProviderRef {
    ProviderRef::new(name).unwrap()
}

#[test]
fn matching_provider_record_is_fresh() {
    let now = 1_000_000;
    let record = valid_record(now, 37.77, -122.41);
    let mut runtime = StubProvider::single(Ok(Some((EXIT_V4, record.clone()))));

    let resolution = resolve_geo(&mut runtime, EXIT_V4, &[provider("geo-a")], now);
    assert_eq!(resolution, GeoResolution::Fresh(record));
}

#[test]
fn conflicting_provider_records_are_uncertain() {
    let now = 1_000_000;
    let mut runtime = StubProvider::pair(
        Ok(Some((EXIT_V4, valid_record(now, 37.77, -122.41)))),
        Ok(Some((EXIT_V4, valid_record(now, 51.50, -0.12)))),
    );

    let resolution = resolve_geo(
        &mut runtime,
        EXIT_V4,
        &[provider("geo-a"), provider("geo-b")],
        now,
    );
    assert_eq!(resolution, GeoResolution::Uncertain);
}

#[test]
fn one_failing_provider_does_not_poison_the_other() {
    let now = 1_000_000;
    let record = valid_record(now, 37.77, -122.41);
    let mut runtime = StubProvider::pair(
        Err(ProviderFailure::Timeout),
        Ok(Some((EXIT_V4, record.clone()))),
    );

    let resolution = resolve_geo(
        &mut runtime,
        EXIT_V4,
        &[provider("geo-a"), provider("geo-b")],
        now,
    );
    assert_eq!(resolution, GeoResolution::Fresh(record));
}

#[test]
fn all_providers_failing_is_unavailable() {
    let now = 1_000_000;
    let mut runtime = StubProvider::pair(
        Err(ProviderFailure::Unreachable),
        Err(ProviderFailure::InvalidData),
    );

    let resolution = resolve_geo(
        &mut runtime,
        EXIT_V4,
        &[provider("geo-a"), provider("geo-b")],
        now,
    );
    assert_eq!(resolution, GeoResolution::Unavailable);
}

#[test]
fn record_for_a_different_exit_is_filtered() {
    let now = 1_000_000;
    let mut runtime = StubProvider::single(Ok(Some((OTHER_V4, valid_record(now, 37.77, -122.41)))));

    let resolution = resolve_geo(&mut runtime, EXIT_V4, &[provider("geo-a")], now);
    assert_eq!(resolution, GeoResolution::Unavailable);
}

#[test]
fn syntactically_valid_but_expired_record_is_filtered() {
    let now = 1_000_000;
    let mut expired = valid_record(now, 37.77, -122.41);
    expired.expires_at_unix = now - 1; // expired
    let mut runtime = StubProvider::single(Ok(Some((EXIT_V4, expired))));

    let resolution = resolve_geo(&mut runtime, EXIT_V4, &[provider("geo-a")], now);
    assert_eq!(resolution, GeoResolution::Unavailable);
}

#[test]
fn provider_with_no_data_is_unavailable() {
    let now = 1_000_000;
    let mut runtime = StubProvider::single(Ok(None));

    let resolution = resolve_geo(&mut runtime, EXIT_V4, &[provider("geo-a")], now);
    assert_eq!(resolution, GeoResolution::Unavailable);
}
