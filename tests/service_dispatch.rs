//! Control-API request dispatch contract.
//!
//! The dispatcher routes a decoded request to the matching service handler and
//! returns a fully encoded, bounded response frame. Handler failures map to
//! stable error envelopes; success wraps the result payload.

use serde_json::{json, Value};
use wificalling_location_gateway::service::api::{
    decode_request, decode_v2_profile_request, RequestParams, SERVICE_API_ID, SERVICE_API_V2_ID,
};
use wificalling_location_gateway::service::dispatch::{
    dispatch, dispatch_v2, DispatchError, InMemoryProfileStore, ServiceDispatch,
};

struct RecordedDispatch {
    status_result: Result<Value, DispatchError>,
    enable_result: Result<(), DispatchError>,
    disable_result: Result<(), DispatchError>,
    reload_result: Result<(), DispatchError>,
    refresh_result: Result<(), DispatchError>,
    calls: Vec<&'static str>,
}

impl RecordedDispatch {
    fn ok_status() -> Self {
        Self {
            status_result: Ok(json!({"service_phase": "disabled"})),
            enable_result: Ok(()),
            disable_result: Ok(()),
            reload_result: Ok(()),
            refresh_result: Ok(()),
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
    fn set_manual_location(&mut self, _params: &RequestParams) -> Result<(), DispatchError> {
        self.calls.push("geo.set");
        Ok(())
    }
    fn clear_manual_location(&mut self) -> Result<(), DispatchError> {
        self.calls.push("geo.clear");
        Ok(())
    }

    fn search_location(&mut self, query: &str) -> Result<Value, DispatchError> {
        self.calls.push("geo.search");
        Ok(serde_json::json!({ "city": query, "latitude": 1.0, "longitude": 2.0 }))
    }

    fn refresh_evidence(&mut self) -> Result<(), DispatchError> {
        self.calls.push("refresh");
        self.refresh_result
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

fn decoded_v2(method: &str, params: Value) -> wificalling_location_gateway::service::api::ApiV2ProfileRequest {
    let frame = serde_json::to_vec(&json!({
        "api_version": SERVICE_API_V2_ID,
        "request_id": "profile-1",
        "method": method,
        "params": params
    }))
    .unwrap();
    decode_v2_profile_request(&frame).unwrap()
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
        ("control.refresh", "refresh"),
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

#[test]
fn geo_set_and_geo_clear_route_to_their_handlers() {
    let mut service = RecordedDispatch::ok_status();
    let frame = serde_json::to_vec(&json!({
        "api_version": SERVICE_API_ID,
        "request_id": "geo-1",
        "method": "geo.set",
        "params": {"query": "London, UK"}
    }))
    .unwrap();
    let request = decode_request(&frame).unwrap();
    let response = dispatch(&request, &mut service).unwrap();
    assert_eq!(service.calls, vec!["geo.set"]);
    assert!(parse(&response).get("result").is_some());

    let frame = serde_json::to_vec(&json!({
        "api_version": SERVICE_API_ID,
        "request_id": "geo-2",
        "method": "geo.clear",
        "params": {}
    }))
    .unwrap();
    let request = decode_request(&frame).unwrap();
    let response = dispatch(&request, &mut service).unwrap();
    assert_eq!(service.calls, vec!["geo.set", "geo.clear"]);
    assert!(parse(&response).get("result").is_some());
}

#[test]
fn geo_search_returns_the_place_without_applying() {
    let mut service = RecordedDispatch::ok_status();
    let frame = serde_json::to_vec(&json!({
        "api_version": SERVICE_API_ID,
        "request_id": "search-1",
        "method": "geo.search",
        "params": {"query": "Tokyo"}
    }))
    .unwrap();
    let request = decode_request(&frame).unwrap();
    let response = dispatch(&request, &mut service).unwrap();
    assert_eq!(service.calls, vec!["geo.search"]);
    let payload = parse(&response);
    assert_eq!(payload["result"]["city"], "Tokyo");
    assert!(payload["result"]["latitude"].is_number());
}

#[test]
fn geo_search_without_query_is_rejected() {
    let mut service = RecordedDispatch::ok_status();
    let frame = serde_json::to_vec(&json!({
        "api_version": SERVICE_API_ID,
        "request_id": "search-2",
        "method": "geo.search",
        "params": {}
    }))
    .unwrap();
    let request = decode_request(&frame).unwrap();
    let response = dispatch(&request, &mut service).unwrap();
    assert!(
        service.calls.is_empty(),
        "handler must not run without a query"
    );
    assert_eq!(parse(&response)["error"]["code"], "invalid_location");
}

#[test]
fn geo_set_with_coordinates_decodes_params() {
    let frame = serde_json::to_vec(&json!({
        "api_version": SERVICE_API_ID,
        "request_id": "geo-3",
        "method": "geo.set",
        "params": {"latitude": 51.5074, "longitude": -0.1278}
    }))
    .unwrap();
    let request = decode_request(&frame).unwrap();
    let params = request.params();
    assert_eq!(params.latitude, Some(51.5074));
    assert_eq!(params.longitude, Some(-0.1278));
    assert_eq!(params.query, None);
}

#[test]
fn geo_set_unknown_params_are_rejected() {
    let frame = serde_json::to_vec(&json!({
        "api_version": SERVICE_API_ID,
        "request_id": "geo-4",
        "method": "geo.set",
        "params": {"unexpected": true}
    }))
    .unwrap();
    assert_eq!(
        decode_request(&frame),
        Err(wificalling_location_gateway::service::api::ApiErrorCode::MalformedRequest)
    );
}

#[test]
fn v2_profile_dispatch_supports_bounded_create_get_update_list_delete() {
    let mut profiles = InMemoryProfileStore::new();
    let create = decoded_v2(
        "profile.create",
        json!({
            "profile_id": "phone",
            "label": "Phone",
            "assigned_device": "192.168.1.10",
            "node_ref": "node-a",
            "node_mode": "fixed",
            "geo_source": "auto",
            "enabled": true
        }),
    );
    let created = parse(&dispatch_v2(&create, &mut profiles).unwrap());
    assert_eq!(created["api_version"], SERVICE_API_V2_ID);
    assert_eq!(created["result"]["profile_id"], "phone");

    let listed = parse(
        &dispatch_v2(
            &decoded_v2("profile.list", json!({})),
            &mut profiles,
        )
        .unwrap(),
    );
    assert_eq!(listed["result"]["profiles"].as_array().unwrap().len(), 1);
    assert_eq!(listed["result"]["profiles"][0]["profile_id"], "phone");
    assert!(listed["result"]["profiles"][0].get("assigned_device").is_none());

    let fetched = parse(
        &dispatch_v2(
            &decoded_v2("profile.get", json!({"profile_id": "phone"})),
            &mut profiles,
        )
        .unwrap(),
    );
    assert_eq!(fetched["result"]["profile"]["label"], "Phone");
    assert!(fetched["result"]["profile"].get("node_ref").is_none());

    let updated = parse(
        &dispatch_v2(
            &decoded_v2(
                "profile.update",
                json!({"profile_id": "phone", "label": "Work phone", "enabled": false}),
            ),
            &mut profiles,
        )
        .unwrap(),
    );
    assert_eq!(updated["result"]["profile_id"], "phone");
    let fetched_after_update = parse(
        &dispatch_v2(
            &decoded_v2("profile.get", json!({"profile_id": "phone"})),
            &mut profiles,
        )
        .unwrap(),
    );
    assert_eq!(fetched_after_update["result"]["profile"]["label"], "Work phone");
    assert_eq!(fetched_after_update["result"]["profile"]["enabled"], false);

    let deleted = parse(
        &dispatch_v2(
            &decoded_v2("profile.delete", json!({"profile_id": "phone"})),
            &mut profiles,
        )
        .unwrap(),
    );
    assert_eq!(deleted["result"]["profile_id"], "phone");
    let listed_empty = parse(
        &dispatch_v2(
            &decoded_v2("profile.list", json!({})),
            &mut profiles,
        )
        .unwrap(),
    );
    assert!(listed_empty["result"]["profiles"].as_array().unwrap().is_empty());
}

#[test]
fn v2_profile_dispatch_maps_unknown_profile_to_v2_error_envelope() {
    let mut profiles = InMemoryProfileStore::new();
    let response = parse(
        &dispatch_v2(
            &decoded_v2("profile.get", json!({"profile_id": "missing"})),
            &mut profiles,
        )
        .unwrap(),
    );
    assert_eq!(response["api_version"], SERVICE_API_V2_ID);
    assert_eq!(response["error"]["code"], "profile_not_found");
    assert!(response.get("result").is_none());
}
