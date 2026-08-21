//! Integration test: control API over a real Unix-domain socket.
//!
//! Spins up a [`ControlServer`] on a temporary socket, connects a client, and
//! exercises the full read-decode-dispatch-encode-write loop end to end.

#![cfg(unix)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use wificalling_location_gateway::service::api::RequestParams;
use wificalling_location_gateway::service::api::{SERVICE_API_ID, SERVICE_API_V2_ID};
use wificalling_location_gateway::service::dispatch::{
    DispatchError, InMemoryProfileStore, ServiceDispatch,
};
use wificalling_location_gateway::service::server::ControlServer;

struct StubDispatch {
    enable_fails: bool,
}

impl ServiceDispatch for StubDispatch {
    fn status(&mut self) -> Result<Value, DispatchError> {
        Ok(json!({"service_phase": "disabled", "response_mode": "forward_original"}))
    }
    fn enable(&mut self) -> Result<(), DispatchError> {
        if self.enable_fails {
            Err(DispatchError::EngineUnhealthy)
        } else {
            Ok(())
        }
    }
    fn disable(&mut self) -> Result<(), DispatchError> {
        Ok(())
    }
    fn reload(&mut self) -> Result<(), DispatchError> {
        Ok(())
    }
    fn set_manual_location(&mut self, _params: &RequestParams) -> Result<(), DispatchError> {
        Ok(())
    }
    fn clear_manual_location(&mut self) -> Result<(), DispatchError> {
        Ok(())
    }

    fn search_location(&mut self, _query: &str) -> Result<Value, DispatchError> {
        Ok(serde_json::json!({ "city": "stub", "latitude": 1.0, "longitude": 2.0 }))
    }
}

fn encoded(payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(4 + payload.len());
    bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

fn request_frame(method: &str, request_id: &str) -> Vec<u8> {
    encoded(
        &serde_json::to_vec(&json!({
            "api_version": SERVICE_API_ID,
            "request_id": request_id,
            "method": method,
            "params": {}
        }))
        .unwrap(),
    )
}

async fn read_response(stream: &mut UnixStream) -> Vec<u8> {
    let mut header = [0u8; 4];
    stream.read_exact(&mut header).await.unwrap();
    let len = u32::from_be_bytes(header) as usize;
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body).await.unwrap();
    body
}

static SOCKET_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_socket_path() -> PathBuf {
    // Parallel tests share a process id and clock resolution, so a monotonic
    // counter is required to guarantee distinct socket paths.
    let counter = SOCKET_COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir();
    let mut path = dir;
    path.push(format!("wloc-test-{}-{}.sock", std::process::id(), counter));
    path
}

async fn spawn_server(handler: StubDispatch, path: PathBuf) -> tokio::task::JoinHandle<()> {
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    let server = ControlServer::new(handler);
    tokio::spawn(async move {
        server
            .serve(listener, std::time::Duration::from_secs(3600))
            .await;
    })
}

async fn spawn_profile_server(handler: StubDispatch, path: PathBuf) -> tokio::task::JoinHandle<()> {
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    let server = ControlServer::with_profile_dispatch(handler, InMemoryProfileStore::new());
    tokio::spawn(async move {
        server
            .serve(listener, std::time::Duration::from_secs(3600))
            .await;
    })
}

fn v2_request_frame(method: &str, request_id: &str, params: Value) -> Vec<u8> {
    encoded(
        &serde_json::to_vec(&json!({
            "api_version": SERVICE_API_V2_ID,
            "request_id": request_id,
            "method": method,
            "params": params
        }))
        .unwrap(),
    )
}

