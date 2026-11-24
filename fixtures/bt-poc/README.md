# bt-poc fixture

Situations and directives for the second half of the local-LLM proof of concept: the model writes a behaviour tree for an NPC or an enemy, and a small simulator checks what the tree does.

**Synthetic, not canon.** The enemies, the situations and the event names are invented for the test. The NPCs and their beliefs come from `fixtures/lore-poc`.

## Layout

| Path | What it is |
|---|---|
| `scenarios.toml` | 20 scenarios: actor, situation, directive, scripted trace, checks. |
| `vocabulary.json` | What a tree may use: node types, conditions, commands, components, events per actor, limits. |
| `records/enemies/` | Four synthetic enemy records in schema v1. Each body is the design the tree must follow. |
| `prompt-examples.json` | Two worked examples for the prompt, as the BT DSL page asks (§6). They are not in any scenario. |
| `reference/` | A good and a bad tree per scenario, plus ten trees that break one gate rule each. |
| `expected/reference.json` | What `check.py` makes of every reference tree, including the tick-by-tick timeline. A golden file for the Rust tree runtime. |
| `check.py` | The reference evaluator (Python 3.11, standard library). It is the executable form of this README. |

NPC actors (b01–b09) are speakers of `fixtures/lore-poc`; their lore comes from there, by the same speaker rule. Enemy actors (b10–b20) have no lore; their record body is in every prompt.

## A request

The prompt holds:

- the actor: the persona layers for an NPC, or the enemy record's title and body;
- lore, for NPCs only, by scope (none, all visible, retrieved with the directive as the query);
- the situation at install time: the entities with handle, kind, name and distance; the actor's health; the flags that are set now; the flags that may become set, with their meaning;
- the directive, in prose;
- the vocabulary: node types and how a tick runs, the conditions, the components with their values, and the actor's events;
- the two worked examples.

The reply is one JSON object. `sources` comes first, as in the lore PoC, so the model commits to its records before it builds:

```json
{
  "sources": ["B-918"],
  "tree": { "name": "calm-carving", "root": { "type": "Selector", "children": [ … ] } }
}
```

`tree` is the `BehaviorTreeDef` of the BT DSL page (140083201): a `name` and a `root` node, every node tagged by `type`.

## Vocabulary

From the BT DSL page:

- **Node types:** Sequence, Selector, Parallel, Decorator_Repeat, Decorator_Invert, Decorator_Timeout, Condition, Action, Wait. Parallel's policy is `all` or `one`; the page leaves it open.
- **Conditions:** `health_below`, `entity_in_range`, `flag_set`.
- **Commands:** `SetComponentValue`, `TriggerEvent`.

Added by the fixture:

- **Conditions:** `allies_in_range` (pack and squad rules) and `time_since_install_over` (timers, see b19).
- **Components:** every actor has the same four, which make up its whole state:

  | Field | Values |
  |---|---|
  | `MovementState.mode` | Idle, Patrol, Hold, Advance, Charge, Kite, Flee, TakeCover, Regroup, Follow |
  | `Target.entity` | an entity handle, or `none` |
  | `Destination.entity` | an entity handle, or `none` |
  | `Attack.style` | None, Melee, Ranged, Heavy |

- **Events:** each actor may fire only its own (`vocabulary.json`, `actors`). Payloads have at most one field.
- **Parameter schemas** in the shape that `parameter_constraints()` emits, with `x-theos-*` keywords. `x-theos-entity-kind`, `x-theos-allow-none` and `x-theos-scenario-flag` are fixture additions.

Left out: `has_component` (every actor has the same components), `random_chance` (scoring must be deterministic), and `SpawnEntity`, `DestroyEntity` and `UpdateBehaviorTree` (no scenario needs them yet).

## How a tick runs

The page does not define tick semantics, so the fixture does. `check.py` implements exactly this.

