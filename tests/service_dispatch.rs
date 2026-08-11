//! Control-API request dispatch contract.
//!
//! The dispatcher routes a decoded request to the matching service handler and
//! returns a fully encoded, bounded response frame. Handler failures map to
//! stable error envelopes; success wraps the result payload.

use serde_json::{json, Value};
use wificalling_location_gateway::service::api::{decode_request, SERVICE_API_ID};
use wificalling_location_gateway::service::dispatch::{dispatch, DispatchError, ServiceDispatch};

struct RecordedDispatch {
    status_result: Result<Value, DispatchError>,
    enable_result: Result<(), DispatchError>,
    disable_result: Result<(), DispatchError>,
    reload_result: Result<(), DispatchError>,
    calls: Vec<&'static str>,
}

impl RecordedDispatch {
    fn ok_status() -> Self {
        Self {
            status_result: Ok(json!({"service_phase": "disabled"})),
            enable_result: Ok(()),
            disable_result: Ok(()),
            reload_result: Ok(()),
            calls: Vec::new(),
        }
    }
}

impl ServiceDispatch for RecordedDispatch {
    fn status(&mut self) -> Result<Value, DispatchError> {
        self.calls.push("status");
        self.status_result.clone()
    }
    fn enable(&mut self) -> Result<(), DispatchError> {
        self.calls.push("enable");
        self.enable_result
    }
    fn disable(&mut self) -> Result<(), DispatchError> {
        self.calls.push("disable");
        self.disable_result
    }
    fn reload(&mut self) -> Result<(), DispatchError> {
        self.calls.push("reload");
        self.reload_result
    }
}

fn decoded(method: &str) -> wificalling_location_gateway::service::api::ApiRequest {
    let frame = serde_json::to_vec(&json!({
        "api_version": SERVICE_API_ID,
        "request_id": "req-1",
        "method": method,
        "params": {}
    }))
    .unwrap();
    decode_request(&frame).unwrap()
}

fn parse(frame: &[u8]) -> Value {
    serde_json::from_slice(frame).unwrap()
}

#[test]
fn status_get_routes_to_status_handler_and_wraps_the_result() {
    let mut service = RecordedDispatch::ok_status();
    let response = dispatch(&decoded("status.get"), &mut service).unwrap();

    assert_eq!(service.calls, vec!["status"]);
    let value = parse(&response);
    assert_eq!(value["api_version"], SERVICE_API_ID);
    assert_eq!(value["request_id"], "req-1");
    assert_eq!(value["result"]["service_phase"], "disabled");
    assert!(value.get("error").is_none());
}

#[test]
fn control_methods_route_to_their_handlers_with_an_empty_result() {
    for (method, expected_call) in [
        ("control.enable", "enable"),
        ("control.disable", "disable"),
        ("control.reload", "reload"),
    ] {
        let mut service = RecordedDispatch::ok_status();
        let response = dispatch(&decoded(method), &mut service).unwrap();

        assert_eq!(service.calls, vec![expected_call]);
        let value = parse(&response);
        assert_eq!(value["request_id"], "req-1");
        assert!(value["result"].is_object());
        assert_eq!(value["result"].as_object().unwrap().len(), 0);
        assert!(value.get("error").is_none());
    }
}

#[test]
fn handler_errors_map_to_stable_envelopes_with_component_and_retryable() {
    let cases = [
        (
            DispatchError::InvalidConfig,
            "invalid_config",
            "service",
            false,
        ),
        (
            DispatchError::EngineUnhealthy,
            "engine_unhealthy",
            "engine",
            true,
        ),
        (
            DispatchError::RedirectPresent,
            "redirect_present",
            "network",
            false,
        ),
        (
            DispatchError::CleanupUnsafe,
            "cleanup_unsafe",
            "service",
            false,
        ),
        (
            DispatchError::RuntimeFailure,
            "runtime_failure",
            "engine",
            true,
        ),
        (DispatchError::Unavailable, "unavailable", "service", true),
    ];

    for (error, code, component, retryable) in cases {
        let mut service = RecordedDispatch::ok_status();
        service.enable_result = Err(error);
        let response = dispatch(&decoded("control.enable"), &mut service).unwrap();

        let value = parse(&response);
        assert_eq!(value["error"]["code"], code, "wrong code for {error:?}");
        assert_eq!(
            value["error"]["component"], component,
            "wrong component for {error:?}"
        );
        assert_eq!(
            value["error"]["retryable"], retryable,
            "wrong retryable for {error:?}"
        );
        assert!(
            value.get("result").is_none(),
            "error response must not carry result"
        );
    }
}

#[test]
fn request_id_is_echoed_in_both_success_and_error_responses() {
    let frame = serde_json::to_vec(&json!({
        "api_version": SERVICE_API_ID,
        "request_id": "trace-42",
        "method": "control.enable",
        "params": {}
    }))
    .unwrap();
    let request = decode_request(&frame).unwrap();

    let mut ok_service = RecordedDispatch::ok_status();
    let ok_response = parse(&dispatch(&request, &mut ok_service).unwrap());
    assert_eq!(ok_response["request_id"], "trace-42");

    let mut err_service = RecordedDispatch::ok_status();
    err_service.enable_result = Err(DispatchError::EngineUnhealthy);
    let err_response = parse(&dispatch(&request, &mut err_service).unwrap());
    assert_eq!(err_response["request_id"], "trace-42");
}

#[test]
fn status_handler_error_returns_an_error_envelope_not_a_result() {
    let mut service = RecordedDispatch::ok_status();
    service.status_result = Err(DispatchError::Unavailable);
    let response = dispatch(&decoded("status.get"), &mut service).unwrap();

    let value = parse(&response);
    assert_eq!(value["error"]["code"], "unavailable");
    assert!(value.get("result").is_none());
}
