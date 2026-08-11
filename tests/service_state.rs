use wificalling_location_gateway::service::state::{
    reduce, ResponseMode, ServiceEvent, ServicePhase, ServiceState, TransitionError,
};

fn ready_for_redirect() -> ServiceState {
    let state = reduce(
        &ServiceState::disabled(),
        ServiceEvent::BeginEnable {
            scope_valid: true,
            ipv6_ready: true,
        },
    )
    .expect("valid enable must start");
    let state = reduce(&state, ServiceEvent::EngineReady).expect("engine becomes ready");
    reduce(&state, ServiceEvent::WatchdogArmed).expect("watchdog becomes ready")
}

#[test]
fn redirect_can_only_be_installed_after_every_safety_prerequisite() {
    let starting = reduce(
        &ServiceState::disabled(),
        ServiceEvent::BeginEnable {
            scope_valid: true,
            ipv6_ready: true,
        },
    )
    .expect("valid enable must start");

    assert_eq!(
        reduce(&starting, ServiceEvent::RedirectInstalled),
        Err(TransitionError::SafetyPrerequisiteMissing)
    );

    let ready = ready_for_redirect();
    let intercepting =
        reduce(&ready, ServiceEvent::RedirectInstalled).expect("safe redirect may install");
    assert_eq!(intercepting.phase(), ServicePhase::Intercepting);
    assert!(intercepting.safety().redirect_present());
    assert!(intercepting.safety().engine_ready());
    assert!(intercepting.safety().watchdog_armed());
    assert!(intercepting.safety().scope_valid());
    assert!(intercepting.safety().ipv6_ready());
    assert_eq!(intercepting.response_mode(), ResponseMode::ForwardOriginal);
}

#[test]
fn invalid_scope_or_ipv6_policy_never_reaches_a_redirectable_state() {
    for (scope_valid, ipv6_ready) in [(false, true), (true, false), (false, false)] {
        assert_eq!(
            reduce(
                &ServiceState::disabled(),
                ServiceEvent::BeginEnable {
                    scope_valid,
                    ipv6_ready,
                },
            ),
            Err(TransitionError::InvalidSafetyScope)
        );
    }
}

#[test]
fn health_failure_withdraws_redirect_and_keeps_patch_mode_disabled() {
    let intercepting = reduce(&ready_for_redirect(), ServiceEvent::RedirectInstalled)
        .expect("safe redirect may install");
    let degraded = reduce(&intercepting, ServiceEvent::EngineUnhealthy)
        .expect("health failure must be handled");

    assert_eq!(degraded.phase(), ServicePhase::DegradedPassThrough);
    assert!(!degraded.safety().redirect_present());
    assert!(!degraded.safety().engine_ready());
    assert_eq!(degraded.response_mode(), ResponseMode::ForwardOriginal);
}

#[test]
fn disable_withdraws_redirect_before_the_engine_stops_and_is_idempotent() {
    let intercepting = reduce(&ready_for_redirect(), ServiceEvent::RedirectInstalled)
        .expect("safe redirect may install");
    let draining =
        reduce(&intercepting, ServiceEvent::BeginDisable).expect("disable must start draining");
    assert_eq!(draining.phase(), ServicePhase::Draining);
    assert!(!draining.safety().redirect_present());

    let disabled = reduce(&draining, ServiceEvent::EngineStopped).expect("engine may stop");
    assert_eq!(disabled, ServiceState::disabled());
    assert_eq!(
        reduce(&disabled, ServiceEvent::BeginDisable).expect("disable is idempotent"),
        disabled
    );
}

#[test]
fn lifecycle_events_reject_out_of_order_or_spurious_health_changes() {
    let starting = reduce(
        &ServiceState::disabled(),
        ServiceEvent::BeginEnable {
            scope_valid: true,
            ipv6_ready: true,
        },
    )
    .expect("valid enable must start");

    assert_eq!(
        reduce(&starting, ServiceEvent::WatchdogArmed),
        Err(TransitionError::InvalidTransition)
    );
    assert_eq!(
        reduce(&ServiceState::disabled(), ServiceEvent::EngineUnhealthy),
        Err(TransitionError::InvalidTransition)
    );
}