- The tree is ticked every 0.5 s, from t = 0 to the scenario's `duration`, both included.
- Before each tick, every trace step with `at <= t` that has not run yet is applied, in order.
- Each tick evaluates the root once. A node returns success, failure or running.
- **Sequence** runs children in order: failure stops it with failure, running stops it with running, and all successes give success. **Selector** is the mirror image: success stops it, and all failures give failure.
- **Memory.** A Sequence or Selector that returned running resumes at that child on the next tick; earlier children are not run again until the composite finishes. A tree with no Wait finishes every tick, so it re-runs from the root each tick and reacts at once.
- **Parallel** ticks every child each tick. `all` fails on any failure and succeeds when all succeed; `one` succeeds on any success and fails when all fail. Otherwise it is running.
- **Condition** succeeds when its check is true.
- **Action** applies its effect and succeeds. SetComponentValue sets a field; TriggerEvent adds the event to this tick's log. The scripted world does not react to actions.
- **Wait** is running until `seconds` have passed since it started, then succeeds.
- **Decorator_Invert** swaps success and failure. **Decorator_Timeout** fails, and resets its child, when the child is still running `seconds` after the decorator started. **Decorator_Repeat** runs its child `count` times (at most 10), within the same tick if the child finishes at once.
- **Condition rules:**
  - `health_below` is strict: health below the threshold.
  - `entity_in_range` is false for a removed entity.
  - `allies_in_range` counts entities of kind `ally`, never the actor itself.
  - `time_since_install_over` is true from `t >= seconds`.

## Scenarios

| Id | Actor | What it tests |
|---|---|---|
| b01 | hedge-Scribe | Carve whenever you can, but a Scribe carves only when calm (B-918). |
| b02 | hedge-Scribe | Wake a glyph: the Breath-Test comes first (B-919). |
| b03 | academy-Scribe | The same directive; the measuring-cord, never the Breath-Test (B-920; rejects B-919). |
| b04 | Cantor | A working's order: Ground-Ohm, Kharrian's Stride for distance, Severing Sa (B-936–938). |
| b05 | Cantor | The directive asks for a chant in a loop; the songbooks forbid it (B-940). |
| b06 | binder | Teach an apprentice: Grandfather Ember, never Vareth (B-947). |
| b07 | sect elder | Strongest blow, but never from a gate at full ebb (B-929). |
| b08 | plate reader | A vague directive ("what your people came for"); hide when the patrol returns. |
| b09 | shrine keeper | Bell and shelter. No lore applies; a good tree cites nothing. |
| b10 | thorn archer | Keep the distance, never melee. |
| b11 | stone brute | Never retreat; heavy blows below a third. 0.45 at 1.5 s catches a threshold of one half. |
| b12 | reed lurker | Wait for three of the pack; at 1.5 s only two are close. |
| b13 | reed lurker | Slink back when badly hurt; 0.7 is not badly hurt, 0.2 is. |
| b14 | warden captain | Call, hold the bridge, no chase, regroup when one warden is left. |
| b15 | warden captain | The directive says chase; the record says never. The record wins. |
| b16 | thorn archer | Out of arrows: fall back to the captain. |
| b17 | thorn archer | Told to charge and melee; the record forbids both. |
| b18 | stone brute | Told to cast a fireball, which does not exist. |
| b19 | reed lurker | Ambush with a 4 s timer. |
| b20 | thorn archer | No directive: the record alone. |

Fields of a scenario:

| Field | Meaning |
|---|---|
| `actor` | An `npc` of lore-poc or an `enemy` of `records/`. |
| `tags` | `lore`, `lore-irrelevant`, `combat`, `robustness`. |
| `priority` | `WorldEvent` (30 s to generate) or `CombatTactical` (8 s), from the AI Latency Budget page. |
| `duration` | Seconds the tree runs. |
| `directive` | What the director asks for, in prose. |
| `lore_expect`, `also_ok`, `rejects` | As in lore-poc: the beliefs a good tree rests on, may also cite, or argues against. |
| `trace` | Scripted changes, applied at `at`: `health`, `distances`, `set_flags`, `clear_flags`, `remove`. |
| `world` | The situation at t = 0: `health`, `flags` set now, `entities`. |
| `flags` | The flags that can be set, with the meaning the prompt shows. `flag_set` may name only these. |
| `check` | What the tree must do (below). |

## Checks

A check covers ticks with `from <= t <= to`; both are optional and default to the whole run. In `state`, a list means any of its values.

