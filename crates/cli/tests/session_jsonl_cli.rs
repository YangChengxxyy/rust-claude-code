use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::thread;

#[test]
fn print_mode_writes_one_session_end_event() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let addr = listener.local_addr().expect("mock server addr");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let mut buf = [0_u8; 8192];
        let _ = stream.read(&mut buf).expect("read request");
        let body = r#"{"id":"msg_mock_1","type":"message","role":"assistant","content":[{"type":"text","text":"mock pong"}],"model":"mock-model","stop_reason":"end_turn","stop_sequence":null,"usage":{"input_tokens":1,"output_tokens":2,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
            body.len(), body
        );
        stream.write_all(response.as_bytes()).expect("write response");
    });

    let temp_root = std::env::temp_dir().join(format!(
        "rust-claude-jsonl-cli-test-{}-{}",
        std::process::id(),
        chrono::Local::now()
            .timestamp_nanos_opt()
            .unwrap_or_default()
    ));
    let home = temp_root.join("home");
    let claude_config = temp_root.join("claude");
    let project = temp_root.join("project");
    fs::create_dir_all(&home).expect("create home");
    fs::create_dir_all(&claude_config).expect("create claude config");
    fs::create_dir_all(&project).expect("create project");

    let output = Command::new(env!("CARGO_BIN_EXE_rust-claude"))
        .current_dir(&project)
        .env("HOME", &home)
        .env("CLAUDE_CONFIG_DIR", &claude_config)
        .env("ANTHROPIC_API_KEY", "test-key")
        .env("RUST_CLAUDE_BASE_URL", format!("http://{}", addr))
        .env("RUST_CLAUDE_STREAM", "0")
        .arg("--trust")
        .arg("--no-stream")
        .arg("Reply with exactly mock pong")
        .output()
        .expect("run cli");

    server.join().expect("mock server exits");

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let sessions_dir = home.join(".config").join("rust-claude-code").join("sessions");
    let files: Vec<_> = fs::read_dir(&sessions_dir)
        .expect("read sessions dir")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        .collect();
    assert_eq!(files.len(), 1, "expected exactly one jsonl session file");

    let content = fs::read_to_string(&files[0]).expect("read session jsonl");
    let session_end_count = content
        .lines()
        .filter(|line| line.contains(r#""type":"session_end""#))
        .count();
    assert_eq!(session_end_count, 1, "jsonl content:\n{content}");

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn resume_completed_jsonl_appends_without_truncating_history() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let addr = listener.local_addr().expect("mock server addr");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let mut buf = [0_u8; 8192];
        let _ = stream.read(&mut buf).expect("read request");
        let body = r#"{"id":"msg_mock_2","type":"message","role":"assistant","content":[{"type":"text","text":"resumed pong"}],"model":"mock-model","stop_reason":"end_turn","stop_sequence":null,"usage":{"input_tokens":1,"output_tokens":2,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
            body.len(), body
        );
        stream.write_all(response.as_bytes()).expect("write response");
    });

    let temp_root = std::env::temp_dir().join(format!(
        "rust-claude-jsonl-resume-test-{}-{}",
        std::process::id(),
        chrono::Local::now()
            .timestamp_nanos_opt()
            .unwrap_or_default()
    ));
    let home = temp_root.join("home");
    let claude_config = temp_root.join("claude");
    let project = temp_root.join("project");
    let sessions_dir = home.join(".config").join("rust-claude-code").join("sessions");
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");
    fs::create_dir_all(&claude_config).expect("create claude config");
    fs::create_dir_all(&project).expect("create project");

    let session_id = "20990103_000000";
    let session_path = sessions_dir.join(format!("{session_id}.jsonl"));
    fs::write(
        &session_path,
        concat!(
            "{\"type\":\"header\",\"id\":\"20990103_000000\",\"model\":\"mock-model\",\"model_setting\":\"mock-model\",\"cwd\":\"/tmp\",\"created_at\":\"2026-05-10T00:00:00+00:00\"}\n",
            "{\"type\":\"user_message\",\"message\":{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"original prompt\"}]}}\n",
            "{\"type\":\"assistant_message\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"original answer\"}]}}\n",
            "{\"type\":\"session_end\",\"updated_at\":\"2026-05-10T00:00:01+00:00\"}\n",
        ),
    )
    .expect("write completed session");

    let output = Command::new(env!("CARGO_BIN_EXE_rust-claude"))
        .current_dir(&project)
        .env("HOME", &home)
        .env("CLAUDE_CONFIG_DIR", &claude_config)
        .env("ANTHROPIC_API_KEY", "test-key")
        .env("RUST_CLAUDE_BASE_URL", format!("http://{}", addr))
        .env("RUST_CLAUDE_STREAM", "0")
        .arg("--trust")
        .arg("--no-stream")
        .arg("--resume")
        .arg(session_id)
        .arg("new prompt")
        .output()
        .expect("run cli");

    server.join().expect("mock server exits");

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let content = fs::read_to_string(&session_path).expect("read session jsonl");
    assert!(content.contains("original prompt"), "jsonl content:\n{content}");
    assert!(content.contains("original answer"), "jsonl content:\n{content}");
    assert!(content.contains("new prompt"), "jsonl content:\n{content}");
    assert!(content.contains("resumed pong"), "jsonl content:\n{content}");

    let _ = fs::remove_dir_all(&temp_root);
}
