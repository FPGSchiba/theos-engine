use assert_cmd::Command;
use regex::Regex;
use std::process::Output;

/// Remove common ANSI escape sequences so text-based assertions are stable.
fn strip_ansi(input: &str) -> String {
    let ansi_re = Regex::new(r"\x1B\[[0-9;]*[mK]").unwrap();
    ansi_re.replace_all(input, "").into_owned()
}

/// Helper: pick the captured output to assert on.
/// Prefers stdout if it contains something; otherwise falls back to stderr.
fn choose_output(output: &Output) -> String {
    let stdout = output.stdout.clone();
    let stderr = output.stderr.clone();

    if !stdout.is_empty() {
        String::from_utf8_lossy(&stdout).into_owned()
    } else {
        String::from_utf8_lossy(&stderr).into_owned()
    }
}

#[test]
fn startup_line_matches_layout() {
    let mut cmd = Command::cargo_bin("theos-launcher").expect("binary exists");

    let assert = cmd
        .env("RUST_LOG", "not-a-valid-filter!!!")
        .assert()
        .success();

    let output = assert.get_output();

    let raw_text = choose_output(output);
    let clean_text = strip_ansi(&raw_text);

    // Example expected shape:
    // 2026-04-27T20:05:52.6484254Z [INFO] theos_launcher: Theos Engine v0.1.0 - Kernel starting...
    // Allow flexible spacing and capture timestamp, level, module, and message.
    let re =
        Regex::new(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z\s+\[[A-Z]+\]\s+[\w_:]+:\s+.+")
            .expect("regex should compile");

    assert!(
        re.is_match(&clean_text),
        "log output did not match expected layout.\nRaw:\n{}\n\nStripped:\n{}",
        raw_text,
        clean_text
    );

    assert!(
        clean_text.contains("Kernel starting"),
        "expected startup message in output.\nOutput:\n{}",
        clean_text
    );
}

#[test]
fn module_path_is_present() {
    let mut cmd = Command::cargo_bin("theos-launcher").expect("binary exists");

    let assert = cmd
        .env("RUST_LOG", "not-a-valid-filter!!!")
        .assert()
        .success();

    let output = assert.get_output();

    let raw_text = choose_output(output);
    let clean_text = strip_ansi(&raw_text);

    // Match the module/target path between ] and :
    // Example: [INFO] theos_launcher:
    let re = Regex::new(r"]\s+([\w_:]+):").expect("regex should compile");

    let captures = re
        .captures(&clean_text)
        .unwrap_or_else(|| panic!("regex should match in:\n{}", clean_text));

    assert_eq!(&captures[1], "theos_launcher");
}
