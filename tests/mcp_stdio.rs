#![cfg(windows)]

use serde_json::{Value, json};
use std::io::{Read, Write};
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

// Exercise the actual Windows-subsystem executable with the pipes an MCP host
// supplies. These requests only discover tools and read status, never execute
// input actions or change the laptop's power configuration.
#[test]
fn mcp_preserves_stdio_and_existing_tools() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_autoclicktimer"))
        .arg("mcp")
        .creation_flags(0x08000000)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    for request in [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"act_get_remote_mode","arguments":{}}}),
    ] {
        writeln!(input, "{request}").unwrap();
    }
    drop(input);
    let mut output = child.stdout.take().unwrap();
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let mut text = String::new();
        let result = output.read_to_string(&mut text).map(|_| text);
        let _ = send.send(result);
    });
    let output = match receive.recv_timeout(Duration::from_secs(20)) {
        Ok(result) => result.unwrap(),
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            panic!("MCP stdio did not finish: {error}");
        }
    };
    assert!(child.wait().unwrap().success());
    let replies: Vec<Value> = output
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(replies.len(), 3, "Unexpected stdout: {output}");
    assert_eq!(
        replies[0]["result"]["serverInfo"]["name"],
        "autoclicktimer-mcp"
    );
    let tools = replies[1]["result"]["tools"].as_array().unwrap();
    for name in [
        "act_execute_action",
        "act_schedule_queue",
        "act_set_caffeine",
        "act_get_remote_mode",
        "act_set_remote_mode",
    ] {
        assert!(
            tools.iter().any(|tool| tool["name"] == name),
            "Missing tool {name}"
        );
    }
    assert_eq!(replies[2]["result"]["isError"], false);
    let status: Value =
        serde_json::from_str(replies[2]["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert!(status["enabled"].is_boolean());
    assert!(status["effective"].is_boolean());
    assert!(status["active_scheme"].is_string());
}
