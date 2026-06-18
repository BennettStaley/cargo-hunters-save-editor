"""Oracle harness: prove the Rust engine is a faithful, stable save round-trip.

Plain serde_json reformats floats and can shift their value by 1 ULP (its float
parser disagrees with Python's), so "byte-identical to Python" is the WRONG bar
— Python's own load/dump round-trip can perturb floats too. With
`arbitrary_precision`, the Rust engine never parses numbers to f64; it preserves
every literal verbatim, which is the strongest correctness guarantee.

The gate, run on real saves:
  A. NO VALUE CHANGE   parse(rust_out) == parse(original_save)   (no dropped /
     added / reordered / altered values; array order included)
  B. IDEMPOTENT        rust(rust_out) == rust_out  byte-for-byte (stable point)

We also REPORT whether rust_out matches Python's `json.dumps(indent=4)` purely
for information; it is not required to pass.

Usage:
    python tests/oracle.py roundtrip "<save_path>"
Run from the engine crate dir (locates target/release/oracle.exe).
"""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path

ENGINE_DIR = Path(__file__).resolve().parent.parent
ORACLE_EXE = ENGINE_DIR / "target" / "release" / "oracle.exe"


def rust_roundtrip(save_path: Path, out_path: Path) -> bytes:
    subprocess.run(
        [str(ORACLE_EXE), "roundtrip", str(save_path), str(out_path)], check=True
    )
    return out_path.read_bytes()


def first_diff(a: bytes, b: bytes, who_a: str, who_b: str) -> str:
    n = min(len(a), len(b))
    for i in range(n):
        if a[i] != b[i]:
            lo = max(0, i - 40)
            return (
                f"first diff at byte {i} (len {who_a}={len(a)} {who_b}={len(b)}):\n"
                f"  {who_a}: ...{a[lo:i+40]!r}\n"
                f"  {who_b}: ...{b[lo:i+40]!r}"
            )
    if len(a) != len(b):
        longer, who = (a, who_a) if len(a) > len(b) else (b, who_b)
        return f"common prefix equal; {who} has {abs(len(a)-len(b))} extra bytes: {longer[n:n+60]!r}"
    return "no difference"


def main() -> int:
    if len(sys.argv) < 3 or sys.argv[1] != "roundtrip":
        print('usage: python tests/oracle.py roundtrip "<save_path>"')
        return 2
    save_path = Path(sys.argv[2])
    if not ORACLE_EXE.exists():
        print(f"oracle.exe not found at {ORACLE_EXE}; run: cargo build --release --bin oracle")
        return 2

    original = save_path.read_bytes()
    with tempfile.TemporaryDirectory() as td:
        rust1 = rust_roundtrip(save_path, Path(td) / "r1.json")
        # Feed Rust output back through Rust for the idempotency check.
        (Path(td) / "r1src.save").write_bytes(rust1)
        rust2 = rust_roundtrip(Path(td) / "r1src.save", Path(td) / "r2.json")

    # A. No value change vs the original save.
    no_change = json.loads(rust1.decode("utf-8")) == json.loads(original.decode("utf-8"))
    # B. Idempotent fixed point.
    idempotent = rust1 == rust2
    # Info: does it match Python's (potentially lossy) reformatting?
    py = json.dumps(json.loads(original.decode("utf-8")), indent=4, ensure_ascii=False).encode("utf-8")
    py_match = rust1 == py

    print(f"sizes: rust={len(rust1)}  python={len(py)}  original={len(original)}")
    print(f"[GATE] A no-value-change vs original : {'PASS' if no_change else 'FAIL'}")
    print(f"[GATE] B idempotent fixed point      : {'PASS' if idempotent else 'FAIL'}")
    print(f"[info] matches Python json.dumps     : {'yes' if py_match else 'no'}")
    if not no_change:
        print("  !! VALUE CHANGED ON ROUND-TRIP — this must never happen")
    if not idempotent:
        print(first_diff(rust1, rust2, "pass1", "pass2"))
    if not py_match:
        print("  (Python-match is informational only)")
        print("  " + first_diff(rust1, py, "rust", "py").replace("\n", "\n  "))

    return 0 if (no_change and idempotent) else 1


if __name__ == "__main__":
    raise SystemExit(main())
