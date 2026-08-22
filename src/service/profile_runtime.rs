//! Runtime ownership for independent v2 device profiles.
//!
//! A profile owns only its own WLOC redirect and status. Gateway/sing-box is a
//! shared resource: it is prepared and health-checked once, then profiles are
//! admitted independently. The manager never selects an arbitrary profile or
//! node and never stops the shared Gateway when one profile fails.

use std::net::IpAddr;

use crate::config::{DeviceProfile, ProfileModel};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileRuntimePhase {
    Disabled,
    Preparing,
    Passthrough,
    Intercepting,
    DegradedPassthrough,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileRuntimeStatus {
    pub profile_id: String,
    pub phase: ProfileRuntimePhase,
    pub reason_code: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileRuntimeError {
    UnknownProfile,
    ProfileDisabled,
    UnsupportedDevice,
    EngineStart,
    EngineUnhealthy,
    RedirectInstall,
    RedirectStillPresent,
    CleanupUnsafe,
}

/// OpenWrt adapter implemented by the product runtime.
///
/// Methods are profile-scoped for redirect operations. Engine operations are
/// deliberately shared so adding a profile cannot create another sing-box or
/// WLOC process on a small gateway.
pub trait ProfileRuntimeControl {
    fn ensure_shared_engine(&mut self) -> Result<(), ProfileRuntimeError>;
    fn shared_engine_healthy(&mut self) -> Result<bool, ProfileRuntimeError>;
    fn install_profile_redirect(
        &mut self,
        profile: &DeviceProfile,
    ) -> Result<(), ProfileRuntimeError>;
    fn remove_profile_redirect(&mut self, profile_id: &str) -> Result<(), ProfileRuntimeError>;
    fn profile_redirect_present(&mut self, profile_id: &str) -> Result<bool, ProfileRuntimeError>;
}

pub struct ProfileRuntimeManager<R: ProfileRuntimeControl> {
    model: ProfileModel,
    runtime: R,
    statuses: Vec<ProfileRuntimeStatus>,
    shared_engine_ready: bool,
}

impl<R: ProfileRuntimeControl> ProfileRuntimeManager<R> {
    pub fn new(model: ProfileModel, runtime: R) -> Self {
        let statuses = model
            .profiles()
            .iter()
            .map(|profile| ProfileRuntimeStatus {
                profile_id: profile.id.clone(),
                phase: ProfileRuntimePhase::Disabled,
                reason_code: "disabled",
            })
            .collect();
        Self {
            model,
            runtime,
            statuses,
            shared_engine_ready: false,
        }
    }

    pub fn model(&self) -> &ProfileModel {
        &self.model
    }

    pub fn runtime(&self) -> &R {
        &self.runtime
    }

    pub fn runtime_mut(&mut self) -> &mut R {
        &mut self.runtime
    }

    pub fn statuses(&self) -> &[ProfileRuntimeStatus] {
        &self.statuses
    }

    pub fn status(&self, profile_id: &str) -> Result<&ProfileRuntimeStatus, ProfileRuntimeError> {
        self.statuses
            .iter()
            .find(|status| status.profile_id == profile_id)
            .ok_or(ProfileRuntimeError::UnknownProfile)
    }

    pub fn enable(&mut self, profile_id: &str) -> Result<(), ProfileRuntimeError> {
        let index = self.profile_index(profile_id)?;
        let profile = self.model.profiles()[index].clone();
        if !profile.enabled {
            self.set_status(index, ProfileRuntimePhase::Disabled, "profile_disabled");
            return Err(ProfileRuntimeError::ProfileDisabled);
        }
        if !runtime_device_supported(&profile) {
            self.set_status(
                index,
                ProfileRuntimePhase::DegradedPassthrough,
                "unsupported_device_binding",
            );
            return Err(ProfileRuntimeError::UnsupportedDevice);
        }
        if self.statuses[index].phase == ProfileRuntimePhase::Intercepting {
            return Ok(());
        }

        self.set_status(index, ProfileRuntimePhase::Preparing, "preparing");
        if !self.shared_engine_ready && self.runtime.ensure_shared_engine().is_err() {
            self.set_status(
                index,
                ProfileRuntimePhase::DegradedPassthrough,
                "engine_start_failed",
            );
            return Err(ProfileRuntimeError::EngineStart);
        }
        // Health is sampled for every new profile admission. The shared
        // process may have degraded after an earlier profile was enabled.
        match self.runtime.shared_engine_healthy() {
            Ok(true) => self.shared_engine_ready = true,
            Ok(false) | Err(_) => {
                self.shared_engine_ready = false;
                self.set_status(
                    index,
                    ProfileRuntimePhase::DegradedPassthrough,
                    "engine_unhealthy",
                );
                return Err(ProfileRuntimeError::EngineUnhealthy);
            }
        }

        if let Err(error) = self.runtime.install_profile_redirect(&profile) {
            return self.fail_profile_redirect(index, error, profile_id);
        }
        match self.runtime.profile_redirect_present(profile_id) {
            Ok(true) => {
                self.set_status(index, ProfileRuntimePhase::Intercepting, "intercepting");
                Ok(())
            }
            Ok(false) => self.fail_profile_redirect(
                index,
                ProfileRuntimeError::RedirectStillPresent,
                profile_id,
            ),
            Err(_) => {
                self.fail_profile_redirect(index, ProfileRuntimeError::CleanupUnsafe, profile_id)
            }
        }
    }

    pub fn disable(&mut self, profile_id: &str) -> Result<(), ProfileRuntimeError> {
        let index = self.profile_index(profile_id)?;
        if self.statuses[index].phase == ProfileRuntimePhase::Disabled {
            return Ok(());
        }
        if self.runtime.remove_profile_redirect(profile_id).is_err()
            || self
                .runtime
                .profile_redirect_present(profile_id)
                .unwrap_or(true)
        {
            self.set_status(
                index,
                ProfileRuntimePhase::DegradedPassthrough,
                "cleanup_unsafe",
            );
            return Err(ProfileRuntimeError::CleanupUnsafe);
        }
        match self.runtime.profile_redirect_present(profile_id) {
            Ok(false) => {
                self.set_status(index, ProfileRuntimePhase::Disabled, "disabled");
                Ok(())
            }
            Ok(true) | Err(_) => {
                self.set_status(
                    index,
                    ProfileRuntimePhase::DegradedPassthrough,
                    "cleanup_unsafe",
                );
                Err(ProfileRuntimeError::CleanupUnsafe)
            }
        }
    }

    pub fn reload(&mut self, profile_id: &str) -> Result<(), ProfileRuntimeError> {
        let phase = self.status(profile_id)?.phase;
        if phase != ProfileRuntimePhase::Disabled {
            self.disable(profile_id)?;
        }
        self.enable(profile_id)
    }

    fn profile_index(&self, profile_id: &str) -> Result<usize, ProfileRuntimeError> {
        self.model
            .profiles()
            .iter()
            .position(|profile| profile.id == profile_id)
            .ok_or(ProfileRuntimeError::UnknownProfile)
    }

    fn set_status(&mut self, index: usize, phase: ProfileRuntimePhase, reason_code: &'static str) {
        self.statuses[index] = ProfileRuntimeStatus {
            profile_id: self.statuses[index].profile_id.clone(),
            phase,
            reason_code,
        };
    }

    fn fail_profile_redirect(
        &mut self,
        index: usize,
        error: ProfileRuntimeError,
        profile_id: &str,
    ) -> Result<(), ProfileRuntimeError> {
        if self.runtime.remove_profile_redirect(profile_id).is_err() {
            self.set_status(
                index,
                ProfileRuntimePhase::DegradedPassthrough,
                "cleanup_unsafe",
            );
            return Err(ProfileRuntimeError::CleanupUnsafe);
        }
        match self.runtime.profile_redirect_present(profile_id) {
            Ok(true) | Err(_) => {
                self.set_status(
                    index,
                    ProfileRuntimePhase::DegradedPassthrough,
                    "cleanup_unsafe",
                );
                return Err(ProfileRuntimeError::CleanupUnsafe);
            }
            Ok(false) => {}
        }
        self.set_status(
            index,
            ProfileRuntimePhase::DegradedPassthrough,
            reason_for(error),
        );
        Err(error)
    }
}

fn runtime_device_supported(profile: &DeviceProfile) -> bool {
    profile
        .assigned_device
        .as_deref()
        .and_then(|address| address.parse::<IpAddr>().ok())
        .is_some()
}

fn reason_for(error: ProfileRuntimeError) -> &'static str {
    match error {
        ProfileRuntimeError::RedirectInstall => "redirect_install_failed",
        ProfileRuntimeError::RedirectStillPresent => "redirect_not_confirmed",
        ProfileRuntimeError::CleanupUnsafe => "cleanup_unsafe",
        _ => "profile_runtime_failed",
    }
}
