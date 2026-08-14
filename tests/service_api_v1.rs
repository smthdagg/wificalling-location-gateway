use wificalling_location_gateway::service::api::{
    decode_request, ApiErrorCode, ApiMethod, MAX_CONTROL_FRAME_BYTES, SERVICE_API_ID,
};

#[test]
fn status_request_decodes_with_the_frozen_v1_contract() {
    let request = decode_request(
        br#"{"api_version":"wloc.service/v1","request_id":"req-1","method":"status.get","params":{}}"#,
    )
    .expect("valid status request must decode");

    assert_eq!(request.api_version(), SERVICE_API_ID);
    assert_eq!(request.request_id(), "req-1");
    assert_eq!(request.method(), ApiMethod::StatusGet);
}

#[test]
fn supported_control_methods_are_explicit_and_finite() {
    for (wire_method, expected) in [
        ("control.enable", ApiMethod::ControlEnable),
        ("control.disable", ApiMethod::ControlDisable),
        ("control.reload", ApiMethod::ControlReload),
        ("control.refresh", ApiMethod::Refresh),
    ] {
        let payload = format!(
            r#"{{"api_version":"wloc.service/v1","request_id":"req-2","method":"{wire_method}","params":{{}}}}"#
        );
        assert_eq!(
            decode_request(payload.as_bytes())
                .expect("supported control method must decode")
                .method(),
            expected
        );
    }
}

#[test]
fn incompatible_versions_unknown_methods_and_unknown_fields_fail_closed() {
    for (payload, expected) in [
        (
            br#"{"api_version":"wloc.service/v2","request_id":"req-1","method":"status.get","params":{}}"#.as_slice(),
            ApiErrorCode::IncompatibleVersion,
        ),
        (
            br#"{"api_version":"wloc.service/v1","request_id":"req-1","method":"debug.dump","params":{}}"#.as_slice(),
            ApiErrorCode::UnknownMethod,
        ),
        (
            br#"{"api_version":"wloc.service/v1","request_id":"req-1","method":"status.get","params":{"raw":true}}"#.as_slice(),
            ApiErrorCode::MalformedRequest,
        ),
        (
            br#"{"api_version":"wloc.service/v1","request_id":"req-1","method":"status.get","params":{},"extra":1}"#.as_slice(),
            ApiErrorCode::MalformedRequest,
        ),
    ] {
        assert_eq!(decode_request(payload).unwrap_err(), expected);
    }
}

#[test]
fn frame_and_request_id_limits_are_enforced_before_dispatch() {
    let oversized = vec![b' '; MAX_CONTROL_FRAME_BYTES + 1];
    assert_eq!(
        decode_request(&oversized).unwrap_err(),
        ApiErrorCode::FrameTooLarge
    );

    let long_id = "a".repeat(65);
    let payload = format!(
        r#"{{"api_version":"wloc.service/v1","request_id":"{long_id}","method":"status.get","params":{{}}}}"#
    );
    assert_eq!(
        decode_request(payload.as_bytes()).unwrap_err(),
        ApiErrorCode::InvalidRequestId
    );
}
