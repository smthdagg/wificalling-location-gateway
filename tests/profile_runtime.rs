use wificalling_location_gateway::config::{
    DeviceProfile, LocationMode, NodeSelectionMode, ProfileModel,
};
use wificalling_location_gateway::service::profile_runtime::{
    ProfileRuntimeControl, ProfileRuntimeError, ProfileRuntimeManager, ProfileRuntimePhase,
};

fn profile(id: &str, address: &str) -> DeviceProfile {
    DeviceProfile {
        id: id.to_owned(),
        label: id.to_owned(),
        assigned_device: Some(address.to_owned()),
        node_ref: format!("node-{id}"),
        node_mode: NodeSelectionMode::Fixed,
        location_mode: LocationMode::Auto,
        manual_latitude: None,
        manual_longitude: None,
        manual_location_ref: None,
        enabled: true,
    }
}

#[derive(Default, Debug)]
struct FakeRuntime {
    operations: Vec<String>,
    healthy: bool,
    fail_install_for: Option<String>,
    redirects: Vec<String>,
}

impl ProfileRuntimeControl for FakeRuntime {
    fn ensure_shared_engine(&mut self) -> Result<(), ProfileRuntimeError> {
        self.operations.push("engine.start".to_owned());
        Ok(())
    }

    fn shared_engine_healthy(&mut self) -> Result<bool, ProfileRuntimeError> {
        self.operations.push("engine.health".to_owned());
        Ok(self.healthy)
    }

    fn install_profile_redirect(
        &mut self,
        profile: &DeviceProfile,
    ) -> Result<(), ProfileRuntimeError> {
        self.operations.push(format!("redirect.add:{}", profile.id));
        if self.fail_install_for.as_deref() == Some(profile.id.as_str()) {
            return Err(ProfileRuntimeError::RedirectInstall);
        }
        self.redirects.push(profile.id.clone());
        Ok(())
    }

    fn remove_profile_redirect(&mut self, profile_id: &str) -> Result<(), ProfileRuntimeError> {
        self.operations
            .push(format!("redirect.remove:{profile_id}"));
        self.redirects.retain(|id| id != profile_id);
        Ok(())
    }

    fn profile_redirect_present(
        &mut self,
        profile_id: &str,
    ) -> Result<bool, ProfileRuntimeError> {
        self.operations
            .push(format!("redirect.present:{profile_id}"));
        Ok(self.redirects.iter().any(|id| id == profile_id))
    }
}

fn manager(runtime: FakeRuntime) -> ProfileRuntimeManager<FakeRuntime> {
    let model = ProfileModel::new(vec![
        profile("phone", "192.168.1.10"),
        profile("tablet", "192.168.1.11"),
    ])
    .unwrap();
    ProfileRuntimeManager::new(model, runtime)
}

#[test]
fn profiles_share_one_engine_but_install_independent_redirects() {
    let mut manager = manager(FakeRuntime {
        healthy: true,
        ..FakeRuntime::default()
    });

    manager.enable("phone").unwrap();
    manager.enable("tablet").unwrap();

    assert_eq!(manager.status("phone").unwrap().phase, ProfileRuntimePhase::Intercepting);
    assert_eq!(manager.status("tablet").unwrap().phase, ProfileRuntimePhase::Intercepting);
    assert_eq!(
        manager
            .runtime()
            .operations
            .iter()
            .filter(|operation| operation.as_str() == "engine.start")
            .count(),
        1
    );
    assert!(manager.runtime().redirects.contains(&"phone".to_owned()));
    assert!(manager.runtime().redirects.contains(&"tablet".to_owned()));
}

#[test]
fn disabling_one_profile_does_not_touch_the_other_redirect() {
    let mut manager = manager(FakeRuntime {
        healthy: true,
        ..FakeRuntime::default()
    });
    manager.enable("phone").unwrap();
    manager.enable("tablet").unwrap();

    manager.disable("phone").unwrap();

    assert_eq!(manager.status("phone").unwrap().phase, ProfileRuntimePhase::Disabled);
    assert_eq!(manager.status("tablet").unwrap().phase, ProfileRuntimePhase::Intercepting);
    assert_eq!(manager.runtime().redirects, vec!["tablet".to_owned()]);
    assert!(manager
        .runtime()
        .operations
        .contains(&"redirect.remove:phone".to_owned()));
    assert!(!manager
        .runtime()
        .operations
        .contains(&"redirect.remove:tablet".to_owned()));
}

#[test]
fn failed_profile_install_degrades_only_that_profile() {
    let mut manager = manager(FakeRuntime {
        healthy: true,
        fail_install_for: Some("tablet".to_owned()),
        ..FakeRuntime::default()
    });
    manager.enable("phone").unwrap();

    assert_eq!(
        manager.enable("tablet"),
        Err(ProfileRuntimeError::RedirectInstall)
    );
    assert_eq!(manager.status("phone").unwrap().phase, ProfileRuntimePhase::Intercepting);
    assert_eq!(manager.status("tablet").unwrap().phase, ProfileRuntimePhase::DegradedPassthrough);
    assert_eq!(manager.runtime().redirects, vec!["phone".to_owned()]);
    assert!(manager
        .runtime()
        .operations
        .contains(&"redirect.remove:tablet".to_owned()));
}

#[test]
fn unhealthy_shared_engine_never_installs_any_profile_redirect() {
    let mut manager = manager(FakeRuntime::default());
    assert_eq!(
        manager.enable("phone"),
        Err(ProfileRuntimeError::EngineUnhealthy)
    );
    assert_eq!(manager.status("phone").unwrap().phase, ProfileRuntimePhase::DegradedPassthrough);
    assert!(manager.runtime().redirects.is_empty());
}

#[test]
fn every_new_profile_enable_rechecks_shared_engine_health() {
    let mut manager = manager(FakeRuntime {
        healthy: true,
        ..FakeRuntime::default()
    });
    manager.enable("phone").unwrap();
    manager.runtime_mut().healthy = false;

    assert_eq!(
        manager.enable("tablet"),
        Err(ProfileRuntimeError::EngineUnhealthy)
    );
    assert_eq!(manager.status("phone").unwrap().phase, ProfileRuntimePhase::Intercepting);
    assert_eq!(manager.status("tablet").unwrap().phase, ProfileRuntimePhase::DegradedPassthrough);
    assert_eq!(manager.runtime().redirects, vec!["phone".to_owned()]);
}

#[test]
fn unsupported_mac_profile_is_rejected_without_runtime_operation() {
    let model = ProfileModel::new(vec![profile("phone", "aa:bb:cc:dd:ee:ff")]).unwrap();
    let mut manager = ProfileRuntimeManager::new(
        model,
        FakeRuntime {
            healthy: true,
            ..FakeRuntime::default()
        },
    );

    assert_eq!(
        manager.enable("phone"),
        Err(ProfileRuntimeError::UnsupportedDevice)
    );
    assert_eq!(manager.status("phone").unwrap().phase, ProfileRuntimePhase::DegradedPassthrough);
    assert!(manager.runtime().operations.is_empty());
}

#[test]
fn unknown_profile_is_rejected_without_runtime_operation() {
    let mut manager = manager(FakeRuntime {
        healthy: true,
        ..FakeRuntime::default()
    });
    assert_eq!(
        manager.enable("missing"),
        Err(ProfileRuntimeError::UnknownProfile)
    );
    assert!(manager.runtime().operations.is_empty());
}
