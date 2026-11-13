//! The prompt a local model gets to write a behaviour tree.
//!
//! The system part holds everything but the directive: the actor, the lore (NPCs
//! only), the situation, the vocabulary, the worked examples and the reply format.
//! The user part is the directive. The wording is written once and not tuned.

use std::fmt::Write as _;

use serde_json::Value;
use theos_ai::{HolderKind, PersonaLayer, Prompt, RecordId, Speaker};

use crate::fixture::{BtScenario, Entity, Fixture};

/// The user part when the scenario has no directive.
const NO_DIRECTIVE: &str = "Act on your record and the situation.";

/// The node types in the order the prompt lists them.
const NODE_ORDER: [&str; 9] = [
    "Sequence",
    "Selector",
    "Parallel",
    "Decorator_Repeat",
    "Decorator_Invert",
    "Decorator_Timeout",
    "Condition",
    "Action",
    "Wait",
];

const TICK_RULES: &str = "How a tick runs:
- The tree is ticked every 0.5 s while it runs. Each tick evaluates the root once; a node returns success, failure or running.
- Sequence runs its children in order: a failure stops it with failure, a running child stops it with running, and when all succeed it succeeds. Selector is the mirror image: a success stops it with success, and when all fail it fails.
- A Sequence or Selector that returned running resumes at that child on the next tick; the children before it are not run again until the composite finishes. A tree with no Wait finishes every tick, so it starts again from the root each tick and reacts at once.
- Parallel ticks every child each tick. With policy \"all\" it fails on any failure and succeeds when all succeed; with \"one\" it succeeds on any success and fails when all fail. Otherwise it is running.
- Condition succeeds when its check is true. Action applies its effect and succeeds: SetComponentValue sets a field, TriggerEvent fires the event.
- Wait is running until its seconds have passed since it started, then succeeds.
- Decorator_Invert swaps success and failure. Decorator_Timeout fails, and resets its child, when the child is still running after its seconds. Decorator_Repeat runs its child count times (at most 10).
- health_below is strict: your health is below the threshold. entity_in_range is false for an entity that is gone. allies_in_range counts entities of kind ally, never you. time_since_install_over is true once that many seconds have passed since the tree was installed.";

/// One entity line of the situation: `- handle: kind, 12 m`, plus `, called "name"` when named.
pub fn entity_line(e: &Entity) -> String {
    let mut line = format!("- {}: {}, {} m", e.handle, e.kind, e.distance);
    if let Some(name) = &e.name {
        let _ = write!(line, ", called \"{name}\"");
    }
    line
}

fn layer_of(speaker: &Speaker, kind: HolderKind) -> Option<&PersonaLayer> {
    speaker.layers.iter().skip(1).find(|l| l.kind == kind)
}

fn push_layer_line(out: &mut String, label: &str, layer: Option<&PersonaLayer>, bodies: bool) {
    let Some(layer) = layer else { return };
    let _ = write!(out, "{label}: {}.", layer.title);
    if bodies && !layer.body.is_empty() {
        let _ = write!(out, " {}", layer.body);
    }
    out.push('\n');
}

fn push_lens_line(out: &mut String, prefix: &str, value: Option<&String>) {
    if let Some(v) = value {
        let _ = writeln!(out, "{prefix} {v}.");
    }
}

/// The persona layers and lens of a speaker, as the dialogue prompt writes them.
fn push_persona(out: &mut String, speaker: &Speaker, bodies: bool) {
    match speaker.layers.first() {
        Some(me) => {
            match &me.role {
                Some(role) => {
                    let _ = writeln!(out, "You are {}, {role}.", me.title);
                }
                None => {
                    let _ = writeln!(out, "You are {}.", me.title);
                }
            }
            if bodies && !me.body.is_empty() {
                let _ = writeln!(out, "{}", me.body);
            }
        }
        None => {
            let _ = writeln!(out, "You are {}.", speaker.id);
        }
    }
    push_layer_line(
        out,
        "Your faction",
        layer_of(speaker, HolderKind::Faction),
        bodies,
    );
    push_layer_line(
        out,
        "Your people",
        layer_of(speaker, HolderKind::People),
        bodies,
    );
    let lens = &speaker.lens;
    push_lens_line(
        out,
        "Your people see the world this way:",
        lens.thinks_from.as_ref(),
    );
    push_lens_line(out, "Your stance:", lens.stance.as_ref());
    push_lens_line(out, "You speak to:", lens.speaks_to.as_ref());
}

