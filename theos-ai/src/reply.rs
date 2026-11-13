//! The structured reply an NPC produces: a JSON schema to constrain generation
//! and a parser that validates the result against the lore shown in the prompt.

use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::driver::StopReason;
use crate::lore::RecordId;

/// Most lore records a single reply may cite.
pub const MAX_SOURCES: usize = 5;
/// Longest reply text, in characters, the schema allows.
pub const MAX_REPLY_CHARS: usize = 600;

/// A parsed, validated NPC reply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reply {
    /// Lore records the reply cites, deduplicated, in first-cited order.
    pub sources: Vec<RecordId>,
    /// What the NPC says.
    pub text: String,
}

/// Why a raw model output could not be turned into a [`Reply`].
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ReplyError {
    /// The output is not valid JSON or does not match the reply shape.
    #[error("malformed reply: {0}")]
    Malformed(String),
    /// The reply cites a record that was not in the prompt.
    #[error("reply cites `{0}`, which was not in the prompt")]
    UnknownSource(RecordId),
    /// The reply text is empty or only whitespace.
    #[error("reply text is empty")]
    EmptyReply,
    /// Generation stopped before the model finished, so there is nothing to parse.
    #[error("generation did not finish: {0:?}")]
    Unfinished(StopReason),
}

/// Wire shape of a reply; rejects keys the schema does not define.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireReply {
    sources: Vec<RecordId>,
    reply: String,
}

/// Builds the JSON schema that constrains a reply for a prompt containing `lore_ids`.
///
/// `sources` may only hold those ids, at most [`MAX_SOURCES`]; with no lore it is
/// fixed to the empty array. Properties are inserted `sources` first, then `reply`:
/// the order is load-bearing, because the backend relies on it so the model commits
/// to its records before it speaks.
pub fn reply_schema(lore_ids: &[RecordId]) -> Value {
    let mut sources = Map::new();
    sources.insert("type".into(), json!("array"));
    if lore_ids.is_empty() {
        sources.insert("maxItems".into(), json!(0));
    } else {
        sources.insert("items".into(), json!({ "enum": lore_ids }));
        sources.insert("maxItems".into(), json!(MAX_SOURCES));
    }

    let mut properties = Map::new();
    properties.insert("sources".into(), Value::Object(sources));
    properties.insert(
        "reply".into(),
        json!({ "type": "string", "minLength": 1, "maxLength": MAX_REPLY_CHARS }),
    );

    json!({
        "type": "object",
        "properties": Value::Object(properties),
        "required": ["sources", "reply"],
        "additionalProperties": false,
    })
}

/// Parses raw model output into a [`Reply`], checking it against `lore_ids`.
///
/// Surrounding whitespace is ignored and duplicate sources are dropped, keeping
/// first-cited order.
///
/// # Errors
///
/// [`ReplyError::Malformed`] for bad JSON or unexpected keys,
/// [`ReplyError::UnknownSource`] for a source not in `lore_ids`, and
/// [`ReplyError::EmptyReply`] for a blank reply text.
pub fn parse_reply(raw: &str, lore_ids: &[RecordId]) -> Result<Reply, ReplyError> {
    let wire: WireReply =
        serde_json::from_str(raw.trim()).map_err(|e| ReplyError::Malformed(e.to_string()))?;
    let mut sources: Vec<RecordId> = Vec::with_capacity(wire.sources.len());
    for id in wire.sources {
        if !lore_ids.contains(&id) {
            return Err(ReplyError::UnknownSource(id));
        }
        if !sources.contains(&id) {
            sources.push(id);
        }
    }
    if wire.reply.trim().is_empty() {
        return Err(ReplyError::EmptyReply);
    }
    Ok(Reply {
        sources,
        text: wire.reply,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(xs: &[&str]) -> Vec<RecordId> {
        xs.iter().map(|s| RecordId::from(*s)).collect()
    }

    #[test]
    fn reply_schema_lists_lore_ids_as_source_enum() {
        let schema = reply_schema(&ids(&["B-945", "B-946"]));
        let sources = &schema["properties"]["sources"];
        assert_eq!(sources["items"]["enum"], json!(["B-945", "B-946"]));
        assert_eq!(sources["maxItems"], json!(5));
    }

    #[test]
    fn reply_schema_puts_sources_before_reply() {
        let schema = reply_schema(&ids(&["B-945"]));
        let keys: Vec<_> = schema["properties"].as_object().unwrap().keys().collect();
        assert_eq!(keys, ["sources", "reply"]);
    }

    #[test]
    fn reply_schema_with_no_lore_fixes_sources_empty() {
        let schema = reply_schema(&[]);
        let sources = &schema["properties"]["sources"];
        assert_eq!(sources["maxItems"], json!(0));
        assert!(sources.get("items").is_none());
    }

    #[test]
    fn reply_schema_bounds_reply_length_and_forbids_extra_keys() {
        let schema = reply_schema(&[]);
        let reply = &schema["properties"]["reply"];
        assert_eq!(reply["minLength"], json!(1));
        assert_eq!(reply["maxLength"], json!(600));
        assert_eq!(schema["additionalProperties"], json!(false));
        assert_eq!(schema["required"], json!(["sources", "reply"]));
    }

    #[test]
    fn parse_reply_reads_sources_and_text() {
        let r = parse_reply(r#"{"sources":["B-946"],"reply":"Hush."}"#, &ids(&["B-946"])).unwrap();
        assert_eq!(r.sources, ids(&["B-946"]));
        assert_eq!(r.text, "Hush.");
    }

    #[test]
    fn parse_reply_decodes_escapes_and_ignores_surrounding_whitespace() {
        let raw = "\n  {\"sources\":[],\"reply\":\"She said \\\"no\\\".\\nÆther – ok\"}  \n";
        let r = parse_reply(raw, &[]).unwrap();
        assert_eq!(r.text, "She said \"no\".\nÆther – ok");
    }

    #[test]
    fn parse_reply_rejects_source_outside_context() {
        let err = parse_reply(r#"{"sources":["B-1"],"reply":"x"}"#, &ids(&["B-2"])).unwrap_err();
        assert_eq!(err, ReplyError::UnknownSource(RecordId::from("B-1")));
    }

    #[test]
    fn parse_reply_drops_duplicate_sources_keeping_order() {
        let raw = r#"{"sources":["B-2","B-1","B-2"],"reply":"x"}"#;
        let r = parse_reply(raw, &ids(&["B-1", "B-2"])).unwrap();
        assert_eq!(r.sources, ids(&["B-2", "B-1"]));
    }

    #[test]
    fn parse_reply_rejects_blank_reply() {
        let err = parse_reply(r#"{"sources":[],"reply":"   "}"#, &[]).unwrap_err();
        assert_eq!(err, ReplyError::EmptyReply);
    }

    #[test]
    fn parse_reply_rejects_extra_keys_and_bad_json() {
        let extra = parse_reply(r#"{"sources":[],"reply":"x","mood":"calm"}"#, &[]);
        assert!(matches!(extra, Err(ReplyError::Malformed(_))));
        let bad = parse_reply("not json", &[]);
        assert!(matches!(bad, Err(ReplyError::Malformed(_))));
    }
}
