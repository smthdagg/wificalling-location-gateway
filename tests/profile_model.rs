use std::net::IpAddr;

use wificalling_location_gateway::config::{
    DeviceProfile, LocationMode, NodeSelectionMode, ProfileError, ProfileModel, WlocUciConfig,
};

fn profile(id: &str, address: &str) -> DeviceProfile {
    DeviceProfile {
        id: id.to_owned(),
        label: "Living room phone".to_owned(),
        assigned_device: Some(address.to_owned()),
        node_ref: "node-a".to_owned(),
        node_mode: NodeSelectionMode::Fixed,
        location_mode: LocationMode::Auto,
        manual_latitude: None,
        manual_longitude: None,
        manual_location_ref: None,
        enabled: true,
    }
}

#[test]
fn legacy_single_device_configuration_migrates_idempotently() {
    let config = WlocUciConfig {
        enabled: true,
        location_mode: LocationMode::Manual,
        manual_latitude: Some(22.3193),
        manual_longitude: Some(114.1694),
        node_ref: "node-a".to_owned(),
        assigned_device: "192.168.1.100".to_owned(),
        ..WlocUciConfig::default()
    };

    let first = ProfileModel::from_legacy(&config).expect("legacy config is valid");
    let second = ProfileModel::from_legacy(&config).expect("legacy config is valid");
    assert_eq!(first, second);
    assert_eq!(first.profiles().len(), 1);
    assert_eq!(first.profiles()[0].id, "default");
    assert_eq!(
        first.profiles()[0].assigned_device.as_deref(),
        Some("192.168.1.100")
    );
    assert_eq!(first.profiles()[0].location_mode, LocationMode::Manual);
    assert_eq!(first.profiles()[0].manual_latitude, Some(22.3193));
}

#[test]
fn explicit_profiles_reject_duplicates_and_invalid_addresses() {
    let duplicate = ProfileModel::new(vec![
        profile("phone", "192.168.1.10"),
        profile("phone", "192.168.1.11"),
    ]);
    assert!(matches!(duplicate, Err(ProfileError::DuplicateId(_))));

    let invalid = ProfileModel::new(vec![profile("phone", "not-an-address")]);
    assert!(matches!(
        invalid,
        Err(ProfileError::InvalidDeviceAddress(_))
    ));

    let duplicate_device = ProfileModel::new(vec![
        profile("phone", "192.168.1.10"),
        profile("tablet", "192.168.1.10"),
    ]);
    assert!(matches!(
        duplicate_device,
        Err(ProfileError::DuplicateAssignedDevice(_))
    ));
}

#[test]
fn validation_accepts_only_private_ipv4_runtime_bindings() {
    let ipv4 = ProfileModel::new(vec![profile("phone", "192.168.1.10")]).unwrap();
    assert_eq!(
        ipv4.profiles()[0].assigned_device.as_deref(),
        Some("192.168.1.10")
    );

    for address in ["fd00::10", "aa:bb:cc:dd:ee:ff"] {
        assert!(matches!(
            ProfileModel::new(vec![profile("tablet", address)]),
            Err(ProfileError::InvalidDeviceAddress(_))
        ));
    }
}

#[test]
fn unusable_ip_and_mac_addresses_are_rejected() {
    for address in [
        "0.0.0.0",
        "0.0.0.1",
        "127.0.0.1",
        "192.0.2.10",
        "169.254.1.2",
        "224.0.0.1",
        "::",
        "::1",
        "ff02::1",
        "2001:db8::10",
        "00:00:00:00:00:00",
        "01:00:00:00:00:01",
    ] {
        let rejected = matches!(
            ProfileModel::new(vec![profile("phone", address)]),
            Err(ProfileError::InvalidDeviceAddress(_))
        );
        assert!(rejected, "address should not be accepted: {address}");
    }
}

#[test]
fn manual_location_requires_a_complete_finite_in_range_pair() {
    let mut value = profile("phone", "192.168.1.10");
    value.location_mode = LocationMode::Manual;
    value.manual_latitude = Some(1.0);
    assert!(matches!(
        ProfileModel::new(vec![value.clone()]),
        Err(ProfileError::IncompleteLocation)
    ));

    value.manual_longitude = Some(181.0);
    assert!(matches!(
        ProfileModel::new(vec![value.clone()]),
        Err(ProfileError::InvalidLocation)
    ));

    value.manual_longitude = Some(2.0);
    value.manual_latitude = Some(f64::NAN);
    assert!(matches!(
        ProfileModel::new(vec![value]),
        Err(ProfileError::InvalidLocation)
    ));
}

