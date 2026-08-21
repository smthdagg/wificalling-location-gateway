//! Source-device routing for per-profile WLOC patch targets.
//!
//! The router is deliberately independent from the shared Gateway lifecycle.
//! It maps one validated LAN IP to one profile and reads only that profile's
//! current target. There is no default profile, first-match fallback, or
//! cross-profile target reuse when a route is missing, disabled, or degraded.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex};

use crate::config::ProfileModel;
use crate::wloc::PatchTarget;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfilePatchRouterError {
    UnknownProfile,
    UnsupportedDevice,
    InvalidTarget,
    StatePoisoned,
}

#[derive(Clone)]
struct ProfileRoute {
    assigned_device: IpAddr,
    enabled: Arc<Mutex<bool>>,
    target: Arc<Mutex<Option<PatchTarget>>>,
}

/// A bounded, immutable route table with mutable per-profile state.
#[derive(Clone)]
pub struct ProfilePatchRouter {
    routes: Arc<HashMap<String, ProfileRoute>>,
    by_device: Arc<HashMap<IpAddr, String>>,
}

impl ProfilePatchRouter {
    /// Build a route table from the validated profile model. Runtime routing
    /// currently accepts only IPv4 device bindings because the OpenWrt
    /// profile redirect helper is IPv4-scoped; MAC bindings are not silently
    /// interpreted as a different device.
    pub fn new(model: &ProfileModel) -> Result<Self, ProfilePatchRouterError> {
        let mut routes = HashMap::with_capacity(model.profiles().len());
        let mut by_device = HashMap::with_capacity(model.profiles().len());
        for profile in model.profiles() {
            let Some(address) = profile.assigned_device.as_deref() else {
                return Err(ProfilePatchRouterError::UnsupportedDevice);
            };
            let ip = address
                .parse::<IpAddr>()
                .map_err(|_| ProfilePatchRouterError::UnsupportedDevice)?;
            if !matches!(ip, IpAddr::V4(_)) {
                return Err(ProfilePatchRouterError::UnsupportedDevice);
            }
            if by_device.insert(ip, profile.id.clone()).is_some() {
                return Err(ProfilePatchRouterError::UnsupportedDevice);
            }
            routes.insert(
                profile.id.clone(),
                ProfileRoute {
                    assigned_device: ip,
                    // UCI enablement is intent only. The route becomes live
                    // after the runtime manager installs and verifies the
                    // profile redirect.
                    enabled: Arc::new(Mutex::new(false)),
                    target: Arc::new(Mutex::new(None)),
                },
            );
        }
        Ok(Self {
            routes: Arc::new(routes),
            by_device: Arc::new(by_device),
        })
    }

    pub fn profile_sink(
        &self,
        profile_id: &str,
    ) -> Result<Arc<Mutex<Option<PatchTarget>>>, ProfilePatchRouterError> {
        self.route(profile_id)
            .map(|route| Arc::clone(&route.target))
    }

    pub fn set_enabled(
        &self,
        profile_id: &str,
        enabled: bool,
    ) -> Result<(), ProfilePatchRouterError> {
        let route = self.route(profile_id)?;
        *route
            .enabled
            .lock()
            .map_err(|_| ProfilePatchRouterError::StatePoisoned)? = enabled;
        if !enabled {
            self.clear_target(profile_id)?;
        }
        Ok(())
    }

    pub fn set_target(
        &self,
        profile_id: &str,
        target: Option<PatchTarget>,
    ) -> Result<(), ProfilePatchRouterError> {
        let route = self.route(profile_id)?;
        if let Some(target) = target {
            validate_target(target)?;
            *route
                .target
                .lock()
                .map_err(|_| ProfilePatchRouterError::StatePoisoned)? = Some(target);
        } else {
            self.clear_target(profile_id)?;
        }
        Ok(())
    }

    pub fn clear_target(&self, profile_id: &str) -> Result<(), ProfilePatchRouterError> {
        let route = self.route(profile_id)?;
        *route
            .target
            .lock()
            .map_err(|_| ProfilePatchRouterError::StatePoisoned)? = None;
        Ok(())
    }

    /// Resolve a textual source address from the accepted TCP connection.
    /// Invalid input and unknown addresses intentionally return no target.
    pub fn resolve_source(&self, source: &str) -> Option<PatchTarget> {
        source
            .parse::<IpAddr>()
            .ok()
            .and_then(|ip| self.resolve_ip(ip))
    }

    pub fn resolve_ip(&self, source: IpAddr) -> Option<PatchTarget> {
        let profile_id = self.by_device.get(&source)?;
        let route = self.routes.get(profile_id)?;
        if !*route.enabled.lock().ok()? {
            return None;
        }
        *route.target.lock().ok()?
    }

    pub fn profile_for_ip(&self, source: IpAddr) -> Option<&str> {
        self.by_device.get(&source).map(String::as_str)
    }

    pub fn assigned_device(&self, profile_id: &str) -> Result<Ipv4Addr, ProfilePatchRouterError> {
        match self.route(profile_id)?.assigned_device {
            IpAddr::V4(ip) => Ok(ip),
            IpAddr::V6(_) => Err(ProfilePatchRouterError::UnsupportedDevice),
        }
    }

    fn route(&self, profile_id: &str) -> Result<&ProfileRoute, ProfilePatchRouterError> {
        self.routes
            .get(profile_id)
            .ok_or(ProfilePatchRouterError::UnknownProfile)
    }
}

fn validate_target(target: PatchTarget) -> Result<(), ProfilePatchRouterError> {
    if target.latitude.is_finite()
        && target.longitude.is_finite()
        && (-90.0..=90.0).contains(&target.latitude)
        && (-180.0..=180.0).contains(&target.longitude)
    {
        Ok(())
    } else {
        Err(ProfilePatchRouterError::InvalidTarget)
    }
}

#[cfg(test)]
mod tests {
    use super::validate_target;
    use crate::wloc::PatchTarget;

    #[test]
    fn target_validation_rejects_non_finite_coordinates() {
        assert!(validate_target(PatchTarget::new(f64::NAN, 0.0)).is_err());
        assert!(validate_target(PatchTarget::new(0.0, f64::INFINITY)).is_err());
    }
}
