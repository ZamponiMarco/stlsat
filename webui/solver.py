"""Thin wrapper around the `stlsat` binary.

Writes the formula to a temp file, invokes the binary with the chosen
options, enforces a timeout, and parses the textual output into a dict.
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import tempfile
import time

STLSAT_BIN = os.environ.get("STLSAT_BIN") or shutil.which("stlsat") or os.path.expanduser(
    "~/.cargo/bin/stlsat"
)

ENGINES = ("tableau", "fol", "smt")
SOLVERS = ("auto", "z3", "dl")

_RESULT_RE = re.compile(r"result:\s*(Some\(true\)|Some\(false\)|None)")
_DURATION_RE = re.compile(r"DURATION_SEC:\s*([0-9.eE+-]+)")
_UNSAT_CORE_RE = re.compile(r"Unsat core:\s*(.+)")
_TRACE_LEN_RE = re.compile(r"Trace length:\s*(\d+)")
_NODE_COUNT_RE = re.compile(r"Node count:\s*(\d+)")

MAX_DOT_BYTES = 4 * 1024 * 1024  # don't ship absurdly large graphs to the browser


def binary_available() -> bool:
    return os.path.isfile(STLSAT_BIN) and os.access(STLSAT_BIN, os.X_OK)


def _parse_trace(stdout: str) -> list[list[str]] | None:
    """Parse the trace block printed after `Trace length: N`:

        [
          [a, b],
          [c]
        ]
    """
    m = _TRACE_LEN_RE.search(stdout)
    if not m:
        return None
    tail = stdout[m.end():]
    steps: list[list[str]] = []
    in_block = False
    for line in tail.splitlines():
        s = line.strip()
        if s == "[":
            in_block = True
            continue
        if s == "]":
            break
        if in_block and s.startswith("["):
            inner = s.rstrip(",").strip()
            inner = inner[1:-1] if inner.startswith("[") and inner.endswith("]") else inner
            steps.append([p.strip() for p in inner.split(",") if p.strip()])
    return steps


def check_formula(
    formula: str,
    engine: str = "tableau",
    solver: str = "auto",
    mltl: bool = False,
    trace: bool = False,
    unsat_core: bool = False,
    graph: bool = False,
    max_depth: int | None = None,
    timeout: float = 60.0,
) -> dict:
    if engine not in ENGINES:
        raise ValueError(f"unknown engine {engine!r}")
    if solver not in SOLVERS:
        raise ValueError(f"unknown solver {solver!r}")
    formula = formula.strip()
    if not formula:
        raise ValueError("empty formula")

    fd, path = tempfile.mkstemp(suffix=".stl", text=True)
    with os.fdopen(fd, "w") as f:
        f.write(formula + "\n")

    dot_path = None
    cmd = [STLSAT_BIN, path, "--engine", engine, "--solver", solver]
    if mltl:
        cmd.append("--mltl")
    if engine == "tableau":
        if trace:
            cmd.append("--trace-extraction")
        if unsat_core:
            cmd.append("--unsat-core-extraction")
        if graph:
            dfd, dot_path = tempfile.mkstemp(suffix=".dot")
            os.close(dfd)
            cmd += ["--graph-output", dot_path]
        if max_depth:
            cmd += ["--max-depth", str(int(max_depth))]

    def _cleanup(remove_dot: bool):
        for p in ([path] + ([dot_path] if remove_dot and dot_path else [])):
            try:
                os.unlink(p)
            except OSError:
                pass

    start = time.perf_counter()
    try:
        proc = subprocess.run(
            cmd, capture_output=True, text=True, timeout=timeout,
            env={**os.environ, "RUST_LOG": os.environ.get("RUST_LOG", "")},
        )
    except subprocess.TimeoutExpired:
        _cleanup(remove_dot=True)
        return {
            "status": "timeout",
            "result": None,
            "wall_time_sec": round(time.perf_counter() - start, 3),
            "timeout_sec": timeout,
            "command": cmd,
        }
    except FileNotFoundError:
        _cleanup(remove_dot=True)
        return {
            "status": "error",
            "result": None,
            "error": f"stlsat binary not found at {STLSAT_BIN!r}. "
                     "Run install.sh or set the STLSAT_BIN environment variable.",
            "command": cmd,
        }
    _cleanup(remove_dot=False)

    dot = None
    dot_truncated = False
    if dot_path:
        try:
            size = os.path.getsize(dot_path)
            if size <= MAX_DOT_BYTES:
                with open(dot_path, encoding="utf-8", errors="replace") as f:
                    dot = f.read()
            else:
                dot_truncated = True
        except OSError:
            pass
        try:
            os.unlink(dot_path)
        except OSError:
            pass

    wall = round(time.perf_counter() - start, 3)
    stdout, stderr = proc.stdout, proc.stderr

    if proc.returncode != 0:
        return {
            "status": "error",
            "result": None,
            "error": (stderr.strip() or stdout.strip() or f"exit code {proc.returncode}")[-4000:],
            "wall_time_sec": wall,
            "stdout": stdout[-4000:],
            "command": cmd,
        }

    m = _RESULT_RE.search(stdout)
    verdict = {"Some(true)": "sat", "Some(false)": "unsat", "None": "unknown"}.get(
        m.group(1) if m else "", "unknown"
    )
    dm = _DURATION_RE.search(stdout)
    cm = _UNSAT_CORE_RE.search(stdout)
    nm = _NODE_COUNT_RE.search(stdout)

    return {
        "status": "ok",
        "result": verdict,
        "solver_time_sec": float(dm.group(1)) if dm else None,
        "wall_time_sec": wall,
        "unsat_core": cm.group(1).strip() if cm else None,
        "trace": _parse_trace(stdout),
        "dot": dot,
        "dot_truncated": dot_truncated,
        "node_count": int(nm.group(1)) if nm else None,
        "stdout": stdout[-8000:],
        "command": cmd,
    }