#[tokio::test]
async fn status_get_round_trips_over_a_real_socket() {
    let path = temp_socket_path();
    let task = spawn_server(
        StubDispatch {
            enable_fails: false,
        },
        path.clone(),
    )
    .await;

    let mut client = UnixStream::connect(&path).await.unwrap();
    client
        .write_all(&request_frame("status.get", "req-1"))
        .await
        .unwrap();

    let body = read_response(&mut client).await;
    let value: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["api_version"], SERVICE_API_ID);
    assert_eq!(value["request_id"], "req-1");
    assert_eq!(value["result"]["service_phase"], "disabled");
    assert!(value.get("error").is_none());

    task.abort();
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn control_enable_success_returns_empty_result() {
    let path = temp_socket_path();
    let task = spawn_server(
        StubDispatch {
            enable_fails: false,
        },
        path.clone(),
    )
    .await;

    let mut client = UnixStream::connect(&path).await.unwrap();
    client
        .write_all(&request_frame("control.enable", "en-1"))
        .await
        .unwrap();

    let body = read_response(&mut client).await;
    let value: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["request_id"], "en-1");
    assert!(value["result"].is_object());
    assert_eq!(value["result"].as_object().unwrap().len(), 0);

    task.abort();
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn handler_failure_returns_error_envelope_over_socket() {
    let path = temp_socket_path();
    let task = spawn_server(StubDispatch { enable_fails: true }, path.clone()).await;

    let mut client = UnixStream::connect(&path).await.unwrap();
    client
        .write_all(&request_frame("control.enable", "en-2"))
        .await
        .unwrap();

    let body = read_response(&mut client).await;
    let value: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["request_id"], "en-2");
    assert_eq!(value["error"]["code"], "engine_unhealthy");
    assert_eq!(value["error"]["component"], "engine");
    assert_eq!(value["error"]["retryable"], true);
    assert!(value.get("result").is_none());

    task.abort();
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn malformed_request_returns_error_and_connection_closes() {
    let path = temp_socket_path();
    let task = spawn_server(
        StubDispatch {
            enable_fails: false,
        },
        path.clone(),
    )
    .await;

    let mut client = UnixStream::connect(&path).await.unwrap();
    // Send garbage that is not valid JSON.
    client.write_all(&encoded(b"not-json")).await.unwrap();

    let body = read_response(&mut client).await;
    let value: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["error"]["code"], "malformed_request");

    // The server should still be alive for a new connection.
    let mut second = UnixStream::connect(&path).await.unwrap();
    second
        .write_all(&request_frame("status.get", "req-2"))
        .await
        .unwrap();
    let body2 = read_response(&mut second).await;
    let value2: Value = serde_json::from_slice(&body2).unwrap();
    assert_eq!(value2["result"]["service_phase"], "disabled");

    task.abort();
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn multiple_requests_on_one_connection_succeed() {
    let path = temp_socket_path();
    let task = spawn_server(
        StubDispatch {
            enable_fails: false,
        },
        path.clone(),
    )
    .await;

    let mut client = UnixStream::connect(&path).await.unwrap();

    client
        .write_all(&request_frame("status.get", "a"))
        .await
        .unwrap();
    let body = read_response(&mut client).await;
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap()["request_id"],
        "a"
    );

    client
        .write_all(&request_frame("control.disable", "b"))
        .await
        .unwrap();
    let body = read_response(&mut client).await;
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap()["request_id"],
        "b"
    );

    client
        .write_all(&request_frame("status.get", "c"))
        .await
        .unwrap();
    let body = read_response(&mut client).await;
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap()["request_id"],
        "c"
    );

    task.abort();
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn v2_profile_crud_round_trips_through_control_server() {
    let path = temp_socket_path();
    let task = spawn_profile_server(
        StubDispatch {
            enable_fails: false,
        },
        path.clone(),
    )
    .await;
    let mut client = UnixStream::connect(&path).await.unwrap();

    client
        .write_all(&v2_request_frame(
            "profile.create",
            "p-create",
            json!({
                "profile_id": "phone",
                "label": "Phone",
                "assigned_device": "192.168.1.10",
                "node_ref": "node-a",
                "node_mode": "fixed",
                "geo_source": "auto",
                "enabled": true
            }),
        ))
        .await
        .unwrap();
    let created: Value = serde_json::from_slice(&read_response(&mut client).await).unwrap();
    assert_eq!(created["api_version"], SERVICE_API_V2_ID);
    assert_eq!(created["result"]["profile_id"], "phone");

    client
        .write_all(&v2_request_frame("profile.list", "p-list", json!({})))
        .await
        .unwrap();
    let listed: Value = serde_json::from_slice(&read_response(&mut client).await).unwrap();
    assert_eq!(listed["result"]["profiles"][0]["profile_id"], "phone");
    assert!(listed["result"]["profiles"][0].get("assigned_device").is_none());

    client
        .write_all(&v2_request_frame(
            "profile.update",
            "p-update",
            json!({"profile_id": "phone", "label": "Updated", "enabled": false}),
        ))
        .await
        .unwrap();
    let updated: Value = serde_json::from_slice(&read_response(&mut client).await).unwrap();
    assert_eq!(updated["result"]["profile_id"], "phone");

    client
        .write_all(&v2_request_frame(
            "profile.get",
            "p-get",
            json!({"profile_id": "phone"}),
        ))
        .await
        .unwrap();
    let fetched: Value = serde_json::from_slice(&read_response(&mut client).await).unwrap();
    assert_eq!(fetched["result"]["profile"]["label"], "Updated");
    assert!(fetched["result"]["profile"].get("node_ref").is_none());

    client
        .write_all(&v2_request_frame(
            "profile.delete",
            "p-delete",
            json!({"profile_id": "phone"}),
        ))
        .await
        .unwrap();
    let deleted: Value = serde_json::from_slice(&read_response(&mut client).await).unwrap();
    assert_eq!(deleted["result"]["profile_id"], "phone");

    task.abort();
    let _ = std::fs::remove_file(&path);
}