/// The lore block: exactly the given ids that are in the corpus, rejected ones apart.
fn push_lore(out: &mut String, fixture: &Fixture, scenario: &BtScenario, lore_ids: &[RecordId]) {
    let rejected: &[String] = fixture
        .visible
        .get(&scenario.actor)
        .map_or(&[], |e| e.rejected.as_slice());
    let is_rejected = |id: &RecordId| rejected.iter().any(|r| r == id.as_str());
    let mut seen: Vec<&RecordId> = Vec::new();
    for id in lore_ids {
        if !seen.contains(&id) {
            seen.push(id);
        }
    }

    out.push_str("\nWhat you know:\n");
    let mut any = false;
    for id in seen.iter().filter(|id| !is_rejected(id)) {
        let Some(r) = fixture.corpus.get(id) else {
            continue;
        };
        any = true;
        let _ = writeln!(out, "[{}] {}", r.id, r.title);
        if !r.body.is_empty() {
            let _ = writeln!(out, "{}", r.body);
        }
        for line in &r.voice {
            let _ = writeln!(out, "> {line}");
        }
    }
    if !any {
        out.push_str("Nothing beyond the ways of your people.\n");
    }
    let mut header = false;
    for id in seen.iter().filter(|id| is_rejected(id)) {
        let Some(r) = fixture.corpus.get(id) else {
            continue;
        };
        if !header {
            out.push_str("What you reject:\n");
            header = true;
        }
        let _ = writeln!(out, "[{}] {}", r.id, r.title);
    }
}

fn num(v: &Value) -> String {
    v.as_f64().map_or_else(|| v.to_string(), |f| f.to_string())
}

/// A parameter or field schema of the vocabulary, in words.
fn describe(v: &Value) -> String {
    let Some(obj) = v.as_object() else {
        return match v.as_str() {
            Some("nodes") => "a list of nodes".to_owned(),
            Some("node") => "a node".to_owned(),
            Some("condition name") => "a condition name from the list below".to_owned(),
            Some("condition params") => "the parameters of that condition".to_owned(),
            Some("command name") => "a command name from the list below".to_owned(),
            Some("command params") => "the parameters of that command".to_owned(),
            Some("component name") => "a component name from the list below".to_owned(),
            Some("field name") => "a field of that component".to_owned(),
            Some("per component field") => "one of that field's values".to_owned(),
            Some("event name") => "the name of one of your events".to_owned(),
            Some("per event") => "the event's payload, {} when it has none".to_owned(),
            Some(other) => other.to_owned(),
            None => v.to_string(),
        };
    };
    if let Some(values) = obj.get("enum").and_then(Value::as_array) {
        let list: Vec<String> = values.iter().map(Value::to_string).collect();
        return format!("one of {}", list.join(", "));
    }
    if obj.contains_key("x-theos-live-entity") {
        let mut s = "the handle of an entity in the situation".to_owned();
        if let Some(kind) = obj.get("x-theos-entity-kind").and_then(Value::as_str) {
            let _ = write!(s, ", of kind {kind}");
        }
        if obj.get("x-theos-allow-none") == Some(&Value::Bool(true)) {
            s.push_str(", or \"none\"");
        }
        return s;
    }
    if obj.contains_key("x-theos-scenario-flag") {
        return "the name of a flag listed in the situation".to_owned();
    }
    let kind = obj.get("type").and_then(Value::as_str).unwrap_or("value");
    let mut s = match kind {
        "number" => "a number".to_owned(),
        "integer" => "a whole number".to_owned(),
        other => format!("a {other}"),
    };
    if let (Some(lo), Some(hi)) = (obj.get("minimum"), obj.get("maximum")) {
        let _ = write!(s, " from {} to {}", num(lo), num(hi));
    }
    if let Some(unit) = obj.get("x-theos-units").and_then(Value::as_str) {
        let _ = write!(s, " ({unit})");
    }
    s
}

