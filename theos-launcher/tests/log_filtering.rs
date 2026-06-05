use assert_cmd::Command;
use regex::Regex;
use std::process::Output;

/// Remove common ANSI escape sequences so text-based assertions are stable.
fn strip_ansi(input: &str) -> String {
    // Matches CSI sequences like \x1b[31m and similar control codes.
    // This is conservative and removes common SGR (m) escapes.
    let ansi_re = Regex::new(r"\x1B\[[0-9;]*[mK]").unwrap();
    ansi_re.replace_all(input, "").into_owned()
}

/// Helper: pick the captured output to assert on.
/// Prefers stdout if it contains something; otherwise falls back to stderr.
/// Returns the chosen textual output and a tuple of (stdout, stderr) for diagnostics.
fn choose_output(output: &Output) -> (String, (Vec<u8>, Vec<u8>)) {
    let stdout = output.stdout.clone();
    let stderr = output.stderr.clone();

    let chosen_bytes = if !stdout.is_empty() {
        stdout.clone()
    } else {
        stderr.clone()
    };
    let chosen = String::from_utf8_lossy(&chosen_bytes).into_owned();

    (chosen, (stdout, stderr))
}

#[test]
fn defaults_to_info_when_rust_log_missing() {
    let mut cmd = Command::cargo_bin("theos-launcher").expect("binary exists");

    // run without RUST_LOG set (ensure it's not set in the child env)
    let assert = cmd
        .env_remove("RUST_LOG")
        .env("THEOS_DRY_RUN", "1")
        .assert()
        .success();
    let output = assert.get_output();

    let (raw_text, (stdout_bytes, stderr_bytes)) = choose_output(output);
    let clean_text = strip_ansi(&raw_text);

    // Find a timestamp, a bracketed level and a module: message
    let re = Regex::new(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z\s+\[[A-Z]+\]\s+.+?:\s+.+")
        .expect("regex should compile");

    if re.find(&clean_text).is_none() || !clean_text.contains("Kernel starting") {
        let stdout_text = String::from_utf8_lossy(&stdout_bytes);
        let stderr_text = String::from_utf8_lossy(&stderr_bytes);
        panic!(
            "log output did not match expected layout.\nRaw chosen output:\n{}\n\nStripped output:\n{}\n\nSTDOUT:\n{}\n\nSTDERR:\n{}\n",
            raw_text, clean_text, stdout_text, stderr_text
        );
    }
}

#[test]
fn invalid_rust_log_does_not_crash_and_falls_back() {
    let mut cmd = Command::cargo_bin("theos-launcher").expect("binary exists");

    // set an intentionally malformed RUST_LOG
    let assert = cmd
        .env("RUST_LOG", "not-a-valid-filter!!!")
        .env("THEOS_DRY_RUN", "1")
        .assert()
        .success();

    let output = assert.get_output();
    let (raw_text, (stdout_bytes, stderr_bytes)) = choose_output(output);

    // If nothing was produced, show both streams for diagnosis and fail
    if raw_text.trim().is_empty() {
        let stdout_text = String::from_utf8_lossy(&stdout_bytes);
        let stderr_text = String::from_utf8_lossy(&stderr_bytes);
        panic!(
            "expected some log output even with invalid RUST_LOG; got empty\n\nSTDOUT:\n{}\n\nSTDERR:\n{}\n",
            stdout_text, stderr_text
        );
    }

    let clean_text = strip_ansi(&raw_text);

    // Ensure we see at least one log level token (INFO/WARN/ERROR)
    if !(clean_text.contains("[INFO]")
        || clean_text.contains("[WARN]")
        || clean_text.contains("[ERROR]"))
    {
        panic!(
            "expected fallback log level token in output, got:\n{}",
            clean_text
        );
    }
}