#[test]
fn transactional_update_does_not_partially_replace_model() {
    let original = ProfileModel::new(vec![profile("phone", "192.168.1.10")]).unwrap();
    let mut model = original.clone();
    let result = model.replace(vec![profile("phone", "bad")]);
    assert!(result.is_err());
    assert_eq!(model, original);
}

#[test]
fn multiple_profiles_cannot_be_selected_by_the_single_runtime() {
    let model = ProfileModel::new(vec![
        profile("phone", "192.168.1.10"),
        profile("tablet", "192.168.1.11"),
    ])
    .unwrap();
    assert!(matches!(
        model.single_runtime_profile(),
        Err(ProfileError::MultipleProfilesRequireUnifiedRuntime)
    ));
}

#[test]
fn profile_status_is_redacted_and_bounded() {
    let model = ProfileModel::new(vec![profile("phone", "192.168.1.10")]).unwrap();
    let status = model.redacted_status().unwrap();
    let text = serde_json::to_string(&status).unwrap();
    assert!(text.contains("phone"));
    assert!(!text.contains("192.168.1.10"));
    assert!(!text.contains("node-a"));
    assert!(text.len() <= 4096);
}

#[test]
fn profile_count_and_serialized_size_are_bounded() {
    let too_many = (0..=8)
        .map(|index| {
            profile(
                &format!("phone_{index}"),
                &format!("192.168.1.{}", index + 10),
            )
        })
        .collect();
    assert!(matches!(
        ProfileModel::new(too_many),
        Err(ProfileError::TooManyProfiles)
    ));

    let mut oversized = profile("phone", "192.168.1.10");
    oversized.label = "x".repeat(10_000);
    assert!(matches!(
        ProfileModel::new(vec![oversized]),
        Err(ProfileError::FieldTooLong { .. })
    ));
}

#[test]
fn explicit_device_sections_parse_and_profile_model_prefers_them() {
    let text = r#"
config wloc-service 'main'
    option enabled '0'
config device 'phone'
    option label 'Living room phone'
    option assigned_device '192.168.1.100'
    option node_ref 'node-a'
    option node_mode 'fixed'
    option geo_source 'manual'
    option manual_lat '22.3'
    option manual_lon '114.1'
    option manual_location_ref 'hong-kong'
    option enabled '1'
"#;
    let config = WlocUciConfig::parse(text).expect("explicit profile parses");
    assert!(!config.profiles.is_empty());
    let model = config.profile_model().unwrap();
    assert_eq!(model.profiles().len(), 1);
    assert_eq!(model.profiles()[0].id, "phone");
    assert_eq!(
        model.profiles()[0].manual_location_ref.as_deref(),
        Some("hong-kong")
    );
}

#[test]
fn explicit_device_section_rejects_unknown_modes() {
    let text = "config device 'phone'\n\toption node_mode 'random'\n";
    assert!(matches!(
        WlocUciConfig::parse(text),
        Err(wificalling_location_gateway::config::UciError::Profile(_))
    ));
}

#[test]
fn explicit_device_section_requires_an_assigned_address() {
    let text = "config device 'phone'\n\toption node_ref 'node-a'\n";
    assert!(matches!(
        WlocUciConfig::parse(text),
        Err(wificalling_location_gateway::config::UciError::Profile(_))
    ));
}

#[test]
fn oversized_uci_text_is_rejected_before_profile_accumulation() {
    let text = format!(
        "config device 'phone'\n\toption label '{}'\n",
        "x".repeat(33_000)
    );
    assert!(matches!(
        WlocUciConfig::parse(&text),
        Err(wificalling_location_gateway::config::UciError::ConfigTooLarge)
    ));
}

#[test]
fn profile_ids_are_compatible_with_uci_named_sections() {
    let accepted = ProfileModel::new(vec![profile("phone_2", "192.168.1.10")]);
    assert!(accepted.is_ok());

    let rejected = ProfileModel::new(vec![profile("phone-2", "192.168.1.10")]);
    assert!(matches!(rejected, Err(ProfileError::InvalidProfileId(_))));
}

#[test]
fn unused_import_guard_keeps_the_test_explicit_about_ip_support() {
    let _: IpAddr = "192.168.1.10".parse().unwrap();
}
