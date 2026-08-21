//! Unified Gateway/WLOC lifecycle ownership.
//!
//! The supervisor is deliberately independent of procd and shell commands.
//! It owns the ordering contract used by both the Rust control plane and the
//! OpenWrt entry point: children start in passthrough, health is observed,
//! the watchdog is armed, and only then may the component-owned redirect be
//! installed. Failure and stop paths withdraw the redirect before children
//! are drained or stopped.

use std::time::Duration;

use super::control::{self, ControlError, RuntimeControl};

pub const MANAGED_CHILDREN: u8 = 2; // Gateway/sing-box + WLOC control/proxy
pub const MAX_MANAGED_CHILDREN: u8 = 3; // includes one bounded probe child

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SupervisorLimits {
    pub max_children: u8,
    pub max_restart_attempts: u8,
    pub restart_window: Duration,
    pub initial_restart_backoff: Duration,
    pub max_restart_backoff: Duration,
    pub health_poll_interval: Duration,
}

impl Default for SupervisorLimits {
    fn default() -> Self {
        Self {
            max_children: MAX_MANAGED_CHILDREN,
            max_restart_attempts: 3,
            restart_window: Duration::from_secs(300),
            initial_restart_backoff: Duration::from_secs(5),
            max_restart_backoff: Duration::from_secs(120),
            health_poll_interval: Duration::from_secs(10),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisorConfigError {
    TooManyChildren,
    InvalidRestartBudget,
    InvalidHealthPollInterval,
}

impl SupervisorLimits {
    pub fn validate(self) -> Result<(), SupervisorConfigError> {
        if self.max_children == 0 || self.max_children > MAX_MANAGED_CHILDREN {
            return Err(SupervisorConfigError::TooManyChildren);
        }
        if self.max_restart_attempts == 0
            || self.restart_window.is_zero()
            || self.initial_restart_backoff.is_zero()
            || self.max_restart_backoff < self.initial_restart_backoff
        {
            return Err(SupervisorConfigError::InvalidRestartBudget);
        }
        if self.health_poll_interval.is_zero()
            || self.health_poll_interval > Duration::from_secs(60)
        {
            return Err(SupervisorConfigError::InvalidHealthPollInterval);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisorPhase {
    Stopped,
    StartingPassthrough,
    ReadyPassthrough,
    Intercepting,
    DegradedPassthrough,
    Draining,
    /// Cleanup could not prove that the component-owned redirect is absent.
    /// The state is deliberately conservative so callers never mistake an
    /// uncertain firewall state for a cleanly stopped service.
    CleanupUnsafe,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SupervisorState {
    phase: SupervisorPhase,
    child_count: u8,
    redirect_present: bool,
    watchdog_armed: bool,
    scope_valid: bool,
    ipv6_ready: bool,
}

impl SupervisorState {
    pub const fn stopped() -> Self {
        Self {
            phase: SupervisorPhase::Stopped,
            child_count: 0,
            redirect_present: false,
            watchdog_armed: false,
            scope_valid: false,
            ipv6_ready: false,
        }
    }

    pub const fn phase(self) -> SupervisorPhase {
        self.phase
    }

    pub const fn child_count(self) -> u8 {
        self.child_count
    }

    pub const fn redirect_present(self) -> bool {
        self.redirect_present
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisorEvent {
    StartRequested {
        scope_valid: bool,
        ipv6_ready: bool,
        child_count: u8,
    },
    ChildrenReady,
    WatchdogArmed,
    RedirectInstalled,
    HealthFailed,
    StopRequested,
    ChildrenStopped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisorTransitionError {
    InvalidSafetyScope,
    TooManyChildren,
    SafetyPrerequisiteMissing,
    InvalidTransition,
}

pub fn reduce(
    current: &SupervisorState,
    event: SupervisorEvent,
    limits: SupervisorLimits,
) -> Result<SupervisorState, SupervisorTransitionError> {
    let mut next = *current;
    match event {
        SupervisorEvent::StartRequested {
            scope_valid,
            ipv6_ready,
            child_count,
        } => {
            if current.phase != SupervisorPhase::Stopped {
                return Err(SupervisorTransitionError::InvalidTransition);
            }
            if !scope_valid || !ipv6_ready {
                return Err(SupervisorTransitionError::InvalidSafetyScope);
            }
            if child_count == 0 || child_count > limits.max_children {
                return Err(SupervisorTransitionError::TooManyChildren);
            }
            next.phase = SupervisorPhase::StartingPassthrough;
            next.child_count = child_count;
            next.scope_valid = true;
            next.ipv6_ready = true;
        }
        SupervisorEvent::ChildrenReady => {
            if current.phase != SupervisorPhase::StartingPassthrough {
                return Err(SupervisorTransitionError::InvalidTransition);
            }
            next.phase = SupervisorPhase::ReadyPassthrough;
        }
        SupervisorEvent::WatchdogArmed => {
            if current.phase != SupervisorPhase::ReadyPassthrough {
                return Err(SupervisorTransitionError::InvalidTransition);
            }
            next.watchdog_armed = true;
        }
        SupervisorEvent::RedirectInstalled => {
            if current.phase != SupervisorPhase::ReadyPassthrough
                || !current.watchdog_armed
                || !current.scope_valid
                || !current.ipv6_ready
            {
                return Err(SupervisorTransitionError::SafetyPrerequisiteMissing);
            }
            next.phase = SupervisorPhase::Intercepting;
            next.redirect_present = true;
        }
        SupervisorEvent::HealthFailed => {
            if !matches!(
                current.phase,
                SupervisorPhase::StartingPassthrough
                    | SupervisorPhase::ReadyPassthrough
                    | SupervisorPhase::Intercepting
            ) {
                return Err(SupervisorTransitionError::InvalidTransition);
            }
            next.phase = SupervisorPhase::DegradedPassthrough;
            next.redirect_present = false;
            next.watchdog_armed = false;
        }
        SupervisorEvent::StopRequested => {
            if current.phase == SupervisorPhase::Stopped {
                return Ok(*current);
            }
            next.phase = SupervisorPhase::Draining;
            next.redirect_present = false;
            next.watchdog_armed = false;
        }
        SupervisorEvent::ChildrenStopped => {
            if !matches!(
                current.phase,
                SupervisorPhase::Draining | SupervisorPhase::DegradedPassthrough
            ) {
                return Err(SupervisorTransitionError::InvalidTransition);
            }
            next = SupervisorState::stopped();
        }
    }
    Ok(next)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestartDecision {
    Allowed { delay: Duration },
    Backoff { retry_at_unix: u64 },
    Exhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RestartBudget {
    window_started_at: Option<u64>,
    attempts: u8,
    next_allowed_at: u64,
}

impl RestartBudget {
    pub const fn new() -> Self {
        Self {
            window_started_at: None,
            attempts: 0,
            next_allowed_at: 0,
        }
    }

    pub fn decide(&self, now_unix: u64, limits: SupervisorLimits) -> RestartDecision {
        let Some(window_started_at) = self.window_started_at else {
            return RestartDecision::Allowed {
                delay: Duration::ZERO,
            };
        };
        if now_unix.saturating_sub(window_started_at) >= limits.restart_window.as_secs() {
            return RestartDecision::Allowed {
                delay: Duration::ZERO,
            };
        }
        if self.attempts >= limits.max_restart_attempts {
            return RestartDecision::Exhausted;
        }
        if now_unix < self.next_allowed_at {
            return RestartDecision::Backoff {
                retry_at_unix: self.next_allowed_at,
            };
        }
        RestartDecision::Allowed {
            delay: Duration::from_secs(
                limits
                    .initial_restart_backoff
                    .as_secs()
                    .saturating_mul(1_u64 << self.attempts.min(6)),
            ),
        }
    }

    pub fn record_attempt(&mut self, now_unix: u64, limits: SupervisorLimits) {
        if self
            .window_started_at
            .map(|started| now_unix.saturating_sub(started) >= limits.restart_window.as_secs())
            .unwrap_or(true)
        {
            self.window_started_at = Some(now_unix);
            self.attempts = 0;
        }
        self.attempts = self.attempts.saturating_add(1);
        let backoff = limits
            .initial_restart_backoff
            .as_secs()
            .saturating_mul(1_u64 << self.attempts.saturating_sub(1).min(6));
        self.next_allowed_at =
            now_unix.saturating_add(backoff.min(limits.max_restart_backoff.as_secs()));
    }
}

impl Default for RestartBudget {
    fn default() -> Self {
        Self::new()
    }
}

/// Runtime adapter owned by the unified service boundary.
pub struct UnifiedSupervisor<R: RuntimeControl> {
    runtime: R,
    state: SupervisorState,
    limits: SupervisorLimits,
    restart_budget: RestartBudget,
}

impl<R: RuntimeControl> UnifiedSupervisor<R> {
    pub fn new(runtime: R) -> Self {
        Self::with_limits(runtime, SupervisorLimits::default())
    }

    pub fn with_limits(runtime: R, limits: SupervisorLimits) -> Self {
        assert!(
            limits.validate().is_ok(),
            "invalid unified supervisor limits"
        );
        Self {
            runtime,
            state: SupervisorState::stopped(),
            limits,
            restart_budget: RestartBudget::new(),
        }
    }

    pub fn state(&self) -> SupervisorState {
        self.state
    }

    pub fn limits(&self) -> SupervisorLimits {
        self.limits
    }

    pub fn restart_budget(&self) -> RestartBudget {
        self.restart_budget
    }

    pub fn enable(&mut self, scope_valid: bool, ipv6_ready: bool) -> Result<(), SupervisorError> {
        self.state = reduce(
            &self.state,
            SupervisorEvent::StartRequested {
                scope_valid,
                ipv6_ready,
                child_count: MANAGED_CHILDREN,
            },
            self.limits,
        )
        .map_err(SupervisorError::Transition)?;

        if let Err(error) = control::enable(&mut self.runtime, scope_valid, ipv6_ready) {
            if matches!(error, ControlError::CleanupUnsafe) {
                self.enter_cleanup_unsafe();
            } else {
                self.state = SupervisorState::stopped();
            }
            return Err(SupervisorError::Control(error));
        }
        self.state = reduce(&self.state, SupervisorEvent::ChildrenReady, self.limits)
            .map_err(SupervisorError::Transition)?;
        self.state = reduce(&self.state, SupervisorEvent::WatchdogArmed, self.limits)
            .map_err(SupervisorError::Transition)?;
        self.state = reduce(&self.state, SupervisorEvent::RedirectInstalled, self.limits)
            .map_err(SupervisorError::Transition)?;
        Ok(())
    }

    pub fn disable(&mut self) -> Result<(), SupervisorError> {
        self.state = reduce(&self.state, SupervisorEvent::StopRequested, self.limits)
            .map_err(SupervisorError::Transition)?;
        if let Err(error) = control::disable(&mut self.runtime) {
            self.enter_cleanup_unsafe();
            return Err(SupervisorError::Control(error));
        }
        self.state = reduce(&self.state, SupervisorEvent::ChildrenStopped, self.limits)
            .map_err(SupervisorError::Transition)?;
        Ok(())
    }

    pub fn record_crash(&mut self, now_unix: u64) -> RestartDecision {
        if let Ok(next) = reduce(&self.state, SupervisorEvent::HealthFailed, self.limits) {
            self.state = next;
        }
        let decision = self.restart_budget.decide(now_unix, self.limits);
        if matches!(decision, RestartDecision::Allowed { .. }) {
            self.restart_budget.record_attempt(now_unix, self.limits);
        }
        decision
    }

    fn enter_cleanup_unsafe(&mut self) {
        self.state = SupervisorState {
            phase: SupervisorPhase::CleanupUnsafe,
            child_count: self.state.child_count.max(MANAGED_CHILDREN),
            // A failed cleanup must be treated as possibly still installed.
            redirect_present: true,
            watchdog_armed: true,
            scope_valid: self.state.scope_valid,
            ipv6_ready: self.state.ipv6_ready,
        };
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisorError {
    Transition(SupervisorTransitionError),
    Control(ControlError),
}
