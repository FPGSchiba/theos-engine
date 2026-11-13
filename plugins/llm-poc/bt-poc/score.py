"""Scores one reply envelope with the bt-poc fixture's own reference evaluator (check.py).

    python -I score.py --fixture <fixtures/bt-poc> --scenarios <file> --scenario <id> <envelope.json>

Prints one JSON object: gate, gate_rule, gate_message, nodes, depth, checks, behaviour, grounding.
Python 3.11+, standard library only. Run in isolated mode (-I), so the fixture directory is put
on sys.path explicitly.
"""
import argparse
import json
import sys
import tomllib
from pathlib import Path


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--fixture", required=True)
    ap.add_argument("--scenarios", required=True)
    ap.add_argument("--scenario", required=True)
    ap.add_argument("envelope")
    args = ap.parse_args()

    fixture = Path(args.fixture).resolve()
    sys.path.insert(0, str(fixture))
    import check  # noqa: E402  (the fixture's evaluator, imported unchanged)

    vocab = json.loads((fixture / "vocabulary.json").read_text(encoding="utf-8"))
    visible_path = fixture.parent / "lore-poc" / "expected" / "visible.json"
    visible = json.loads(visible_path.read_text(encoding="utf-8"))
    scen = tomllib.loads(Path(args.scenarios).read_text(encoding="utf-8"))
    scenarios = {s["id"]: s for s in scen["scenario"]}
    for s in scenarios.values():
        s["actor_has_lore"] = s["actor"] in visible

    envelope = json.loads(Path(args.envelope).read_text(encoding="utf-8"))
    res = check.score(envelope, scenarios[args.scenario], vocab, visible)
    res.pop("timeline")
    print(json.dumps(res))
    return 0


if __name__ == "__main__":
    sys.exit(main())