| Kind | Passes when |
|---|---|
| `state` | Some tick in the window has every listed field. |
| `never` | No tick in the window has every listed field. |
| `always` | Every tick in the window has every listed field. Not used yet. |
| `event` | The event fires in the window. `Chant:Ground-Ohm` matches that payload; `ReadPlate` matches any. |
| `no_event` | The event does not fire in the window. |
| `order` | Every listed event fires, and their first firings come in this order. |
| `structure` | A static rule on the tree. `no-repeat-over-event`: no Decorator_Repeat contains the named event. |

## The gate

The ValidationGate of the BT DSL page, in this order; the first broken rule fails the tree:

1. `shape`: the envelope and every node have exactly their fields.
2. `max-depth`: at most 10 levels; the root is level 1.
3. `max-nodes`: at most 50 nodes.
4. `registered-conditions`
5. `registered-commands`
6. `params`: every parameter against its schema, including live entities, entity kinds, flags of the situation and the actor's own events.
7. `sources`: only lore that can be in this actor's prompt. It is checked here against `lore-poc/expected/visible.json`; the harness checks it against the actual prompt.

The page's rule 6 (nested `UpdateBehaviorTree`) does not apply, because the command is left out.

## Scoring

Per request:

| Measure | Meaning |
|---|---|
| gate | Passes, or the first rule it broke. |
| behaviour | Every check passes. Report the share of checks too. |
| grounding | NPC scenarios only, as in lore-poc: at least one `lore_expect` id and nothing outside `lore_expect`, `also_ok` and `rejects`. With an empty `lore_expect`, only `also_ok` ids or none. |
| leak | NPC scenarios only: every lore record in the prompt is visible to the actor. |
| budget | Generation time against the priority's budget. |
| size | Nodes, depth, prompt and output tokens. |

## Run conditions

| Condition | Values |
|---|---|
| Grammar | On: the reply is constrained by a JSON schema built per request (entity handles, flags, events and lore ids as enums). Off: the prompt alone. |
| Lore scope | NPC scenarios: none, all visible, retrieved. Enemies: none only. |

That is 9 × 3 × 2 + 11 × 2 = 76 requests per model, with thinking off. The grammar-off column measures the page's acceptance criterion: at least 80 % of 20 generated trees pass the gate. With the grammar on, most gate rules cannot be broken; the measure that matters then is behaviour.

**Grammar notes.**

- llguidance supports `$ref`, `anyOf`, `enum` and `const`, so the recursive node schema can be expressed. Its docs say nothing about recursion depth; the spike should confirm it.
- It emits properties in the order of `properties`, so `sources` comes before `tree`.
- The depth and node limits are not expressible in JSON Schema. Unroll the node definition ten levels, or leave them to the gate.
- Raise the token cap above the reply cap of the lore PoC: a 15-node tree is a few hundred tokens.

## check.py

```text
python check.py                      check the fixture: data integrity, every reference tree, the golden file
python check.py --write              rewrite expected/reference.json after an intended change
python check.py --score b07 r.json   score one reply envelope for one scenario
```

Every good reference tree passes the gate and every check. Every bad tree breaks exactly the checks its file lists, or the gate rule it names, so each check is shown to catch something.

## What the fixture invents

- The four enemies and their design records.
- The added conditions, components, modes, attack styles and events.
- The tick semantics, including memory in Sequence and Selector.
- The `at_ebb` flag (b07), a stand-in for the planned Yin-Yang balance of the martial arts.
- The links from beliefs to behaviour. For example, B-918 says a Scribe carves only when calm; b01 turns that into "no CarveRune while threatened".

## Checks run

- `check.py`: data integrity, 20 good, 20 bad and 10 gate trees, all as intended.
- Twelve alternative trees scored by hand. Fair variants pass, such as fleeing instead of taking cover, a third as the threshold, a 10 m kite radius, or a 3.5 s timer. Wrong ones fail, such as a half as the threshold, a pack of one, or a 5 s timer.
- The content repo's checker: the four enemy records pass.
- The editor's `theos-content` (commit `0a6f480`) does not know the `enemy` kind yet and reports `schema/kind` for all four. The harness should read these four files with a plain front-matter parser.
