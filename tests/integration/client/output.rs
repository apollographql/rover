use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

#[test]
fn client_check_json_output_includes_validation_results() {
    // Minimal HTTP server; skip if binding fails.
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(l) => l,
        Err(_) => return,
    };
    let addr = listener.local_addr().unwrap();
    let response_body = r#"{"data": {"service": {"validateOperations": {"validationResults": [{"type":"FAILURE","code":"BAD","description":"nope","operation":{"name":"Hello"}}]}}}}""#;
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut _buf = [0u8; 1024];
            let _ = stream.read(&mut _buf);
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });

    let temp = tempfile::tempdir().unwrap();
    let graphql = temp.path().join("op.graphql");
    fs::write(&graphql, "query Hello { hello }").unwrap();

    let output = Command::cargo_bin("rover")
        .unwrap()
        .env("APOLLO_KEY", "testkey")
        .env(
            "APOLLO_REGISTRY_URL",
            format!("http://{}:{}", addr.ip(), addr.port()),
        )
        .current_dir(temp.path())
        .arg("client")
        .arg("check")
        .arg("graph@current")
        .arg("--include")
        .arg(graphql.to_str().unwrap())
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();

    // Validation failure should exit non-zero now
    assert!(!output.status.success());

    let stdout = output.stdout.clone();

    let json: Value = serde_json::from_slice(&stdout).unwrap();
    assert_eq!(
        json["data"]["client_check"]["validation_results"][0]["type"],
        "FAILURE"
    );
}
