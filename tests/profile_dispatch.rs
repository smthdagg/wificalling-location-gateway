use std::net::IpAddr;

use wificalling_location_gateway::config::{
    DeviceProfile, LocationMode, NodeSelectionMode, ProfileModel,
};
use wificalling_location_gateway::service::profile_dispatch::{
    ProfilePatchRouter, ProfilePatchRouterError,
};
use wificalling_location_gateway::wloc::PatchTarget;

fn profile(id: &str, assigned_device: &str, enabled: bool) -> DeviceProfile {
    DeviceProfile {
        id: id.to_owned(),
        label: id.to_owned(),
        assigned_device: Some(assigned_device.to_owned()),
        node_ref: format!("node-{id}"),
        node_mode: NodeSelectionMode::Fixed,
        location_mode: LocationMode::Auto,
        manual_latitude: None,
        manual_longitude: None,
        manual_location_ref: None,
        enabled,
    }
}

fn target(latitude: f64, longitude: f64) -> PatchTarget {
    PatchTarget::new(latitude, longitude)
}

#[test]
fn source_device_selects_only_its_profile_target() {
    let model = ProfileModel::new(vec![
        profile("phone", "192.168.1.10", true),
        profile("tablet", "192.168.1.11", true),
    ])
    .unwrap();
    let router = ProfilePatchRouter::new(&model).unwrap();
    router.set_enabled("phone", true).unwrap();
    router.set_enabled("tablet", true).unwrap();
    router.set_target("phone", Some(target(1.0, 2.0))).unwrap();
    router.set_target("tablet", Some(target(3.0, 4.0))).unwrap();

    assert_eq!(
        router.resolve_source("192.168.1.10").unwrap(),
        target(1.0, 2.0)
    );
    assert_eq!(
        router.resolve_source("192.168.1.11").unwrap(),
        target(3.0, 4.0)
    );
    assert_eq!(router.resolve_source("192.168.1.12"), None);
}

#[test]
fn disabling_one_profile_withdraws_only_its_target() {
    let model = ProfileModel::new(vec![
        profile("phone", "192.168.1.10", true),
        profile("tablet", "192.168.1.11", true),
    ])
    .unwrap();
    let router = ProfilePatchRouter::new(&model).unwrap();
    router.set_enabled("phone", true).unwrap();
    router.set_enabled("tablet", true).unwrap();
    router.set_target("phone", Some(target(1.0, 2.0))).unwrap();
    router.set_target("tablet", Some(target(3.0, 4.0))).unwrap();
    router.set_enabled("phone", false).unwrap();

    assert_eq!(router.resolve_source("192.168.1.10"), None);
    assert_eq!(
        router.resolve_source("192.168.1.11").unwrap(),
        target(3.0, 4.0)
    );
}

#[test]
fn manual_clear_withdraws_target_until_auto_refreshes() {
    let model = ProfileModel::new(vec![profile("phone", "192.168.1.10", true)]).unwrap();
    let router = ProfilePatchRouter::new(&model).unwrap();
    router.set_enabled("phone", true).unwrap();
    router.set_target("phone", Some(target(1.0, 2.0))).unwrap();
    router.clear_target("phone").unwrap();
    assert_eq!(router.resolve_source("192.168.1.10"), None);
    router.set_target("phone", Some(target(5.0, 6.0))).unwrap();
    assert_eq!(
        router.resolve_source("192.168.1.10").unwrap(),
        target(5.0, 6.0)
    );
}

#[test]
fn invalid_source_and_unsupported_mac_never_fall_back() {
    let model = ProfileModel::new(vec![profile("phone", "aa:bb:cc:dd:ee:ff", true)]).unwrap();
    assert!(matches!(
        ProfilePatchRouter::new(&model),
        Err(ProfilePatchRouterError::UnsupportedDevice)
    ));

    let model = ProfileModel::new(vec![profile("phone", "192.168.1.10", true)]).unwrap();
    let router = ProfilePatchRouter::new(&model).unwrap();
    router.set_enabled("phone", true).unwrap();
    router.set_target("phone", Some(target(1.0, 2.0))).unwrap();
    assert_eq!(router.resolve_source("not-an-ip"), None);
    assert_eq!(router.resolve_source("192.168.1.11"), None);
}

#[test]
fn router_rejects_non_ipv4_runtime_binding_explicitly() {
    let model = ProfileModel::new(vec![profile("phone", "fd00::10", true)]).unwrap();
    assert!(matches!(
        ProfilePatchRouter::new(&model),
        Err(ProfilePatchRouterError::UnsupportedDevice)
    ));
}

#[test]
fn device_addresses_are_canonicalized_before_lookup() {
    let model = ProfileModel::new(vec![profile("phone", "192.168.001.010", true)]);
    assert!(model.is_err());

    let model = ProfileModel::new(vec![profile("phone", "192.168.1.10", true)]).unwrap();
    let router = ProfilePatchRouter::new(&model).unwrap();
    router.set_enabled("phone", true).unwrap();
    router.set_target("phone", Some(target(1.0, 2.0))).unwrap();
    let source: IpAddr = "192.168.1.10".parse().unwrap();
    assert_eq!(router.resolve_ip(source).unwrap(), target(1.0, 2.0));
}
