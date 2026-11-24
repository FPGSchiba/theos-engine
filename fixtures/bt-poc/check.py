"""Reference evaluator of the bt-poc fixture. Python 3.11+, standard library only.

It is the executable form of the rules in README.md: the gate, the tick semantics and the
checks. Use it to verify the fixture after an edit, and to cross-check the Rust harness.

    python check.py                      verify every reference tree against expected/reference.json
    python check.py --write              rewrite expected/reference.json
    python check.py --score b07 reply.json   score one reply envelope for one scenario
"""
import argparse
import json
import sys
import tomllib
from pathlib import Path

HERE = Path(__file__).resolve().parent
NODE_FIELDS = {
    "Sequence": {"children"}, "Selector": {"children"}, "Parallel": {"children", "policy"},
    "Decorator_Repeat": {"child", "count"}, "Decorator_Invert": {"child"},
    "Decorator_Timeout": {"child", "seconds"}, "Condition": {"check", "params"},
    "Action": {"command", "params"}, "Wait": {"seconds"},
}
STATE_FIELDS = ["MovementState.mode", "Target.entity", "Destination.entity", "Attack.style"]
INITIAL_STATE = {"MovementState.mode": "Idle", "Target.entity": "none",
                 "Destination.entity": "none", "Attack.style": "None"}


class GateError(Exception):
    def __init__(self, rule, message):
        super().__init__(f"{rule}: {message}")
        self.rule = rule
        self.message = message


def load():
    vocab = json.loads((HERE / "vocabulary.json").read_text(encoding="utf-8"))
    scen = tomllib.loads((HERE / "scenarios.toml").read_text(encoding="utf-8"))
    visible_path = HERE.parent / "lore-poc" / "expected" / "visible.json"
    visible = json.loads(visible_path.read_text(encoding="utf-8"))
    return vocab, {s["id"]: s for s in scen["scenario"]}, visible


def lore_in_prompt(scenario, visible):
    """The lore ids a prompt may hold for this actor: its held and rejected beliefs."""
    entry = visible.get(scenario["actor"])
    return set(entry["held"]) | set(entry["rejected"]) if entry else set()


# ------------------------------------------------------------------ gate
def walk(node, depth=1):
    yield node, depth
    for child in node.get("children", []) if isinstance(node.get("children"), list) else []:
        yield from walk(child, depth + 1)
    if isinstance(node.get("child"), dict):
        yield from walk(node["child"], depth + 1)


def check_shape(node):
    if not isinstance(node, dict) or node.get("type") not in NODE_FIELDS:
        raise GateError("shape", f"not a known node: {json.dumps(node)[:80]}")
    fields = set(node) - {"type"}
    if fields != NODE_FIELDS[node["type"]]:
        raise GateError("shape", f"{node['type']} needs exactly {sorted(NODE_FIELDS[node['type']])}, has {sorted(fields)}")
    if "children" in node:
        if not isinstance(node["children"], list) or not node["children"]:
            raise GateError("shape", f"{node['type']} needs a non-empty list of children")
        for child in node["children"]:
            check_shape(child)
    if "child" in node:
        check_shape(node["child"])
    if "params" in node and not isinstance(node["params"], dict):
        raise GateError("shape", f"{node['type']} params must be an object")


def check_value(schema, value, ctx, where):
    def bad(msg):
        raise GateError("params", f"{where}: {msg}")

    kind = schema.get("type")
    if kind == "number" and (isinstance(value, bool) or not isinstance(value, (int, float))):
        bad(f"{value!r} is not a number")
    if kind == "integer" and (isinstance(value, bool) or not isinstance(value, int)):
        bad(f"{value!r} is not an integer")
    if kind == "string" and not isinstance(value, str):
        bad(f"{value!r} is not a string")
    if "minimum" in schema and value < schema["minimum"]:
        bad(f"{value} is below {schema['minimum']}")
    if "maximum" in schema and value > schema["maximum"]:
        bad(f"{value} is above {schema['maximum']}")
    if "enum" in schema and value not in schema["enum"]:
        bad(f"{value!r} is not one of {schema['enum']}")
    if schema.get("x-theos-live-entity"):
        if value == "none" and schema.get("x-theos-allow-none"):
            return
        if value not in ctx["entities"]:
            bad(f"{value!r} is not an entity of the situation")
        want = schema.get("x-theos-entity-kind")
        if want and ctx["entities"][value] != want:
            bad(f"{value!r} is a {ctx['entities'][value]}, not a {want}")
    if schema.get("x-theos-scenario-flag") and value not in ctx["flags"]:
        bad(f"{value!r} is not a flag of the situation")


