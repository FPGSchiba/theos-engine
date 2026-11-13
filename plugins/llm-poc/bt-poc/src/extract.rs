//! Pulls the first JSON object out of a free-form reply (grammar off).

use serde_json::Value;

/// The first balanced `{ ... }` of `text` that parses as JSON. Braces inside strings do not
/// count; code fences and prose around the object are ignored. `None` when there is none.
pub fn first_json_object(text: &str) -> Option<Value> {
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(off) = text[from..].find('{') {
        let start = from + off;
        if let Some(end) = balanced_end(bytes, start) {
            if let Ok(v @ Value::Object(_)) = serde_json::from_str(&text[start..=end]) {
                return Some(v);
            }
        }
        from = start + 1;
    }
    None
}

/// The index of the `}` that closes the `{` at `start`, if the braces balance.
fn balanced_end(bytes: &[u8], start: usize) -> Option<usize> {
    let (mut depth, mut in_str, mut escaped) = (0usize, false, false);
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if in_str {
            match (escaped, b) {
                (true, _) => escaped = false,
                (false, b'\\') => escaped = true,
                (false, b'"') => in_str = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_takes_the_first_balanced_object_and_reports_none_when_absent() {
        assert_eq!(
            first_json_object("Here you go: {\"a\": 1} done."),
            Some(json!({"a": 1}))
        );
        assert_eq!(
            first_json_object("```json\n{\"a\": {\"b\": [1, 2]}}\n```"),
            Some(json!({"a": {"b": [1, 2]}}))
        );
        assert_eq!(
            first_json_object("{\"a\": 1} then {\"b\": 2}"),
            Some(json!({"a": 1}))
        );
        assert_eq!(
            first_json_object(r#"{"s": "a } and { b \" }"}"#),
            Some(json!({"s": "a } and { b \" }"}))
        );
        assert_eq!(first_json_object("no object here"), None);
        assert_eq!(first_json_object("{\"unclosed\": 1"), None);
    }

    #[test]
    fn extract_skips_a_brace_that_does_not_start_a_json_object() {
        assert_eq!(
            first_json_object("use {braces} like so: {\"a\": 1}"),
            Some(json!({"a": 1}))
        );
    }
}
