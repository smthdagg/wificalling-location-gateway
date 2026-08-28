use wificalling_location_gateway::service::control::{
    disable, enable, ControlError, RuntimeControl, RuntimeFailure, RuntimeStep,
};

#[derive(Default)]
struct FakeRuntime {
    operations: Vec<RuntimeStep>,
    fail_at: Vec<RuntimeStep>,
    healthy: bool,
    redirect_present: bool,
    install_side_effect_before_failure: bool,
}

impl FakeRuntime {
    fn record(&mut self, step: RuntimeStep) -> Result<(), RuntimeFailure> {
        self.operations.push(step);
        if self.fail_at.contains(&step) {
            return Err(RuntimeFailure);
        }
        Ok(())
    }
}

impl RuntimeControl for FakeRuntime {
    fn start_engine_passthrough(&mut self) -> Result<(), RuntimeFailure> {
        self.record(RuntimeStep::StartEngine)
    }

    fn engine_healthy(&mut self) -> Result<bool, RuntimeFailure> {
        self.record(RuntimeStep::CheckHealth)?;
        Ok(self.healthy)
    }

    fn arm_watchdog(&mut self) -> Result<(), RuntimeFailure> {
        self.record(RuntimeStep::ArmWatchdog)
    }

    fn install_exact_redirect(&mut self) -> Result<(), RuntimeFailure> {
        if self.install_side_effect_before_failure {
            self.redirect_present = true;
        }
        self.record(RuntimeStep::InstallRedirect)?;
        self.redirect_present = true;
        Ok(())
    }

    fn remove_redirect(&mut self) -> Result<(), RuntimeFailure> {
        self.record(RuntimeStep::RemoveRedirect)?;
        self.redirect_present = false;
        Ok(())
    }

    fn disarm_watchdog(&mut self) -> Result<(), RuntimeFailure> {
        self.record(RuntimeStep::DisarmWatchdog)
    }

    fn redirect_present(&mut self) -> Result<bool, RuntimeFailure> {
        self.record(RuntimeStep::VerifyRedirectAbsent)?;
        Ok(self.redirect_present)
    }

    fn drain_engine(&mut self) -> Result<(), RuntimeFailure> {
        self.record(RuntimeStep::DrainEngine)
    }

    fn stop_engine(&mut self) -> Result<(), RuntimeFailure> {
        self.record(RuntimeStep::StopEngine)
    }
}

#[test]
fn enable_installs_redirect_only_after_engine_health_and_watchdog() {
    let mut runtime = FakeRuntime {
        healthy: true,
        ..FakeRuntime::default()
    };

    enable(&mut runtime, true, true).expect("safe enable must succeed");
    assert_eq!(
        runtime.operations,
        [
            RuntimeStep::StartEngine,
            RuntimeStep::CheckHealth,
            RuntimeStep::ArmWatchdog,
            RuntimeStep::InstallRedirect,
        ]
    );
    assert!(runtime.redirect_present);
}

#[test]
fn invalid_scope_performs_no_runtime_operation() {
    for (scope_valid, ipv6_ready) in [(false, true), (false, false)] {
        let mut runtime = FakeRuntime::default();
        assert_eq!(
            enable(&mut runtime, scope_valid, ipv6_ready),
            Err(ControlError::InvalidSafetyScope)
        );
        assert!(runtime.operations.is_empty());
    }
}

#[test]
fn disabled_ipv6_does_not_block_ipv4_interception() {
    let mut runtime = FakeRuntime {
        healthy: true,
        ..FakeRuntime::default()
    };
    assert!(enable(&mut runtime, true, false).is_ok());
}

#[test]
fn every_post_start_failure_compensates_by_removing_redirect_then_stopping() {
    for failed_step in [
        RuntimeStep::CheckHealth,
        RuntimeStep::ArmWatchdog,
        RuntimeStep::InstallRedirect,
    ] {
        let mut runtime = FakeRuntime {
            fail_at: vec![failed_step],
            healthy: true,
            ..FakeRuntime::default()
        };

        assert!(enable(&mut runtime, true, true).is_err());
        assert!(runtime.operations.ends_with(&[
            RuntimeStep::RemoveRedirect,
            RuntimeStep::VerifyRedirectAbsent,
            RuntimeStep::DisarmWatchdog,
            RuntimeStep::StopEngine,
        ]));
        assert!(!runtime.redirect_present);
    }

    let mut unhealthy = FakeRuntime::default();
    assert_eq!(
        enable(&mut unhealthy, true, true),
        Err(ControlError::EngineUnhealthy)
    );
    assert!(unhealthy.operations.ends_with(&[
        RuntimeStep::RemoveRedirect,
        RuntimeStep::VerifyRedirectAbsent,
        RuntimeStep::DisarmWatchdog,
        RuntimeStep::StopEngine,
    ]));
}

#[test]
fn failed_cleanup_keeps_watchdog_and_engine_alive_in_passthrough() {
    for cleanup_failure in [
        RuntimeStep::RemoveRedirect,
        RuntimeStep::VerifyRedirectAbsent,
    ] {
        let mut runtime = FakeRuntime {
            fail_at: vec![RuntimeStep::InstallRedirect, cleanup_failure],
            healthy: true,
            install_side_effect_before_failure: true,
            ..FakeRuntime::default()
        };

        assert_eq!(
            enable(&mut runtime, true, true),
            Err(ControlError::CleanupUnsafe)
        );
        assert!(!runtime.operations.contains(&RuntimeStep::DisarmWatchdog));
        assert!(!runtime.operations.contains(&RuntimeStep::StopEngine));
    }
}

#[test]
fn disable_removes_and_verifies_redirect_before_drain_and_stop() {
    let mut runtime = FakeRuntime {
        healthy: true,
        redirect_present: true,
        ..FakeRuntime::default()
    };

    disable(&mut runtime).expect("disable must succeed");
    assert_eq!(
        runtime.operations,
        [
            RuntimeStep::RemoveRedirect,
            RuntimeStep::VerifyRedirectAbsent,
            RuntimeStep::DisarmWatchdog,
            RuntimeStep::DrainEngine,
            RuntimeStep::StopEngine,
        ]
    );
    assert!(!runtime.redirect_present);
}
