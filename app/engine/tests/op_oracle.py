"""Op-parity oracle: prove each Rust mutation matches the Python engine.

For each operation we apply it through the proven Python engine and through the
Rust engine (via the oracle CLI) on the same real save, then assert the two
results are SEMANTICALLY equal (parsed JSON equal). We compare parsed values,
not bytes, because the Rust engine preserves original float literals while
Python reformats them — both parse to the same f64, so json.loads equality is
the correct, robust criterion.

Deterministic ops only (repair / set / delete / move). add/split mint random
UUIDs, so they're validated by the Rust unit tests instead.

Usage:  python tests/op_oracle.py "<save_path>"
Run from the engine crate dir.
"""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path

ENGINE_DIR = Path(__file__).resolve().parent.parent
REPO_ROOT = ENGINE_DIR.parent.parent  # app/engine -> app -> repo root
ORACLE = ENGINE_DIR / "target" / "release" / "oracle.exe"
sys.path.insert(0, str(REPO_ROOT))

import save_io  # noqa: E402


def load(save: Path) -> dict:
    return json.loads(save.read_text(encoding="utf-8"))


def rust_op(save: Path, args: list[str], out: Path) -> dict:
    subprocess.run([str(ORACLE), "op", *args[:1], str(save), str(out), *args[1:]], check=True)
    return json.loads(out.read_text(encoding="utf-8"))


def py_set_fields(data, source, item_id, qty, cond, dur):
    items = save_io.get_items_list(data, source)
    it = next(i for i in items if i.get("Id") == item_id)
    ad = it.setdefault("AdditionalData", {}).setdefault("_data", {})
    if qty is not None:
        ad["StackableComponent_quantity"] = int(qty)
    if cond is not None:
        ad["Condition_d"] = float(cond)
        ad["Condition_mt"] = float(cond)
    if dur is not None:
        ad["DurabilityComponent_durability"] = float(dur)
        ad["DurabilityComponent_md"] = float(dur)


def py_move(data, source, item_id, i, j):
    items = save_io.get_items_list(data, source)
    it = next(x for x in items if x.get("Id") == item_id)
    it["Position"] = {"I": int(i), "J": int(j)}


def pick_targets(data) -> dict:
    eq = save_io.get_items_list(data, "equipment")
    inv = save_io.get_items_list(data, "inventory")
    # repair: every equipment item that has a condition/durability stat
    repair_ids = []
    for it in eq:
        ad = (it.get("AdditionalData") or {}).get("_data") or {}
        if any(k in ad for k in ("Condition_d", "Condition_mt",
                                 "DurabilityComponent_durability", "DurabilityComponent_md")):
            repair_ids.append(it["Id"])
    # set: an inventory item with a stack quantity
    set_id = next(
        (it["Id"] for it in inv
         if "StackableComponent_quantity" in ((it.get("AdditionalData") or {}).get("_data") or {})),
        inv[0]["Id"],
    )
    # delete: a leaf inventory item (no children)
    parents = {it.get("ParentId") for it in inv}
    del_id = next((it["Id"] for it in inv if it["Id"] not in parents), inv[-1]["Id"])
    # move: same set item to a new slot
    return {"repair": repair_ids[:8], "set": set_id, "delete": del_id, "move": set_id}


def main() -> int:
    if len(sys.argv) < 2:
        print('usage: python tests/op_oracle.py "<save_path>"')
        return 2
    save = Path(sys.argv[1])
    if not ORACLE.exists():
        print("build the oracle first: cargo build --release --bin oracle")
        return 2
    t = pick_targets(load(save))
    cases: list[tuple[str, list[str], callable]] = [
        ("repair", ["repair", ",".join(t["repair"])],
         lambda d: save_io.set_items_condition_durability_full(d, set(t["repair"]), top_off_stacks=True)),
        ("set qty=999", ["set", "inventory", t["set"], "999", "-", "-"],
         lambda d: py_set_fields(d, "inventory", t["set"], 999, None, None)),
        ("set cond=4", ["set", "equipment", t["repair"][0], "-", "4", "-"],
         lambda d: py_set_fields(d, "equipment", t["repair"][0], None, 4, None)),
        ("delete", ["delete", t["delete"]],
         lambda d: save_io.remove_items_by_ids(d, {t["delete"]})),
        ("move 3,5", ["move", "inventory", t["move"], "3", "5"],
         lambda d: py_move(d, "inventory", t["move"], 3, 5)),
    ]

    ok = True
    with tempfile.TemporaryDirectory() as td:
        for name, rust_args, py_fn in cases:
            rust_out = rust_op(save, rust_args, Path(td) / "r.json")
            pd = load(save)
            py_fn(pd)
            same = rust_out == pd
            print(f"[{'PASS' if same else 'FAIL'}] {name}")
            if not same:
                ok = False
                # crude diff: report a few differing top-level item entries
                _report_diff(rust_out, pd)
    return 0 if ok else 1


def _report_diff(a: dict, b: dict) -> None:
    for src in ("inventory", "equipment", "shelter"):
        ia = {it["Id"]: it for it in save_io.get_items_list(a, src)}
        ib = {it["Id"]: it for it in save_io.get_items_list(b, src)}
        if ia.keys() != ib.keys():
            print(f"  {src}: id sets differ (rust {len(ia)} vs py {len(ib)})")
        for k in list(ia.keys() & ib.keys())[:200]:
            if ia[k] != ib[k]:
                print(f"  {src} item {k[:8]} differs:\n    rust={ia[k]}\n    py  ={ib[k]}")
                return


if __name__ == "__main__":
    raise SystemExit(main())
