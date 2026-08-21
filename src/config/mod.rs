//! Persisted daemon configuration.
//!
//! The daemon reads `/etc/config/wloc-service` (UCI) once at startup and
//! applies it to the control plane: enabled state, location mode, manual
//! coordinates, presets, and the exit-probe wiring. LuCI and the root-only
//! profile control adapter both use the native `uci` transaction boundary;
//! keeping the parser inside the daemon prevents unvalidated config from
//! reaching runtime or nftables.

pub mod profile;
pub mod uci;

pub use profile::{
    validate_device_address, validate_location_ref, validate_node_ref, validate_profile_id,
    validate_profile_label, DeviceProfile, NodeSelectionMode, ProfileError, ProfileModel,
    RuntimeProfile,
};
pub use uci::{LocationMode, Preset, UciError, WlocUciConfig};
