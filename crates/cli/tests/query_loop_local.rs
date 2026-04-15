use std::process::Command;

#[test]
#[ignore = "requires local compatible endpoint"]
fn cli_runs_against_local_compatible_endpoint() {
    let output = Command::new(env!("CARGO_BIN_EXE_rust-claude"))
        .env("ANTHROPIC_API_KEY", "ah-1234567890")
        .env("RUST_CLAUDE_BASE_URL", "http://127.0.0.1:8787")
        .env("RUST_CLAUDE_BEARER_AUTH", "1")
        .env("RUST_CLAUDE_MODEL_OVERRIDE", "claude-haiku-4-5-20251001")
        .arg("Reply with exactly: pong")
        .output()
        .expect("cli should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Initialized session state from config."));
    assert!(stdout.to_ascii_lowercase().contains("pong"));
}
