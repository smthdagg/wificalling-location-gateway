//! Persisted daemon configuration.
//!
//! The daemon reads `/etc/config/wloc-service` (UCI) once at startup and
//! applies it to the control plane: enabled state, location mode, manual
//! coordinates, presets, and the exit-probe wiring. LuCI (via `uci`) is the
//! only writer; keeping the parser inside the daemon means the root-only
//! control API stays the single runtime write path.

pub mod profile;
pub mod uci;

pub use profile::{
    validate_device_address, validate_location_ref, validate_node_ref, validate_profile_id,
    validate_profile_label, DeviceProfile, NodeSelectionMode, ProfileError, ProfileModel,
    RuntimeProfile,
};
pub use uci::{LocationMode, Preset, UciError, WlocUciConfig};