def check_params(schema_map, params, ctx, where):
    if set(params) != set(schema_map):
        raise GateError("params", f"{where}: needs exactly {sorted(schema_map)}, has {sorted(params)}")
    for key, schema in schema_map.items():
        check_value(schema, params[key], ctx, f"{where}.{key}")


def gate(envelope, scenario, vocab, visible):
    """Raises GateError on the first broken rule; returns (nodes, depth)."""
    if not isinstance(envelope, dict) or set(envelope) != {"sources", "tree"}:
        raise GateError("shape", "the reply must be an object with exactly sources and tree")
    tree = envelope["tree"]
    if not isinstance(tree, dict) or set(tree) != {"name", "root"} or not isinstance(tree["name"], str):
        raise GateError("shape", "tree must be an object with exactly name and root")
    root = tree["root"]
    check_shape(root)
    nodes = list(walk(root))
    depth = max(d for _, d in nodes)
    limits = vocab["limits"]
    if depth > limits["max_depth"]:
        raise GateError("max-depth", f"depth {depth} exceeds {limits['max_depth']}")
    if len(nodes) > limits["max_nodes"]:
        raise GateError("max-nodes", f"{len(nodes)} nodes exceed {limits['max_nodes']}")
    for node, _ in nodes:
        if node["type"] == "Condition" and node["check"] not in vocab["conditions"]:
            raise GateError("registered-conditions", f"{node['check']!r} is not a registered condition")
    for node, _ in nodes:
        if node["type"] == "Action" and node["command"] not in vocab["commands"]:
            raise GateError("registered-commands", f"{node['command']!r} is not a registered command")
    ctx = {"entities": {e["handle"]: e["kind"] for e in scenario["world"]["entities"]},
           "flags": set(scenario.get("flags", {})) | set(scenario["world"].get("flags", []))}
    allowed_events = vocab["actors"][scenario["actor"]]
    for node, _ in nodes:
        kind = node["type"]
        if kind in vocab["node_types"]:
            for field, schema in vocab["node_types"][kind]["fields"].items():
                if isinstance(schema, dict):
                    check_value(schema, node[field], ctx, f"{kind}.{field}")
        if kind == "Condition":
            check_params(vocab["conditions"][node["check"]]["params"], node["params"], ctx, node["check"])
        elif kind == "Action" and node["command"] == "SetComponentValue":
            p = node["params"]
            if set(p) != {"component", "field", "value"}:
                raise GateError("params", f"SetComponentValue needs component, field, value; has {sorted(p)}")
            comp = vocab["components"].get(p["component"])
            if comp is None or p["field"] not in comp:
                raise GateError("params", f"{p['component']}.{p['field']} is not a component field")
            check_value(comp[p["field"]], p["value"], ctx, f"{p['component']}.{p['field']}")
        elif kind == "Action" and node["command"] == "TriggerEvent":
            p = node["params"]
            if set(p) != {"event_name", "payload"} or not isinstance(p["payload"], dict):
                raise GateError("params", f"TriggerEvent needs event_name and payload; has {sorted(p)}")
            if p["event_name"] not in allowed_events:
                raise GateError("params", f"{p['event_name']!r} is not an event of {scenario['actor']}")
            check_params(vocab["events"][p["event_name"]]["payload"], p["payload"], ctx, p["event_name"])
    sources = envelope["sources"]
    if not isinstance(sources, list) or not all(isinstance(s, str) for s in sources):
        raise GateError("shape", "sources must be a list of ids")
    outside = [s for s in sources if s not in lore_in_prompt(scenario, visible)]
    if outside:
        raise GateError("sources", f"cites {outside}, which a prompt for {scenario['actor']} cannot hold")
    return len(nodes), depth


# ------------------------------------------------------------------ simulation
def event_key(params):
    payload = params["payload"]
    if len(payload) == 1:
        return f"{params['event_name']}:{next(iter(payload.values()))}"
    return params["event_name"]