fn describe_fields(fields: &Value) -> String {
    fields
        .as_object()
        .map(|m| {
            m.iter()
                .map(|(k, v)| format!("{k} ({})", describe(v)))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

fn doc(v: &Value) -> &str {
    v.get("doc").and_then(Value::as_str).unwrap_or("")
}

fn push_vocabulary(out: &mut String, fixture: &Fixture, scenario: &BtScenario) {
    let vocab = &fixture.vocab;
    let limits = &vocab["limits"];
    out.push_str(
        "\nVocabulary\n\nNode types. Every node is a JSON object whose \"type\" is one of these:\n",
    );
    for name in NODE_ORDER {
        let Some(node) = vocab["node_types"].get(name) else {
            continue;
        };
        let _ = writeln!(
            out,
            "- {name}: {}. Fields: {}.",
            doc(node),
            describe_fields(&node["fields"])
        );
    }
    let _ = writeln!(
        out,
        "A tree has at most {} levels (the root is level 1) and {} nodes.\n",
        num(&limits["max_depth"]),
        num(&limits["max_nodes"])
    );
    out.push_str(TICK_RULES);
    out.push_str("\n\nConditions (Condition nodes: \"check\" and \"params\"):\n");
    if let Some(conds) = vocab["conditions"].as_object() {
        for (name, c) in conds {
            let _ = writeln!(
                out,
                "- {name}: {}. Params: {}.",
                doc(c),
                describe_fields(&c["params"])
            );
        }
    }
    out.push_str("\nCommands (Action nodes: \"command\" and \"params\"):\n");
    if let Some(cmds) = vocab["commands"].as_object() {
        for (name, c) in cmds {
            let _ = writeln!(
                out,
                "- {name}: {}. Params: {}.",
                doc(c),
                describe_fields(&c["params"])
            );
        }
    }
    out.push_str(
        "\nComponents. You have these four and no others; they make up your whole state. \
         Set them with SetComponentValue:\n",
    );
    if let Some(comps) = vocab["components"].as_object() {
        for (comp, fields) in comps {
            if let Some(fields) = fields.as_object() {
                for (field, schema) in fields {
                    let _ = writeln!(out, "- {comp}.{field}: {}", describe(schema));
                }
            }
        }
    }
    for (title, key) in [
        ("Movement modes", "modes"),
        ("Attack styles", "attack_styles"),
    ] {
        let _ = writeln!(out, "{title}:");
        if let Some(m) = vocab[key].as_object() {
            for (name, text) in m {
                let _ = writeln!(out, "- {name}: {}", text.as_str().unwrap_or(""));
            }
        }
    }
    out.push_str("\nYour events:\n");
    let own: Vec<&str> = vocab["actors"]
        .get(&scenario.actor)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if own.is_empty() {
        out.push_str("You have no events; do not use TriggerEvent.\n");
    } else {
        for name in own {
            let ev = &vocab["events"][name];
            let payload = describe_fields(&ev["payload"]);
            let payload = if payload.is_empty() {
                "none".to_owned()
            } else {
                payload
            };
            let _ = writeln!(out, "- {name}: {}. Payload: {payload}.", doc(ev));
        }
        out.push_str(
            "Fire one with the command TriggerEvent, params {\"event_name\": <name>, \"payload\": <payload>}; \
             the payload has at most one field, and is {} when the event has none.\n",
        );
    }
}

fn push_situation(out: &mut String, scenario: &BtScenario) {
    let w = &scenario.world;
    out.push_str("\nSituation, at the moment your tree is installed:\nEntities around you:\n");
    for e in &w.entities {
        let _ = writeln!(out, "{}", entity_line(e));
    }
    let _ = writeln!(out, "Your health: {} (from 0 to 1).", w.health);
    if w.flags.is_empty() {
        out.push_str("Flags set now: none.\n");
    } else {
        let _ = writeln!(out, "Flags set now: {}.", w.flags.join(", "));
    }
    if scenario.flags.is_empty() {
        out.push_str("No flag can become set; do not use flag_set.\n");
    } else {
        out.push_str(
            "Flags that may become set while the tree runs (flag_set may name only these):\n",
        );
        for (name, meaning) in &scenario.flags {
            let _ = writeln!(out, "- {name}: {meaning}");
        }
    }
}

fn push_examples(out: &mut String, fixture: &Fixture) {
    let Some(examples) = fixture.examples.as_array() else {
        return;
    };
    out.push_str("\nWorked examples (for the format only; they are not your situation):\n");
    for (i, ex) in examples.iter().enumerate() {
        let _ = writeln!(
            out,
            "\nExample {}. You are {}.",
            i + 1,
            ex["actor"].as_str().unwrap_or("an actor")
        );
        out.push_str("Entities:\n");
        let sit = &ex["situation"];
        for e in sit["entities"].as_array().into_iter().flatten() {
            if let Ok(e) = serde_json::from_value::<Entity>(e.clone()) {
                let _ = writeln!(out, "{}", entity_line(&e));
            }
        }
        let flags: Vec<&str> = sit["flags"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let _ = writeln!(
            out,
            "Health: {}. Flags set now: {}.",
            sit["health"].as_f64().unwrap_or(1.0),
            if flags.is_empty() {
                "none".to_owned()
            } else {
                flags.join(", ")
            }
        );
        let _ = writeln!(out, "Request: {}", ex["directive"].as_str().unwrap_or(""));
        let _ = writeln!(out, "Reply: {}", ex["reply"]);
    }
}

/// Renders the prompt of `scenario` with exactly the lore `lore_ids`.
///
/// NPC actors (speakers of lore-poc) get their persona layers and the lore block;
/// enemies get their design record and no lore block, whatever `lore_ids` holds.
/// `persona_bodies` switches the persona layer bodies on or off, as in dialogue.
pub fn render(
    fixture: &Fixture,
    scenario: &BtScenario,
    lore_ids: &[RecordId],
    persona_bodies: bool,
) -> Prompt {
    let mut s = String::new();
    let is_npc = scenario.is_npc(&fixture.visible);
    if is_npc {
        match fixture
            .speakers
            .iter()
            .find(|sp| sp.id.as_str() == scenario.actor)
        {
            Some(speaker) => push_persona(&mut s, speaker, persona_bodies),
            None => {
                let _ = writeln!(s, "You are {}.", scenario.actor);
            }
        }
        push_lore(&mut s, fixture, scenario, lore_ids);
    } else if let Some(rec) = fixture.enemies.get(&scenario.actor) {
        let _ = writeln!(s, "You are a {}, a creature of this world.", rec.title);
        let _ = writeln!(
            s,
            "Your design record, which is how you behave:\n{}",
            rec.body
        );
    } else {
        let _ = writeln!(s, "You are {}.", scenario.actor);
    }

    s.push_str("\nTask: the user turn holds a request. Write a behaviour tree, a small program your body runs every half second, that does what the request asks in the situation below.\n");
    s.push_str("- Use only the entity handles, flags, components, conditions, commands and events listed here. Invent none.\n");
    s.push_str("- If the request asks for something this vocabulary cannot do, do the nearest thing it can.\n");
    if is_npc {
        s.push_str("- You know only what is written under \"What you know\" and \"What you reject\", and your craft and ways follow from it. Where the request goes against what you know, act as you would. You know the claims under \"What you reject\" and you act against them.\n");
        s.push_str("- Never write the bracketed record ids anywhere but in \"sources\".\n");
    } else {
        s.push_str("- Your design record is how you behave. If the request contradicts it, follow the record.\n");
    }

    push_situation(&mut s, scenario);
    push_vocabulary(&mut s, fixture, scenario);
    push_examples(&mut s, fixture);

    s.push_str("\nReply with one JSON object and nothing else: first \"sources\", the ids of the lore records you draw on (an empty list if none), then \"tree\", with \"name\" (a short name for the tree) and \"root\" (the root node).\n");

    let user = if scenario.directive.is_empty() {
        NO_DIRECTIVE.to_owned()
    } else {
        scenario.directive.clone()
    };
    Prompt { system: s, user }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::tests::load_default;

    fn scenario<'a>(fx: &'a Fixture, id: &str) -> &'a BtScenario {
        fx.scenarios.iter().find(|s| s.id == id).unwrap()
    }

    fn all_visible(fx: &Fixture, actor: &str) -> Vec<RecordId> {
        let e = &fx.visible[actor];
        e.held
            .iter()
            .chain(&e.rejected)
            .map(RecordId::new)
            .collect()
    }

    #[test]
    fn entity_line_includes_name_only_when_present() {
        let fx = load_default();
        let b06 = scenario(&fx, "b06");
        let p = render(&fx, b06, &all_visible(&fx, &b06.actor), true);
        assert!(p
            .system
            .contains("\n- vareth: spirit, 400 m, called \"Vareth the Long Cold\"\n"));
        let b14 = scenario(&fx, "b14");
        let p = render(&fx, b14, &[], true);
        assert!(p.system.contains("\n- player: player, 50 m\n"));
        assert!(p.system.contains("\n- warden-1: ally, 4 m\n"));
    }

    #[test]
    fn enemy_prompt_holds_the_record_body_and_no_lore_block() {
        let fx = load_default();
        let b13 = scenario(&fx, "b13");
        let p = render(&fx, b13, &[], true);
        assert!(p.system.contains(&fx.enemies["fx-reed-lurker"].body));
        assert!(p.system.contains("Reed lurker"));
        assert!(!p.system.contains("What you know"));
        assert!(!p.system.contains("What you reject"));
    }

    #[test]
    fn npc_prompt_lore_block_holds_exactly_the_given_ids() {
        let fx = load_default();
        let b06 = scenario(&fx, "b06");
        let entry = &fx.visible[&b06.actor];
        let given = vec![RecordId::new("B-947")];
        let p = render(&fx, b06, &given, true);
        assert!(p.system.contains("What you know:\n[B-947] "));
        for id in entry.held.iter().chain(&entry.rejected) {
            if id != "B-947" {
                assert!(
                    !p.system.contains(&format!("[{id}]")),
                    "{id} must not be in the prompt"
                );
            }
        }
        let none = render(&fx, b06, &[], true);
        assert!(!none.system.contains("[B-947]"));
    }

    #[test]
    fn npc_prompt_renders_rejected_ids_under_what_you_reject() {
        let fx = load_default();
        let b03 = scenario(&fx, "b03");
        let entry = &fx.visible[&b03.actor];
        let rejected = entry
            .rejected
            .first()
            .expect("b03's actor rejects a belief");
        let p = render(&fx, b03, &[RecordId::new(rejected)], true);
        assert!(p
            .system
            .contains(&format!("What you reject:\n[{rejected}] ")));
    }

    #[test]
    fn directive_is_the_user_part() {
        let fx = load_default();
        let b06 = scenario(&fx, "b06");
        let p = render(&fx, b06, &[], true);
        assert_eq!(p.user, b06.directive);
        assert!(!p.system.contains(&b06.directive));
        let mut silent = scenario(&fx, "b20").clone();
        silent.directive.clear();
        let p = render(&fx, &silent, &[], true);
        assert_eq!(p.user, "Act on your record and the situation.");
    }

    #[test]
    fn prompt_lists_only_the_actors_own_events() {
        let fx = load_default();
        let p = render(&fx, scenario(&fx, "b04"), &[], true);
        assert!(p.system.contains("Chant"));
        assert!(p.system.contains("Ground-Ohm"));
        for other in [
            "CarveRune",
            "BindSpirit",
            "RingBell",
            "CallAllies",
            "ReadPlate",
        ] {
            assert!(!p.system.contains(other), "{other} leaked into b04");
        }
        let p = render(&fx, scenario(&fx, "b10"), &[], true);
        assert!(p.system.contains("You have no events"));
        assert!(!p.system.contains("Chant"));
    }

    #[test]
    fn prompt_holds_flags_health_and_vocabulary() {
        let fx = load_default();
        let p = render(&fx, scenario(&fx, "b01"), &[], true);
        assert!(p.system.contains("threatened: an armed stranger"));
        assert!(p.system.contains("Your health: 1"));
        assert!(p.system.contains("Decorator_Timeout"));
        assert!(p.system.contains("allies_in_range"));
        assert!(p.system.contains("MovementState.mode"));
        assert!(p.system.contains("\"sources\""));
        assert!(p.system.contains("well-watch"));
    }
}
