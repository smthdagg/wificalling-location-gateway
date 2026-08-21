use std::time::Duration;

use wificalling_location_gateway::service::control::{RuntimeControl, RuntimeFailure, RuntimeStep};
use wificalling_location_gateway::service::supervisor::{
    reduce, RestartBudget, RestartDecision, SupervisorEvent, SupervisorLimits, SupervisorPhase,
    SupervisorState, SupervisorTransitionError, MANAGED_CHILDREN, MAX_MANAGED_CHILDREN,
};

fn limits() -> SupervisorLimits {
    SupervisorLimits {
        max_children: MAX_MANAGED_CHILDREN,
        max_restart_attempts: 3,
        restart_window: Duration::from_secs(300),
        initial_restart_backoff: Duration::from_secs(5),
        max_restart_backoff: Duration::from_secs(30),
        health_poll_interval: Duration::from_secs(10),
    }
}

fn ready_for_redirect() -> SupervisorState {
    let limits = limits();
    let state = reduce(
        &SupervisorState::stopped(),
        SupervisorEvent::StartRequested {
            scope_valid: true,
            ipv6_ready: true,
            child_count: MANAGED_CHILDREN,
        },
        limits,
    )
    .unwrap();
    let state = reduce(&state, SupervisorEvent::ChildrenReady, limits).unwrap();
    reduce(&state, SupervisorEvent::WatchdogArmed, limits).unwrap()
}

#[test]
fn redirect_is_last_and_stop_withdraws_before_children() {
    let limits = limits();
    let ready = ready_for_redirect();
    let active = reduce(&ready, SupervisorEvent::RedirectInstalled, limits).unwrap();
    assert_eq!(active.phase(), SupervisorPhase::Intercepting);
    assert!(active.redirect_present());

    let draining = reduce(&active, SupervisorEvent::StopRequested, limits).unwrap();
    assert_eq!(draining.phase(), SupervisorPhase::Draining);
    assert!(!draining.redirect_present());
    assert!(!draining.watchdog_armed());

    let stopped = reduce(&draining, SupervisorEvent::ChildrenStopped, limits).unwrap();
    assert_eq!(stopped, SupervisorState::stopped());
}

#[test]
fn invalid_scope_and_child_count_never_start() {
    let limits = limits();
    for (scope_valid, ipv6_ready) in [(false, true), (true, false), (false, false)] {
        assert_eq!(
            reduce(
                &SupervisorState::stopped(),
                SupervisorEvent::StartRequested {
                    scope_valid,
                    ipv6_ready,
                    child_count: MANAGED_CHILDREN,
                },
                limits,
            ),
            Err(SupervisorTransitionError::InvalidSafetyScope)
        );
    }
    assert_eq!(
        reduce(
            &SupervisorState::stopped(),
            SupervisorEvent::StartRequested {
                scope_valid: true,
                ipv6_ready: true,
                child_count: MAX_MANAGED_CHILDREN + 1,
            },
            limits,
        ),
        Err(SupervisorTransitionError::TooManyChildren)
    );
}

#[test]
fn health_failure_withdraws_redirect_and_enters_passthrough() {
    let limits = limits();
    let active = reduce(
        &ready_for_redirect(),
        SupervisorEvent::RedirectInstalled,
        limits,
    )
    .unwrap();
    let degraded = reduce(&active, SupervisorEvent::HealthFailed, limits).unwrap();
    assert_eq!(degraded.phase(), SupervisorPhase::DegradedPassthrough);
    assert!(!degraded.redirect_present());
    assert!(!degraded.watchdog_armed());
    assert_eq!(degraded.child_count(), MANAGED_CHILDREN);
}

#[test]
fn restart_budget_is_bounded_and_backed_off() {
    let limits = limits();
    let mut budget = RestartBudget::new();
    assert_eq!(
        budget.decide(100, limits),
        RestartDecision::Allowed {
            delay: Duration::ZERO
        }
    );
    budget.record_attempt(100, limits);
    assert_eq!(
        budget.decide(100, limits),
        RestartDecision::Backoff { retry_at_unix: 105 }
    );
    assert_eq!(
        budget.decide(105, limits),
        RestartDecision::Allowed {
            delay: Duration::from_secs(10)
        }
    );
    budget.record_attempt(105, limits);
    budget.record_attempt(115, limits);
    assert_eq!(budget.decide(120, limits), RestartDecision::Exhausted);
    assert_eq!(
        budget.decide(401, limits),
        RestartDecision::Allowed {
            delay: Duration::ZERO
        }
    );
}

#[test]
fn limits_reject_unbounded_runtime_configuration() {
    let mut invalid = limits();
    invalid.max_children = MAX_MANAGED_CHILDREN + 1;
    assert!(invalid.validate().is_err());
    invalid = limits();
    invalid.health_poll_interval = Duration::from_secs(61);
    assert!(invalid.validate().is_err());
}

#[derive(Default)]
struct FakeRuntime {
    operations: Vec<RuntimeStep>,
    healthy: bool,
    redirect_present: bool,
}

impl RuntimeControl for FakeRuntime {
    fn start_engine_passthrough(&mut self) -> Result<(), RuntimeFailure> {
        self.operations.push(RuntimeStep::StartEngine);
        Ok(())
    }
    fn engine_healthy(&mut self) -> Result<bool, RuntimeFailure> {
        self.operations.push(RuntimeStep::CheckHealth);
        Ok(self.healthy)
    }
    fn arm_watchdog(&mut self) -> Result<(), RuntimeFailure> {
        self.operations.push(RuntimeStep::ArmWatchdog);
        Ok(())
    }
    fn install_exact_redirect(&mut self) -> Result<(), RuntimeFailure> {
        self.operations.push(RuntimeStep::InstallRedirect);
        self.redirect_present = true;
        Ok(())
    }
    fn remove_redirect(&mut self) -> Result<(), RuntimeFailure> {
        self.operations.push(RuntimeStep::RemoveRedirect);
        self.redirect_present = false;
        Ok(())
    }
    fn redirect_present(&mut self) -> Result<bool, RuntimeFailure> {
        self.operations.push(RuntimeStep::VerifyRedirectAbsent);
        Ok(self.redirect_present)
    }
    fn disarm_watchdog(&mut self) -> Result<(), RuntimeFailure> {
        self.operations.push(RuntimeStep::DisarmWatchdog);
        Ok(())
    }
    fn drain_engine(&mut self) -> Result<(), RuntimeFailure> {
        self.operations.push(RuntimeStep::DrainEngine);
        Ok(())
    }
    fn stop_engine(&mut self) -> Result<(), RuntimeFailure> {
        self.operations.push(RuntimeStep::StopEngine);
        Ok(())
    }
}

#[test]
fn supervisor_adapter_owns_control_ordering() {
    let runtime = FakeRuntime {
        healthy: true,
        ..FakeRuntime::default()
    };
    let mut supervisor =
        wificalling_location_gateway::service::supervisor::UnifiedSupervisor::new(runtime);
    supervisor.enable(true, true).unwrap();
    assert_eq!(supervisor.state().phase(), SupervisorPhase::Intercepting);
    supervisor.disable().unwrap();
    assert_eq!(supervisor.state().phase(), SupervisorPhase::Stopped);
}

#[test]
fn supervisor_maps_invalid_control_to_a_safe_stopped_state() {
    let runtime = FakeRuntime::default();
    let mut supervisor =
        wificalling_location_gateway::service::supervisor::UnifiedSupervisor::new(runtime);
    assert!(supervisor.enable(false, true).is_err());
    assert_eq!(supervisor.state(), SupervisorState::stopped());
}
