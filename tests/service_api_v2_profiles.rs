use wificalling_location_gateway::service::api::{
    decode_v2_profile_request, encode_v2_error_response, encode_v2_result_response, ApiV2ErrorCode,
    ProfileApiMethod, MAX_CONTROL_FRAME_BYTES, SERVICE_API_V2_ID,
};

#[test]
fn profile_methods_decode_only_on_the_v2_contract() {
    for (method, expected) in [
        ("profile.list", ProfileApiMethod::List),
        ("profile.get", ProfileApiMethod::Get),
        ("profile.create", ProfileApiMethod::Create),
        ("profile.update", ProfileApiMethod::Update),
        ("profile.delete", ProfileApiMethod::Delete),
    ] {
        let params = if method == "profile.create" {
            r#"{"profile_id":"phone","label":"Phone","assigned_device":"192.168.1.10","node_ref":"node-a","node_mode":"fixed","geo_source":"auto","enabled":true}"#
        } else {
            r#"{"profile_id":"phone"}"#
        };
        let payload = format!(
            r#"{{"api_version":"{SERVICE_API_V2_ID}","request_id":"req-1","method":"{method}","params":{params}}}"#
        );
        assert_eq!(
            decode_v2_profile_request(payload.as_bytes())
                .unwrap()
                .method(),
            expected
        );
    }
}

#[test]
fn v1_and_unknown_v2_methods_are_not_reinterpreted_as_profile_operations() {
    let v1 = br#"{"api_version":"wloc.service/v1","request_id":"req-1","method":"profile.list","params":{}}"#;
    assert_eq!(
        decode_v2_profile_request(v1).unwrap_err(),
        ApiV2ErrorCode::IncompatibleVersion
    );

    let unknown = br#"{"api_version":"wloc.service/v2","request_id":"req-1","method":"debug.dump","params":{}}"#;
    assert_eq!(
        decode_v2_profile_request(unknown).unwrap_err(),
        ApiV2ErrorCode::UnknownMethod
    );
}

#[test]
fn unsupported_params_and_invalid_profile_ids_fail_before_dispatch() {
    let unknown = br#"{"api_version":"wloc.service/v2","request_id":"req-1","method":"profile.get","params":{"profile_id":"phone","raw":true}}"#;
    assert_eq!(
        decode_v2_profile_request(unknown).unwrap_err(),
        ApiV2ErrorCode::MalformedRequest
    );

    let invalid = br#"{"api_version":"wloc.service/v2","request_id":"req-1","method":"profile.get","params":{"profile_id":"../phone"}}"#;
    assert_eq!(
        decode_v2_profile_request(invalid).unwrap_err(),
        ApiV2ErrorCode::InvalidParams
    );

    let missing = br#"{"api_version":"wloc.service/v2","request_id":"req-1","method":"profile.get","params":{}}"#;
    assert_eq!(
        decode_v2_profile_request(missing).unwrap_err(),
        ApiV2ErrorCode::InvalidParams
    );

    let bad_mode = br#"{"api_version":"wloc.service/v2","request_id":"req-1","method":"profile.update","params":{"profile_id":"phone","node_mode":"random"}}"#;
    assert_eq!(
        decode_v2_profile_request(bad_mode).unwrap_err(),
        ApiV2ErrorCode::InvalidParams
    );

    let bad_address = br#"{"api_version":"wloc.service/v2","request_id":"req-1","method":"profile.update","params":{"profile_id":"phone","assigned_device":"not-an-address"}}"#;
    assert_eq!(
        decode_v2_profile_request(bad_address).unwrap_err(),
        ApiV2ErrorCode::InvalidParams
    );

    let list_with_mutation = br#"{"api_version":"wloc.service/v2","request_id":"req-1","method":"profile.list","params":{"label":"unexpected"}}"#;
    assert_eq!(
        decode_v2_profile_request(list_with_mutation).unwrap_err(),
        ApiV2ErrorCode::InvalidParams
    );

    let incomplete_create = br#"{"api_version":"wloc.service/v2","request_id":"req-1","method":"profile.create","params":{}}"#;
    assert_eq!(
        decode_v2_profile_request(incomplete_create).unwrap_err(),
        ApiV2ErrorCode::InvalidParams
    );
}

#[test]
fn v2_frame_bound_is_enforced() {
    let oversized = vec![b' '; MAX_CONTROL_FRAME_BYTES + 1];
    assert_eq!(
        decode_v2_profile_request(&oversized).unwrap_err(),
        ApiV2ErrorCode::FrameTooLarge
    );
}

#[test]
fn v2_result_uses_the_v2_envelope_and_frame_limit() {
    let body = serde_json::json!({"profiles": []});
    let encoded = encode_v2_result_response("req-1", &body).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(value["api_version"], SERVICE_API_V2_ID);
    assert_eq!(value["request_id"], "req-1");
    assert!(value.get("error").is_none());
}

#[test]
fn v2_error_uses_the_v2_envelope_and_has_no_result() {
    let encoded = encode_v2_error_response("req-2", ApiV2ErrorCode::InvalidParams).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(value["api_version"], SERVICE_API_V2_ID);
    assert_eq!(value["request_id"], "req-2");
    assert_eq!(value["error"]["code"], "invalid_params");
    assert!(value.get("result").is_none());
}
