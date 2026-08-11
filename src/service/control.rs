//! Transactional runtime ordering behind injected OpenWrt adapters.
//!
//! No nftables, procd, socket, or process command is executed here. Adapters
//! implement those operations later and remain subject to separate review.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeFailure;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeStep {
    StartEngine,
    CheckHealth,
    ArmWatchdog,
    InstallRedirect,
    RemoveRedirect,
    VerifyRedirectAbsent,
    DisarmWatchdog,
    DrainEngine,
    StopEngine,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlError {
    InvalidSafetyScope,
    EngineUnhealthy,
    RedirectStillPresent,
    CleanupUnsafe,
    RuntimeFailure(RuntimeStep),
}

pub trait RuntimeControl {
    fn start_engine_passthrough(&mut self) -> Result<(), RuntimeFailure>;
    fn engine_healthy(&mut self) -> Result<bool, RuntimeFailure>;
    fn arm_watchdog(&mut self) -> Result<(), RuntimeFailure>;
    fn install_exact_redirect(&mut self) -> Result<(), RuntimeFailure>;
    fn remove_redirect(&mut self) -> Result<(), RuntimeFailure>;
    fn redirect_present(&mut self) -> Result<bool, RuntimeFailure>;
    fn disarm_watchdog(&mut self) -> Result<(), RuntimeFailure>;
    fn drain_engine(&mut self) -> Result<(), RuntimeFailure>;
    fn stop_engine(&mut self) -> Result<(), RuntimeFailure>;
}

pub fn enable(
    runtime: &mut impl RuntimeControl,
    scope_valid: bool,
    ipv6_ready: bool,
) -> Result<(), ControlError> {
    if !scope_valid || !ipv6_ready {
        return Err(ControlError::InvalidSafetyScope);
    }

    runtime
        .start_engine_passthrough()
        .map_err(|_| ControlError::RuntimeFailure(RuntimeStep::StartEngine))?;

    let result = enable_after_start(runtime);
    if result.is_err() {
        compensate_after_start(runtime)?;
    }
    result
}

fn enable_after_start(runtime: &mut impl RuntimeControl) -> Result<(), ControlError> {
    let healthy = runtime
        .engine_healthy()
        .map_err(|_| ControlError::RuntimeFailure(RuntimeStep::CheckHealth))?;
    if !healthy {
        return Err(ControlError::EngineUnhealthy);
    }
    runtime
        .arm_watchdog()
        .map_err(|_| ControlError::RuntimeFailure(RuntimeStep::ArmWatchdog))?;
    runtime
        .install_exact_redirect()
        .map_err(|_| ControlError::RuntimeFailure(RuntimeStep::InstallRedirect))?;
    Ok(())
}

fn compensate_after_start(runtime: &mut impl RuntimeControl) -> Result<(), ControlError> {
    let removal = runtime.remove_redirect();
    let redirect_absent = runtime.redirect_present();
    if removal.is_err() || !matches!(redirect_absent, Ok(false)) {
        return Err(ControlError::CleanupUnsafe);
    }
    runtime
        .disarm_watchdog()
        .map_err(|_| ControlError::CleanupUnsafe)?;
    runtime
        .stop_engine()
        .map_err(|_| ControlError::CleanupUnsafe)?;
    Ok(())
}

pub fn disable(runtime: &mut impl RuntimeControl) -> Result<(), ControlError> {
    runtime
        .remove_redirect()
        .map_err(|_| ControlError::RuntimeFailure(RuntimeStep::RemoveRedirect))?;
    if runtime
        .redirect_present()
        .map_err(|_| ControlError::RuntimeFailure(RuntimeStep::VerifyRedirectAbsent))?
    {
        return Err(ControlError::RedirectStillPresent);
    }
    runtime
        .disarm_watchdog()
        .map_err(|_| ControlError::RuntimeFailure(RuntimeStep::DisarmWatchdog))?;
    runtime
        .drain_engine()
        .map_err(|_| ControlError::RuntimeFailure(RuntimeStep::DrainEngine))?;
    runtime
        .stop_engine()
        .map_err(|_| ControlError::RuntimeFailure(RuntimeStep::StopEngine))?;
    Ok(())
}
