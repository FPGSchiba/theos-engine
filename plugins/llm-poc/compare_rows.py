"""Compare two harness results folders row by row (determinism check).

Usage: python -I compare_rows.py <reference-dir> <new-dir>

Reads `rows.jsonl` of both folders (npc-poc or bt-poc format). For every row
key present in both, the model output is compared when both rows stopped with
`finished`: npc-poc rows by their `raw` field, bt-poc rows by the saved tree
file `trees/<key>.json` (raw output plus thinking). Rows cut off by a
deadline depend on timing and are listed apart, not compared.

Prints `identical N / compared M`, the differing keys, the cut-off keys and
the keys present in only one folder. Exit code 0 when every compared row is
identical, 1 otherwise. Never prints model output.
"""

import json
import sys
from pathlib import Path


def load(folder: Path) -> dict:
    rows = {}
    with open(folder / "rows.jsonl", encoding="utf-8") as f:
        for line in f:
            r = json.loads(line)
            if "key" in r:  # bt-poc
                key = r["key"]
                tree = folder / "trees" / f"{key}.json"
                out = tree.read_text(encoding="utf-8") if tree.is_file() else None
            else:  # npc-poc
                key = f"{r['scenario']}-{r['thinking']}-{r['scope']}"
                out = r.get("raw")
            stop = (r.get("metrics") or {}).get("stop")
            # npc-poc writes "finished", bt-poc "Finished".
            rows[key] = (stop.lower() if isinstance(stop, str) else stop, out)
    return rows


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__)
        return 2
    ref, new = load(Path(sys.argv[1])), load(Path(sys.argv[2]))
    compared, identical, differ, cut, missing = 0, 0, [], [], []
    for key in sorted(set(ref) | set(new)):
        if key not in ref or key not in new:
            missing.append(key)
            continue
        (s1, o1), (s2, o2) = ref[key], new[key]
        if s1 != "finished" or s2 != "finished":
            cut.append(f"{key} ({s1} / {s2})")
            continue
        compared += 1
        if o1 is not None and o1 == o2:
            identical += 1
        else:
            differ.append(key)
    print(f"identical {identical} / compared {compared}")
    for title, keys in (("differ", differ), ("cut off (not compared)", cut), ("in one folder only", missing)):
        if keys:
            print(f"{title}: {len(keys)}")
            for k in keys:
                print(f"  {k}")
    return 0 if identical == compared else 1


if __name__ == "__main__":
    sys.exit(main())
