//! wloc-ctl: control CLI for the wloc-service daemon.
//!
//! Bridges LuCI (via ucode) to the root-only Unix-socket control API. Each
//! invocation connects to the daemon socket, sends one framed control request,
//! and prints the JSON response on stdout (exit 0) or an error message (exit 1).

use std::env;
use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let code = run();
    std::process::exit(code);
}

fn run() -> i32 {
    let args: Vec<String> = env::args().skip(1).collect();
    let socket_path =
        env::var("WLOC_SOCKET").unwrap_or_else(|_| "/var/run/wloc-service/control.sock".to_owned());
    run_with_args(&args, &socket_path)
}

fn run_with_args(args: &[String], socket_path: &str) -> i32 {
    if args.is_empty() {
        eprintln!("用法: wloc-ctl <方法> [--query <地名> | --lat <纬度> --lon <经度>]");
        return 2;
    }

    let method = args[0].as_str();
    let wire_method = match map_wire_method(method) {
        Some(wire) => wire,
        None => {
            eprintln!("wloc-ctl: 未知方法 {method}");
            return 2;
        }
    };
    let mut params = match method {
        "status" | "enable" | "disable" | "geo-clear" | "reload" | "refresh" => {
            serde_json::json!({})
        }
        "geo-search" => match parse_geo_set(&args[1..]) {
            Ok(params) => params,
            Err(message) => {
                eprintln!("wloc-ctl: {message}");
                return 2;
            }
        },
        "geo-set" => match parse_geo_set(&args[1..]) {
            Ok(params) => params,
            Err(message) => {
                eprintln!("wloc-ctl: {message}");
                return 2;
            }
        },
        _ => unreachable!("unknown method returned earlier"),
    };
    let profile_id = match parse_profile_arg(&args[1..]) {
        Ok(profile_id) => profile_id,
        Err(message) => {
            eprintln!("wloc-ctl: {message}");
            return 2;
        }
    };
    if let Some(profile_id) = profile_id {
        params["profile_id"] = serde_json::Value::String(profile_id);
    }

    let request = serde_json::json!({
        "api_version": "wloc.service/v1",
        "request_id": format!("ctl-{}", SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)),
        "method": wire_method,
        "params": params,
    });
    let body = match serde_json::to_vec(&request) {
        Ok(body) => body,
        Err(error) => {
            eprintln!("wloc-ctl: 编码失败: {error}");
            return 1;
        }
    };

    let mut stream = match UnixStream::connect(socket_path) {
        Ok(stream) => stream,
        Err(error) => {
            eprintln!("wloc-ctl: 连接 {socket_path} 失败: {error}");
            return 1;
        }
    };

    if let Err(error) = stream.write_all(&(body.len() as u32).to_be_bytes()) {
        eprintln!("wloc-ctl: 写入失败: {error}");
        return 1;
    }
    if let Err(error) = stream.write_all(&body) {
        eprintln!("wloc-ctl: 写入失败: {error}");
        return 1;
    }
    let _ = stream.shutdown(Shutdown::Write);

    let mut header = [0_u8; 4];
    if stream.read_exact(&mut header).is_err() {
        eprintln!("wloc-ctl: 守护进程未返回响应即关闭连接");
        return 1;
    }
    let length = u32::from_be_bytes(header) as usize;
    let mut response = vec![0_u8; length];
    if stream.read_exact(&mut response).is_err() {
        eprintln!("wloc-ctl: 响应不完整");
        return 1;
    }

    match serde_json::from_slice::<serde_json::Value>(&response) {
        Ok(value) => {
            if value.get("error").is_some() {
                eprintln!(
                    "wloc-ctl: 错误 {}",
                    value["error"]["code"].as_str().unwrap_or("未知错误")
                );
                return 1;
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&value).unwrap_or_default()
            );
            0
        }
        Err(_) => {
            println!("{}", String::from_utf8_lossy(&response));
            0
        }
    }
}

fn parse_geo_set(args: &[String]) -> Result<serde_json::Value, String> {
    let mut query = None;
    let mut lat = None;
    let mut lon = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--query" => {
                index += 1;
                query = args.get(index).cloned();
            }
            "--lat" => {
                index += 1;
                lat = args.get(index).and_then(|value| value.parse::<f64>().ok());
            }
            "--lon" => {
                index += 1;
                lon = args.get(index).and_then(|value| value.parse::<f64>().ok());
            }
            other => return Err(format!("unexpected argument: {other}")),
        }
        index += 1;
    }
    match (query, lat, lon) {
        (Some(query), None, None) if !query.trim().is_empty() => {
            Ok(serde_json::json!({"query": query}))
        }
        (None, Some(lat), Some(lon)) => Ok(serde_json::json!({"latitude": lat, "longitude": lon})),
        _ => Err("geo-set 需要 --query \"地名\" 或 --lat <纬度> --lon <经度>".to_owned()),
    }
}

fn parse_profile_arg(args: &[String]) -> Result<Option<String>, String> {
    let mut profile_id = None;
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--profile" {
            index += 1;
            let value = args
                .get(index)
                .ok_or_else(|| "--profile requires a profile id".to_owned())?;
            if value.is_empty()
                || value.len() > 32
                || !value.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || byte == b'_'
                        || byte == b'-'
                })
            {
                return Err("invalid profile id".to_owned());
            }
            profile_id = Some(value.clone());
        }
        index += 1;
    }
    Ok(profile_id)
}