class Sim:
    def __init__(self, scenario, tick):
        self.scenario = scenario
        self.tick_s = tick
        world = scenario["world"]
        self.health = world.get("health", 1.0)
        self.flags = set(world.get("flags", []))
        self.dist = {e["handle"]: e["distance"] for e in world["entities"]}
        self.kind = {e["handle"]: e["kind"] for e in world["entities"]}
        self.state = dict(INITIAL_STATE)
        self.mem = {}
        self.t = 0.0
        self.fired = []

    def apply(self, step):
        if "health" in step:
            self.health = step["health"]
        for handle, d in step.get("distances", {}).items():
            self.dist[handle] = d
        for f in step.get("set_flags", []):
            self.flags.add(f)
        for f in step.get("clear_flags", []):
            self.flags.discard(f)
        for handle in step.get("remove", []):
            self.dist.pop(handle, None)

    def reset(self, path):
        for key in [k for k in self.mem if k[:len(path)] == path]:
            del self.mem[key]

    def condition(self, check, p):
        if check == "health_below":
            return self.health < p["threshold"]
        if check == "entity_in_range":
            return p["target_entity"] in self.dist and self.dist[p["target_entity"]] <= p["radius"]
        if check == "flag_set":
            return p["flag_name"] in self.flags
        if check == "allies_in_range":
            n = sum(1 for h, d in self.dist.items() if self.kind[h] == "ally" and d <= p["radius"])
            return n >= p["at_least"]
        if check == "time_since_install_over":
            return self.t >= p["seconds"]
        raise ValueError(check)

    def run(self, node, path):
        kind = node["type"]
        if kind == "Condition":
            return "success" if self.condition(node["check"], node["params"]) else "failure"
        if kind == "Action":
            p = node["params"]
            if node["command"] == "SetComponentValue":
                self.state[f"{p['component']}.{p['field']}"] = p["value"]
            else:
                self.fired.append(event_key(p))
            return "success"
        if kind == "Wait":
            start = self.mem.setdefault(path, {"start": self.t})["start"]
            if self.t - start >= node["seconds"]:
                del self.mem[path]
                return "success"
            return "running"
        if kind in ("Sequence", "Selector"):
            stop_on = "failure" if kind == "Sequence" else "success"
            start = self.mem.pop(path, {}).get("i", 0)
            for i in range(start, len(node["children"])):
                r = self.run(node["children"][i], path + (i,))
                if r == "running":
                    self.mem[path] = {"i": i}
                    return "running"
                if r == stop_on:
                    return r
            return "success" if kind == "Sequence" else "failure"
        if kind == "Parallel":
            results = [self.run(c, path + (i,)) for i, c in enumerate(node["children"])]
            if node["policy"] == "all":
                out = "failure" if "failure" in results else "success" if all(r == "success" for r in results) else "running"
            else:
                out = "success" if "success" in results else "failure" if all(r == "failure" for r in results) else "running"
            if out != "running":
                self.reset(path)
            return out
        if kind == "Decorator_Invert":
            r = self.run(node["child"], path + (0,))
            return {"success": "failure", "failure": "success"}.get(r, r)
        if kind == "Decorator_Timeout":
            start = self.mem.setdefault(path, {"start": self.t})["start"]
            r = self.run(node["child"], path + (0,))
            if r == "running" and self.t - start >= node["seconds"]:
                self.reset(path)
                return "failure"
            if r != "running":
                self.mem.pop(path, None)
            return r
        if kind == "Decorator_Repeat":
            done = self.mem.pop(path, {}).get("i", 0)
            while done < node["count"]:
                r = self.run(node["child"], path + (0,))
                if r == "running":
                    self.mem[path] = {"i": done}
                    return "running"
                if r == "failure":
                    return "failure"
                done += 1
            return "success"
        raise ValueError(kind)

    def timeline(self, root):
        steps = sorted(self.scenario.get("trace", []), key=lambda s: s["at"])
        out, applied = [], 0
        n_ticks = int(round(self.scenario["duration"] / self.tick_s))
        for i in range(n_ticks + 1):
            self.t = i * self.tick_s
            while applied < len(steps) and steps[applied]["at"] <= self.t:
                self.apply(steps[applied])
                applied += 1
            self.fired = []
            status = self.run(root, ())
            out.append({"t": self.t, "status": status, "state": dict(self.state), "events": list(self.fired)})
        return out


