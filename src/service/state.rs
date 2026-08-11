//! Pure lifecycle reducer for redirect safety ordering.

pub use super::ResponseMode;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServicePhase {
    Disabled,
    Starting,
    ReadyPassThrough,
    Intercepting,
    DegradedPassThrough,
    Draining,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SafetyState {
    redirect_present: bool,
    engine_ready: bool,
    watchdog_armed: bool,
    scope_valid: bool,
    ipv6_ready: bool,
}

impl SafetyState {
    pub const fn redirect_present(self) -> bool {
        self.redirect_present
    }

    pub const fn engine_ready(self) -> bool {
        self.engine_ready
    }

    pub const fn watchdog_armed(self) -> bool {
        self.watchdog_armed
    }

    pub const fn scope_valid(self) -> bool {
        self.scope_valid
    }

    pub const fn ipv6_ready(self) -> bool {
        self.ipv6_ready
    }

    const fn redirect_prerequisites_met(self) -> bool {
        self.engine_ready && self.watchdog_armed && self.scope_valid && self.ipv6_ready
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceState {
    phase: ServicePhase,
    safety: SafetyState,
    response_mode: ResponseMode,
}

impl ServiceState {
    pub const fn disabled() -> Self {
        Self {
            phase: ServicePhase::Disabled,
            safety: SafetyState {
                redirect_present: false,
                engine_ready: false,
                watchdog_armed: false,
                scope_valid: false,
                ipv6_ready: false,
            },
            response_mode: ResponseMode::ForwardOriginal,
        }
    }

    pub const fn phase(self) -> ServicePhase {
        self.phase
    }

    pub const fn safety(self) -> SafetyState {
        self.safety
    }

    pub const fn response_mode(self) -> ResponseMode {
        self.response_mode
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceEvent {
    BeginEnable { scope_valid: bool, ipv6_ready: bool },
    EngineReady,
    WatchdogArmed,
    RedirectInstalled,
    EngineUnhealthy,
    BeginDisable,
    EngineStopped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionError {
    InvalidSafetyScope,
    SafetyPrerequisiteMissing,
    InvalidTransition,
}

pub fn reduce(
    current: &ServiceState,
    event: ServiceEvent,
) -> Result<ServiceState, TransitionError> {
    let mut next = *current;
    match event {
        ServiceEvent::BeginEnable {
            scope_valid,
            ipv6_ready,
        } => {
            if current.phase != ServicePhase::Disabled {
                return Err(TransitionError::InvalidTransition);
            }
            if !scope_valid || !ipv6_ready {
                return Err(TransitionError::InvalidSafetyScope);
            }
            next.phase = ServicePhase::Starting;
            next.safety.scope_valid = true;
            next.safety.ipv6_ready = true;
        }
        ServiceEvent::EngineReady => {
            if current.phase != ServicePhase::Starting {
                return Err(TransitionError::InvalidTransition);
            }
            next.safety.engine_ready = true;
            next.phase = ServicePhase::ReadyPassThrough;
        }
        ServiceEvent::WatchdogArmed => {
            if !matches!(
                current.phase,
                ServicePhase::Starting | ServicePhase::ReadyPassThrough
            ) {
                return Err(TransitionError::InvalidTransition);
            }
            next.safety.watchdog_armed = true;
            next.phase = ServicePhase::ReadyPassThrough;
        }
        ServiceEvent::RedirectInstalled => {
            if current.phase != ServicePhase::ReadyPassThrough
                || !current.safety.redirect_prerequisites_met()
            {
                return Err(TransitionError::SafetyPrerequisiteMissing);
            }
            next.safety.redirect_present = true;
            next.phase = ServicePhase::Intercepting;
        }
        ServiceEvent::EngineUnhealthy => {
            next.safety.redirect_present = false;
            next.safety.engine_ready = false;
            next.phase = ServicePhase::DegradedPassThrough;
            next.response_mode = ResponseMode::ForwardOriginal;
        }
        ServiceEvent::BeginDisable => {
            if current.phase == ServicePhase::Disabled {
                return Ok(*current);
            }
            next.safety.redirect_present = false;
            next.phase = ServicePhase::Draining;
            next.response_mode = ResponseMode::ForwardOriginal;
        }
        ServiceEvent::EngineStopped => {
            if current.phase != ServicePhase::Draining {
                return Err(TransitionError::InvalidTransition);
            }
            next = ServiceState::disabled();
        }
    }
    Ok(next)
}