/// Map a CLI method name to its wire method.
fn map_wire_method(method: &str) -> Option<&'static str> {
    match method {
        "status" => Some("status.get"),
        "enable" => Some("control.enable"),
        "disable" => Some("control.disable"),
        "reload" => Some("control.reload"),
        "geo-set" => Some("geo.set"),
        "geo-clear" => Some("geo.clear"),
        "geo-search" => Some("geo.search"),
        "refresh" => Some("control.refresh"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;

    fn temp_socket(name: &str) -> PathBuf {
        let mut path = env::temp_dir();
        path.push(format!("wloc-ctl-{name}-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        path
    }

    /// Spawn a fake daemon that answers with `response` (raw bytes) and return its socket path.
    fn spawn_fake_daemon(name: &str, response: Vec<u8>) -> PathBuf {
        let path = temp_socket(name);
        let listener = UnixListener::bind(&path).unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut body = Vec::new();
            let mut buf = [0_u8; 4096];
            loop {
                let n = stream.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                body.extend_from_slice(&buf[..n]);
            }
            assert!(body.len() >= 4, "client must send a framed request");
            let _ = stream.write_all(&response);
            let _ = stream.shutdown(Shutdown::Write);
        });
        path
    }

    #[test]
    fn usage_without_args() {
        assert_eq!(run_with_args(&[], "/nonexistent.sock"), 2);
    }

    #[test]
    fn unknown_method_rejected() {
        assert_eq!(run_with_args(&["bogus".to_owned()], "/nonexistent.sock"), 2);
    }

    #[test]
    fn malformed_geo_set_rejected() {
        assert_eq!(
            run_with_args(
                &["geo-set".to_owned(), "--lat".to_owned()],
                "/nonexistent.sock"
            ),
            2
        );
    }

    #[test]
    fn profile_arg_is_validated() {
        assert_eq!(
            parse_profile_arg(&["--profile".to_owned(), "phone_1".to_owned()]).unwrap(),
            Some("phone_1".to_owned())
        );
        assert!(parse_profile_arg(&["--profile".to_owned()]).is_err());
        assert!(parse_profile_arg(&["--profile".to_owned(), "../phone".to_owned()]).is_err());
    }

    #[test]
    fn connect_failure_reported() {
        assert_eq!(
            run_with_args(&["status".to_owned()], "/nonexistent.sock"),
            1
        );
    }

    #[test]
    fn round_trip_prints_response() {
        let path = spawn_fake_daemon("ok", {
            let mut framed = (15_u32).to_be_bytes().to_vec();
            framed.extend_from_slice(br#"{"ok":true,"x":1}"#);
            framed
        });
        let code = run_with_args(&["status".to_owned()], path.to_str().unwrap());
        assert_eq!(code, 0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn round_trip_reports_daemon_error() {
        let body = br#"{"error":{"code":"internal"}}"#.to_vec();
        let mut framed = (body.len() as u32).to_be_bytes().to_vec();
        framed.extend_from_slice(&body);
        let path = spawn_fake_daemon("err", framed);
        let code = run_with_args(&["status".to_owned()], path.to_str().unwrap());
        assert_eq!(code, 1);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn round_trip_geo_set_query_sent() {
        let mut framed = (4_u32).to_be_bytes().to_vec();
        framed.extend_from_slice(&[0_u8; 4]);
        let path = spawn_fake_daemon("geoset", framed);
        let code = run_with_args(
            &[
                "geo-set".to_owned(),
                "--query".to_owned(),
                "London, UK".to_owned(),
            ],
            path.to_str().unwrap(),
        );
        assert_eq!(code, 0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn geo_set_parses_query() {
        let params = parse_geo_set(&["--query".to_owned(), "London, UK".to_owned()]).unwrap();
        assert_eq!(params["query"], "London, UK");
    }

    #[test]
    fn geo_set_parses_coordinates() {
        let params = parse_geo_set(&[
            "--lat".to_owned(),
            "51.5074".to_owned(),
            "--lon".to_owned(),
            "-0.1278".to_owned(),
        ])
        .unwrap();
        assert_eq!(params["latitude"], 51.5074);
        assert_eq!(params["longitude"], -0.1278);
    }

    #[test]
    fn geo_set_rejects_missing_or_conflicting_args() {
        assert!(parse_geo_set(&[]).is_err());
        assert!(parse_geo_set(&["--query".to_owned()]).is_err());
        assert!(parse_geo_set(&[
            "--query".to_owned(),
            "London".to_owned(),
            "--lat".to_owned(),
            "1.0".to_owned(),
            "--lon".to_owned(),
            "2.0".to_owned(),
        ])
        .is_err());
        assert!(parse_geo_set(&["--lat".to_owned(), "1.0".to_owned()]).is_err());
    }

    #[test]
    fn cli_methods_map_to_wire_methods() {
        let cases = [
            ("status", "status.get"),
            ("enable", "control.enable"),
            ("disable", "control.disable"),
            ("reload", "control.reload"),
            ("geo-set", "geo.set"),
            ("geo-clear", "geo.clear"),
            ("geo-search", "geo.search"),
            ("refresh", "control.refresh"),
        ];
        for (cli, wire) in cases {
            assert_eq!(map_wire_method(cli), Some(wire), "mapping for {cli}");
        }
        assert_eq!(map_wire_method("bogus"), None);
    }
}