# ------------------------------------------------------------------ checks
def matches(state, want):
    for field, value in want.items():
        allowed = value if isinstance(value, list) else [value]
        if state[field] not in allowed:
            return False
    return True


def event_matches(key, pattern):
    return key == pattern or (":" not in pattern and key.split(":", 1)[0] == pattern)


def has_repeat_over_event(node, event, inside=False):
    if node["type"] == "Action" and node["command"] == "TriggerEvent":
        return inside and node["params"].get("event_name") == event
    inside = inside or node["type"] == "Decorator_Repeat"
    kids = node.get("children", []) + ([node["child"]] if "child" in node else [])
    return any(has_repeat_over_event(k, event, inside) for k in kids)


def evaluate(check, timeline, root):
    lo = check.get("from", 0.0)
    hi = check.get("to", timeline[-1]["t"])
    window = [row for row in timeline if lo <= row["t"] <= hi]
    kind = check["kind"]
    if kind == "state":
        return any(matches(r["state"], check["state"]) for r in window)
    if kind == "never":
        return not any(matches(r["state"], check["state"]) for r in window)
    if kind == "always":
        return all(matches(r["state"], check["state"]) for r in window)
    if kind == "event":
        return any(event_matches(k, check["event"]) for r in window for k in r["events"])
    if kind == "no_event":
        return not any(event_matches(k, check["event"]) for r in window for k in r["events"])
    if kind == "order":
        log = [(i, j, k) for i, r in enumerate(window) for j, k in enumerate(r["events"])]
        firsts = []
        for pattern in check["order"]:
            hit = next(((i, j) for i, j, k in log if event_matches(k, pattern)), None)
            if hit is None:
                return False
            firsts.append(hit)
        return all(a < b for a, b in zip(firsts, firsts[1:]))
    if kind == "structure":
        if check["rule"] == "no-repeat-over-event":
            return not has_repeat_over_event(root, check["event"])
        raise ValueError(check["rule"])
    raise ValueError(kind)


def grounding(envelope, scenario):
    """Does the tree cite the lore it should? None for actors without lore."""
    if not scenario["actor_has_lore"]:
        return None
    cited = set(envelope["sources"])
    allowed = set(scenario["lore_expect"]) | set(scenario["also_ok"]) | set(scenario["rejects"])
    if not cited <= allowed:
        return False
    return bool(cited & set(scenario["lore_expect"])) if scenario["lore_expect"] else True


def score(envelope, scenario, vocab, visible):
    out = {"gate": "pass", "gate_rule": None, "gate_message": None, "nodes": None, "depth": None,
           "checks": [], "behaviour": None, "grounding": None, "timeline": None}
    try:
        out["nodes"], out["depth"] = gate(envelope, scenario, vocab, visible)
    except GateError as e:
        out.update(gate="fail", gate_rule=e.rule, gate_message=e.message)
        return out
    root = envelope["tree"]["root"]
    timeline = Sim(scenario, vocab["tick_seconds"]).timeline(root)
    out["checks"] = [evaluate(c, timeline, root) for c in scenario["check"]]
    out["behaviour"] = all(out["checks"])
    out["grounding"] = grounding(envelope, scenario)
    out["timeline"] = timeline
    return out


