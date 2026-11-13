//! The JSON schema that constrains a tree reply, built per scenario from the vocabulary.
//!
//! The shape is the one the spike proved against llguidance: an envelope with `sources` before
//! `tree`, and a recursive `$defs.node` that is an `anyOf` of node shapes. Conditions and actions
//! are split per check and per component field so their `params` are fully typed.

use serde_json::{json, Map, Value};
use theos_ai::RecordId;

use crate::fixture::{BtScenario, Entity, Fixture};

/// An object with exactly these properties, in this order, all required.
fn obj(props: Vec<(&str, Value)>) -> Value {
    let required: Vec<&str> = props.iter().map(|(k, _)| *k).collect();
    let properties: Map<String, Value> =
        props.into_iter().map(|(k, v)| (k.to_owned(), v)).collect();
    let mut o = json!({"type": "object", "properties": properties, "additionalProperties": false});
    if !required.is_empty() {
        o["required"] = json!(required);
    }
    o
}

fn tag(name: &str) -> Value {
    json!({"const": name})
}

fn node_ref() -> Value {
    json!({"$ref": "#/$defs/node"})
}

/// The scenario-specific enumerations a parameter schema can ask for.
struct Scope<'a> {
    entities: &'a [Entity],
    flags: Vec<String>,
}

impl Scope<'_> {
    /// A parameter schema from the vocabulary, with the `x-theos-*` keywords resolved against
    /// this scenario. `None` when it resolves to an empty enumeration (nothing could be named).
    fn param(&self, spec: &Value) -> Option<Value> {
        let get = |k: &str| spec.get(k);
        let truthy = |k: &str| get(k).and_then(Value::as_bool).unwrap_or(false);
        let mut names: Option<Vec<String>> = None;
        if let Some(kind) = get("x-theos-entity-kind").and_then(Value::as_str) {
            names = Some(self.handles(Some(kind)));
        } else if truthy("x-theos-live-entity") {
            names = Some(self.handles(None));
        } else if truthy("x-theos-scenario-flag") {
            names = Some(self.flags.clone());
        }
        if let Some(mut names) = names {
            if truthy("x-theos-allow-none") {
                names.push("none".to_owned());
            }
            if names.is_empty() {
                return None;
            }
            return Some(json!({"type": "string", "enum": names}));
        }
        let clean: Map<String, Value> = spec
            .as_object()?
            .iter()
            .filter(|(k, _)| !k.starts_with("x-theos-"))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Some(Value::Object(clean))
    }

    fn handles(&self, kind: Option<&str>) -> Vec<String> {
        self.entities
            .iter()
            .filter(|e| kind.is_none_or(|k| e.kind == k))
            .map(|e| e.handle.clone())
            .collect()
    }

    /// An object of typed parameters; `None` when any of them cannot be satisfied.
    fn params(&self, spec: &Value) -> Option<Value> {
        let mut props = Vec::new();
        if let Some(m) = spec.as_object() {
            for (k, v) in m {
                props.push((k.as_str(), self.param(v)?));
            }
        }
        Some(obj(props))
    }
}

fn children() -> Value {
    json!({"type": "array", "items": node_ref(), "minItems": 1})
}

fn condition(check: &str, params: Value) -> Value {
    obj(vec![
        ("type", tag("Condition")),
        ("check", tag(check)),
        ("params", params),
    ])
}

fn set_component(component: &str, field: &str, value: Value) -> Value {
    obj(vec![
        ("type", tag("Action")),
        ("command", tag("SetComponentValue")),
        (
            "params",
            obj(vec![
                ("component", tag(component)),
                ("field", tag(field)),
                ("value", value),
            ]),
        ),
    ])
}

fn trigger_event(event: &str, payload: Value) -> Value {
    obj(vec![
        ("type", tag("Action")),
        ("command", tag("TriggerEvent")),
        (
            "params",
            obj(vec![("event_name", tag(event)), ("payload", payload)]),
        ),
    ])
}

