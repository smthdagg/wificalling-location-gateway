//! Bounded control-API response encoder contract.
//!
//! The frozen WLOC service API v1 response envelope carries exactly one of
//! `result` or `error`, echoes the request id, and never leaks device,
//! location, provider, or credential material. Every response must fit inside
//! the transport frame bound.

use serde_json::json;
use wificalling_location_gateway::service::api::{
    encode_error_response, encode_result_response, ApiErrorCode, ResponseEncodeError,
    SERVICE_API_ID,
};
// MAX_CONTROL_FRAME_BYTES is the single-source transport bound re-exported by
// the API module; importing it from there proves the re-export path works.
use wificalling_location_gateway::service::api::MAX_CONTROL_FRAME_BYTES;

#[test]
fn error_codes_map_to_stable_snake_case_wire_codes() {
    assert_eq!(ApiErrorCode::FrameTooLarge.wire_code(), "frame_too_large");
    assert_eq!(
        ApiErrorCode::MalformedRequest.wire_code(),
        "malformed_request"
    );
    assert_eq!(
        ApiErrorCode::IncompatibleVersion.wire_code(),
        "incompatible_version"
    );
    assert_eq!(
        ApiErrorCode::InvalidRequestId.wire_code(),
        "invalid_request_id"
    );
    assert_eq!(ApiErrorCode::UnknownMethod.wire_code(), "unknown_method");
}

#[test]
fn decode_errors_are_never_retryable() {
    for code in [
        ApiErrorCode::FrameTooLarge,
        ApiErrorCode::MalformedRequest,
        ApiErrorCode::IncompatibleVersion,
        ApiErrorCode::InvalidRequestId,
        ApiErrorCode::UnknownMethod,
    ] {
        assert!(!code.retryable(), "{code:?} must not be retryable");
    }
}

#[test]
fn error_response_carries_envelope_without_result_or_payload() {
    let bytes = encode_error_response("req-1", ApiErrorCode::UnknownMethod).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(value["api_version"], SERVICE_API_ID);
    assert_eq!(value["request_id"], "req-1");
    assert_eq!(value["error"]["code"], "unknown_method");
    assert_eq!(value["error"]["component"], "service");
    assert_eq!(value["error"]["retryable"], false);
    assert!(
        value.get("result").is_none(),
        "error response must not carry a result field"
    );
    let text = String::from_utf8(bytes).unwrap();
    assert!(
        !text.contains("device") && !text.contains("location") && !text.contains("provider"),
        "error response must not leak device, location, or provider material"
    );
}

#[test]
fn error_response_echoes_an_empty_request_id_for_unparseable_requests() {
    let bytes = encode_error_response("", ApiErrorCode::MalformedRequest).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["request_id"], "");
    assert_eq!(value["error"]["code"], "malformed_request");
}

#[test]
fn result_response_wraps_payload_and_echoes_request_id() {
    let payload = json!({"service_phase": "disabled"});
    let bytes = encode_result_response("req-2", &payload).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(value["api_version"], SERVICE_API_ID);
    assert_eq!(value["request_id"], "req-2");
    assert_eq!(value["result"]["service_phase"], "disabled");
    assert!(
        value.get("error").is_none(),
        "result response must not carry an error field"
    );
}

#[test]
fn result_response_accepts_an_empty_object_for_control_acknowledgements() {
    let bytes = encode_result_response("req-3", &json!({})).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(value["result"].is_object());
    assert_eq!(value["result"].as_object().unwrap().len(), 0);
}

#[test]
fn oversized_result_is_rejected_before_serialization_exceeds_the_frame_bound() {
    // A result whose envelope would exceed the 16 KiB transport bound must be
    // rejected with a distinct error, not truncated or silently emitted.
    let huge = "x".repeat(MAX_CONTROL_FRAME_BYTES);
    let payload = json!({ "blob": huge });
    let error = encode_result_response("req-4", &payload).unwrap_err();
    assert_eq!(error, ResponseEncodeError::Oversized);
}

#[test]
fn error_response_always_fits_within_the_frame_bound() {
    // Even the longest request id must not push an error envelope past the
    // transport bound, because error responses are the fallback path.
    let request_id = "x".repeat(64);
    let bytes = encode_error_response(&request_id, ApiErrorCode::FrameTooLarge).unwrap();
    assert!(
        bytes.len() <= MAX_CONTROL_FRAME_BYTES,
        "error response of {} bytes exceeds the {} byte bound",
        bytes.len(),
        MAX_CONTROL_FRAME_BYTES
    );
}

#[test]
fn result_at_the_boundary_is_accepted_and_just_fits() {
    // Build a result whose total envelope is exactly at the bound. A result a
    // single byte larger must be rejected.
    let empty = encode_result_response("boundary", &json!({})).unwrap();
    // Everything in the envelope except the `{}` result value is fixed overhead.
    let overhead = empty.len() - 2;
    let slack = MAX_CONTROL_FRAME_BYTES.saturating_sub(overhead);
    // A result `{"v":"<fill>"}` serializes to 8 + fill_len bytes.
    let fill_len = slack.saturating_sub(8);
    let payload = json!({ "v": "x".repeat(fill_len) });
    let bytes = encode_result_response("boundary", &payload).unwrap();
    assert!(
        bytes.len() <= MAX_CONTROL_FRAME_BYTES,
        "boundary result of {} bytes exceeds bound",
        bytes.len()
    );

    let too_big = json!({ "v": "x".repeat(fill_len + 1) });
    assert_eq!(
        encode_result_response("boundary", &too_big).unwrap_err(),
        ResponseEncodeError::Oversized
    );
}