# ------------------------------------------------------------------ fixture integrity
def integrity(vocab, scenarios, visible):
    """Problems in the fixture data itself, before any tree is scored."""
    problems = []
    fields = {f"{c}.{f}": s for c, comp in vocab["components"].items() for f, s in comp.items()}
    for sid, s in scenarios.items():
        actor = s["actor"]
        if actor not in vocab["actors"]:
            problems.append(f"{sid}: unknown actor {actor}")
            continue
        handles = {e["handle"]: e["kind"] for e in s["world"]["entities"]}
        flags = set(s.get("flags", {}))
        for e in s["world"]["entities"]:
            if e["kind"] not in ("player", "ally", "foe", "cover", "place", "spirit", "plate"):
                problems.append(f"{sid}: entity {e['handle']} has kind {e['kind']}")
        for f in s["world"].get("flags", []):
            if f not in flags:
                problems.append(f"{sid}: initial flag {f} is not described in [scenario.flags]")
        for step in s.get("trace", []):
            for h in list(step.get("distances", {})) + step.get("remove", []):
                if h not in handles:
                    problems.append(f"{sid}: trace names unknown entity {h}")
            for f in step.get("set_flags", []) + step.get("clear_flags", []):
                if f not in flags:
                    problems.append(f"{sid}: trace names unknown flag {f}")
            if not 0 <= step["at"] <= s["duration"]:
                problems.append(f"{sid}: trace step at {step['at']} is outside the run")
        events = vocab["actors"][actor]
        for i, c in enumerate(s["check"]):
            for field, value in c.get("state", {}).items():
                if field not in fields:
                    problems.append(f"{sid} check {i}: unknown field {field}")
                    continue
                for v in value if isinstance(value, list) else [value]:
                    schema = fields[field]
                    ok = v in schema["enum"] if "enum" in schema else (v == "none" or v in handles)
                    if not ok:
                        problems.append(f"{sid} check {i}: {field} = {v!r} is not a possible value")
            for pattern in ([c["event"]] if "event" in c else []) + c.get("order", []):
                if pattern.split(":", 1)[0] not in events:
                    problems.append(f"{sid} check {i}: {pattern} is not an event of {actor}")
        lore = s["lore_expect"] + s["also_ok"] + s["rejects"]
        entry = visible.get(actor)
        if entry is None:
            if lore:
                problems.append(f"{sid}: lore ids for an actor without lore")
            continue
        for rid in s["lore_expect"] + s["also_ok"]:
            if rid not in entry["held"]:
                problems.append(f"{sid}: {rid} is not held by {actor}")
        for rid in s["rejects"]:
            if rid not in entry["rejected"]:
                problems.append(f"{sid}: {rid} is not rejected by {actor}")
    return problems


# ------------------------------------------------------------------ CLI
def reference_results(vocab, scenarios, visible):
    results, problems = {}, []
    for path in sorted((HERE / "reference").glob("*.json")):
        ref = json.loads(path.read_text(encoding="utf-8"))
        scenario = scenarios[ref["scenario"]]
        res = score(ref["reply"], scenario, vocab, visible)
        results[path.stem] = res
        want = ref["expect"]
        if want == "pass":
            if res["gate"] != "pass" or not res["behaviour"] or res["grounding"] is False:
                problems.append(f"{path.name}: should pass, got gate={res['gate_rule'] or 'pass'} "
                                f"checks={res['checks']} grounding={res['grounding']}")
        elif want.startswith("gate:"):
            if res["gate_rule"] != want[5:]:
                problems.append(f"{path.name}: should fail gate rule {want[5:]}, got {res['gate_rule']} "
                                f"({res['gate_message']})")
        else:
            broken = [i for i, ok in enumerate(res["checks"]) if not ok]
            expected = ref["breaks"]
            if res["gate"] != "pass" or broken != expected:
                problems.append(f"{path.name}: should break checks {expected}, broke {broken} "
                                f"(gate {res['gate_rule'] or 'pass'})")
    return results, problems


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--write", action="store_true")
    ap.add_argument("--score", nargs=2, metavar=("SCENARIO", "REPLY_JSON"))
    args = ap.parse_args()
    vocab, scenarios, visible = load()
    for s in scenarios.values():
        s["actor_has_lore"] = s["actor"] in visible
    if args.score:
        sid, reply = args.score
        res = score(json.loads(Path(reply).read_text(encoding="utf-8")), scenarios[sid], vocab, visible)
        res.pop("timeline")
        print(json.dumps(res, indent=2))
        return 0
    problems = integrity(vocab, scenarios, visible)
    results, ref_problems = reference_results(vocab, scenarios, visible)
    problems += ref_problems
    golden = HERE / "expected" / "reference.json"
    text = json.dumps(results, indent=1, ensure_ascii=False) + "\n"
    if args.write:
        golden.parent.mkdir(exist_ok=True)
        golden.write_text(text, encoding="utf-8", newline="\n")
    elif golden.read_text(encoding="utf-8") != text:
        problems.append("expected/reference.json differs from a fresh run; rerun with --write if the change is intended")
    print(f"{len(results)} reference trees, {len(problems)} problems")
    for p in problems:
        print("  -", p)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