/// The recursive tree schema of one scenario. `lore_ids` are the ids actually rendered in the
/// prompt; they are the only values `sources` may hold.
pub fn tree_schema(fixture: &Fixture, scenario: &BtScenario, lore_ids: &[RecordId]) -> Value {
    let vocab = &fixture.vocab;
    let mut flags: Vec<String> = scenario.flags.keys().cloned().collect();
    for f in &scenario.world.flags {
        if !flags.contains(f) {
            flags.push(f.clone());
        }
    }
    let scope = Scope {
        entities: &scenario.world.entities,
        flags,
    };

    let mut variants: Vec<Value> = Vec::new();

    // Structural nodes: whatever the vocabulary lists besides Condition and Action.
    if let Some(types) = vocab["node_types"].as_object() {
        for (name, def) in types {
            if name == "Condition" || name == "Action" {
                continue;
            }
            let mut props = vec![("type", tag(name))];
            if let Some(fields) = def["fields"].as_object() {
                for (field, spec) in fields {
                    let v = match spec.as_str() {
                        Some("nodes") => children(),
                        Some("node") => node_ref(),
                        _ => match scope.param(spec) {
                            Some(v) => v,
                            None => continue,
                        },
                    };
                    props.push((field.as_str(), v));
                }
            }
            variants.push(obj(props));
        }
    }

    // One Condition per registered check, params typed.
    if let Some(conds) = vocab["conditions"].as_object() {
        for (name, def) in conds {
            if let Some(p) = scope.params(&def["params"]) {
                variants.push(condition(name, p));
            }
        }
    }

    // SetComponentValue per component field; the value is typed by the field.
    if let Some(components) = vocab["components"].as_object() {
        for (component, fields) in components {
            if let Some(fields) = fields.as_object() {
                for (field, spec) in fields {
                    if let Some(v) = scope.param(spec) {
                        variants.push(set_component(component, field, v));
                    }
                }
            }
        }
    }

    // TriggerEvent per event this actor may fire.
    let own: Vec<&str> = vocab["actors"][scenario.actor.as_str()]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    for event in own {
        if let Some(p) = scope.params(&vocab["events"][event]["payload"]) {
            variants.push(trigger_event(event, p));
        }
    }

    let sources = if lore_ids.is_empty() {
        json!({"type": "array", "maxItems": 0})
    } else {
        let ids: Vec<&str> = lore_ids.iter().map(RecordId::as_str).collect();
        json!({"type": "array", "items": {"enum": ids}, "maxItems": ids.len()})
    };
    let mut root = obj(vec![
        ("sources", sources),
        (
            "tree",
            obj(vec![
                (
                    "name",
                    json!({"type": "string", "minLength": 1, "maxLength": 80}),
                ),
                ("root", node_ref()),
            ]),
        ),
    ]);
    root["$defs"] = json!({"node": {"anyOf": variants}});
    root
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::tests::load_default;
    use std::path::Path;

    fn ids_of(fx: &Fixture, s: &BtScenario) -> Vec<RecordId> {
        fx.visible.get(&s.actor).map_or_else(Vec::new, |e| {
            e.held
                .iter()
                .chain(&e.rejected)
                .map(RecordId::new)
                .collect()
        })
    }

    fn reference(name: &str) -> Value {
        let p = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/bt-poc/reference")
            .join(name);
        serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
    }

    fn scenario<'a>(fx: &'a Fixture, id: &str) -> &'a BtScenario {
        fx.scenarios.iter().find(|s| s.id == id).unwrap()
    }

    fn variants(schema: &Value) -> &Vec<Value> {
        schema["$defs"]["node"]["anyOf"].as_array().unwrap()
    }

    #[test]
    fn every_good_reference_tree_validates_against_its_schema() {
        let fx = load_default();
        let mut seen = 0;
        for s in &fx.scenarios {
            let schema = tree_schema(&fx, s, &ids_of(&fx, s));
            let v = jsonschema::validator_for(&schema).unwrap();
            let reply = reference(&format!("{}-good.json", s.id))["reply"].clone();
            let errs: Vec<String> = v.iter_errors(&reply).map(|e| e.to_string()).collect();
            assert!(errs.is_empty(), "{}: {errs:?}", s.id);
            seen += 1;
        }
        assert_eq!(seen, 20);
    }

    /// Validates a gate file's reply against its scenario's schema.
    fn gate_file_validates(fx: &Fixture, file: &str) -> bool {
        let r = reference(file);
        let s = scenario(fx, r["scenario"].as_str().unwrap());
        let schema = tree_schema(fx, s, &ids_of(fx, s));
        jsonschema::validator_for(&schema)
            .unwrap()
            .is_valid(&r["reply"])
    }

    #[test]
    fn unregistered_commands_conditions_events_and_modes_do_not_validate() {
        let fx = load_default();
        for file in [
            "b18-bad.json",
            "g3-gate.json",
            "g6-gate.json",
            "g8-gate.json",
        ] {
            assert!(!gate_file_validates(&fx, file), "{file} must not validate");
        }
    }

    #[test]
    fn gate_trees_with_bad_params_or_sources_do_not_validate() {
        let fx = load_default();
        for file in [
            "g4-gate.json",
            "g5-gate.json",
            "g7-gate.json",
            "g9-gate.json",
            "g10-gate.json",
        ] {
            assert!(!gate_file_validates(&fx, file), "{file} must not validate");
        }
    }

    #[test]
    fn depth_and_node_limits_are_left_to_the_gate() {
        let fx = load_default();
        for file in ["g1-gate.json", "g2-gate.json"] {
            assert!(gate_file_validates(&fx, file), "{file} must still validate");
        }
    }

    fn wait(seconds: f64) -> Value {
        json!({"type": "Wait", "seconds": seconds})
    }

    fn health(threshold: f64) -> Value {
        json!({"type": "Condition", "check": "health_below", "params": {"threshold": threshold}})
    }

    fn envelope(root: Value) -> Value {
        json!({"sources": [], "tree": {"name": "t", "root": root}})
    }

    #[test]
    fn every_structural_node_type_validates_at_its_bounds_and_nowhere_outside() {
        let fx = load_default();
        let schema = tree_schema(&fx, scenario(&fx, "b14"), &[]);
        let v = jsonschema::validator_for(&schema).unwrap();
        let valid = [
            json!({"type": "Parallel", "policy": "all", "children": [wait(1.0)]}),
            json!({"type": "Parallel", "policy": "one", "children": [wait(1.0), health(0.5)]}),
            json!({"type": "Decorator_Repeat", "count": 1, "child": wait(1.0)}),
            json!({"type": "Decorator_Repeat", "count": 10, "child": wait(1.0)}),
            json!({"type": "Decorator_Timeout", "seconds": 0.5, "child": wait(1.0)}),
            json!({"type": "Decorator_Timeout", "seconds": 30, "child": wait(1.0)}),
            wait(0.5),
            wait(30.0),
            json!({"type": "Decorator_Invert", "child": wait(1.0)}),
            health(0.0),
            health(1.0),
        ];
        for (i, root) in valid.into_iter().enumerate() {
            let reply = envelope(root);
            assert!(v.is_valid(&reply), "valid[{i}] rejected: {reply}");
        }
        let invalid = [
            wait(0.1),
            wait(31.0),
            json!({"type": "Decorator_Repeat", "count": 0, "child": wait(1.0)}),
            json!({"type": "Decorator_Repeat", "count": 11, "child": wait(1.0)}),
            json!({"type": "Decorator_Timeout", "seconds": 31, "child": wait(1.0)}),
            json!({"type": "Decorator_Timeout", "seconds": 0.1, "child": wait(1.0)}),
            json!({"type": "Parallel", "policy": "some", "children": [wait(1.0)]}),
            json!({"type": "Sequence", "children": []}),
            health(1.5),
        ];
        for (i, root) in invalid.into_iter().enumerate() {
            let reply = envelope(root);
            assert!(!v.is_valid(&reply), "invalid[{i}] accepted: {reply}");
        }
    }

    #[test]
    fn sources_are_fixed_empty_without_lore() {
        let fx = load_default();
        let schema = tree_schema(&fx, scenario(&fx, "b10"), &[]);
        let src = &schema["properties"]["sources"];
        assert_eq!(src["maxItems"], 0);
        assert!(src.get("items").is_none());
    }

    #[test]
    fn sources_enumerate_the_given_lore_ids() {
        let fx = load_default();
        let ids = [RecordId::new("B-918"), RecordId::new("B-919")];
        let schema = tree_schema(&fx, scenario(&fx, "b01"), &ids);
        let src = &schema["properties"]["sources"];
        assert_eq!(src["maxItems"], 2);
        assert_eq!(src["items"]["enum"], json!(["B-918", "B-919"]));
    }

    #[test]
    fn sources_come_before_tree() {
        let fx = load_default();
        let schema = tree_schema(&fx, scenario(&fx, "b10"), &[]);
        let keys: Vec<&String> = schema["properties"].as_object().unwrap().keys().collect();
        assert_eq!(keys, ["sources", "tree"]);
        assert_eq!(schema["required"], json!(["sources", "tree"]));
        assert_eq!(schema["additionalProperties"], false);
    }

    #[test]
    fn entity_params_are_limited_to_this_scenarios_handles() {
        let fx = load_default();
        let s = scenario(&fx, "b14");
        let handles: Vec<&str> = s.world.entities.iter().map(|e| e.handle.as_str()).collect();
        let schema = tree_schema(&fx, s, &[]);
        let range = variants(&schema)
            .iter()
            .find(|v| v["properties"]["check"]["const"] == "entity_in_range")
            .unwrap();
        assert_eq!(
            range["properties"]["params"]["properties"]["target_entity"]["enum"],
            json!(handles)
        );
        // Target takes the handles plus `none`.
        let target = variants(&schema)
            .iter()
            .find(|v| v["properties"]["params"]["properties"]["component"]["const"] == "Target")
            .unwrap();
        let mut with_none = handles.clone();
        with_none.push("none");
        assert_eq!(
            target["properties"]["params"]["properties"]["value"]["enum"],
            json!(with_none)
        );
        // b06: BindSpirit takes spirit-kind handles only.
        let b06 = scenario(&fx, "b06");
        let spirits: Vec<&str> = b06
            .world
            .entities
            .iter()
            .filter(|e| e.kind == "spirit")
            .map(|e| e.handle.as_str())
            .collect();
        let schema = tree_schema(&fx, b06, &[]);
        let bind = variants(&schema)
            .iter()
            .find(|v| {
                v["properties"]["params"]["properties"]["event_name"]["const"] == "BindSpirit"
            })
            .unwrap();
        assert_eq!(
            bind["properties"]["params"]["properties"]["payload"]["properties"]["spirit"]["enum"],
            json!(spirits)
        );
    }

    #[test]
    fn events_are_limited_to_the_actors_own() {
        let fx = load_default();
        let schema = tree_schema(&fx, scenario(&fx, "b10"), &[]);
        assert!(!variants(&schema)
            .iter()
            .any(|v| v["properties"]["command"]["const"] == "TriggerEvent"));
        let schema = tree_schema(&fx, scenario(&fx, "b14"), &[]);
        let events: Vec<&Value> = variants(&schema)
            .iter()
            .filter(|v| v["properties"]["command"]["const"] == "TriggerEvent")
            .map(|v| &v["properties"]["params"]["properties"]["event_name"]["const"])
            .collect();
        assert_eq!(events, [&json!("CallAllies")]);
    }
}
