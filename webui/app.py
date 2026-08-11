"""FastAPI web interface for stlsat — STL satisfiability checking.

Run with:  ./run.sh   (or:  uvicorn app:app --host 0.0.0.0 --port 8000)

Environment variables:
    STLSAT_BIN          path to the stlsat binary (default: `stlsat` on PATH,
                        falling back to ~/.cargo/bin/stlsat)
    STLSAT_MAX_TIMEOUT  hard cap for per-request timeout in seconds (default 600)
"""

from __future__ import annotations

import os
from pathlib import Path
from typing import Optional

from fastapi import FastAPI, HTTPException
from fastapi.responses import FileResponse
from pydantic import BaseModel, Field

import signals as signals_mod
import solver

MAX_TIMEOUT = float(os.environ.get("STLSAT_MAX_TIMEOUT", "600"))
STATIC_DIR = Path(__file__).parent / "static"

app = FastAPI(
    title="stlsat web interface",
    description="Satisfiability checking for Signal Temporal Logic (STL) formulas.",
    version="0.1.0",
)


class CheckRequest(BaseModel):
    formula: str = Field(..., description="STL formula, e.g. `G[0,10] (x > 0)`")
    engine: str = Field("tableau", description="tableau | fol | smt")
    solver: str = Field("auto", description="auto | z3 | dl")
    mltl: bool = Field(False, description="Use MLTL semantics")
    trace: bool = Field(False, description="Extract a satisfying trace (tableau only)")
    unsat_core: bool = Field(False, description="Extract unsat core (tableau only)")
    graph: bool = Field(False, description="Export the tableau graph as DOT (tableau only)")
    synthesize: bool = Field(True, description="Synthesize concrete signals from the trace")
    max_depth: Optional[int] = Field(None, ge=1, description="Tableau max depth")
    timeout: float = Field(60.0, gt=0, description="Timeout in seconds")


EXAMPLES = {
    "Simple (sat)": "G[0,10] (x > 0)",
    "Conflicting bounds (unsat)": "(G[0,10] (x > 5)) && (F[2,8] (x < 3))",
    "Until": "(x > 0) U[0,5] (y >= 2)",
    "Thermostat": "(G[0,40] (x1 <= 21)) && (G[0,10] ((x2 > 20) U[0,5] (on1))) && (G[0,20] ((x2 > 20) R[2,12] (x1 < 10))) && (F[0,20] (((off1) && (off2)) -> (G[0,5] ((on1) || (on2)))))",
    "Battery": "(G[1,20] (F[3,14] (d1 >= 1.4))) && (F[6,30] (((live1) && (live2)) -> (G[7,24] ((live1) && (live2))))) && (G[1,49] ((d1 > 0.5) && (d2 > 0.5))) && (G[11,50] (((g1 >= 0) || (g2 >= 0)) U[2,14] ((dead1) && (dead2))))",
    "Response pattern": "G[0,100] ((request > 0) -> (F[0,10] (grant > 0)))",
}


@app.get("/", include_in_schema=False)
def index() -> FileResponse:
    return FileResponse(STATIC_DIR / "index.html")


@app.get("/api/health")
def health() -> dict:
    return {
        "ok": solver.binary_available(),
        "binary": solver.STLSAT_BIN,
        "max_timeout_sec": MAX_TIMEOUT,
    }


@app.get("/api/examples")
def examples() -> dict:
    return EXAMPLES


@app.post("/api/check")
def check(req: CheckRequest) -> dict:
    if req.engine not in solver.ENGINES:
        raise HTTPException(422, f"engine must be one of {solver.ENGINES}")
    if req.solver not in solver.SOLVERS:
        raise HTTPException(422, f"solver must be one of {solver.SOLVERS}")
    if not req.formula.strip():
        raise HTTPException(422, "formula must not be empty")

    result = solver.check_formula(
        formula=req.formula,
        engine=req.engine,
        solver=req.solver,
        mltl=req.mltl,
        trace=req.trace,
        unsat_core=req.unsat_core,
        graph=req.graph,
        max_depth=req.max_depth,
        timeout=min(req.timeout, MAX_TIMEOUT),
    )

    if req.synthesize and result.get("trace"):
        try:
            result["signals"] = signals_mod.synthesize(result["trace"])
        except Exception as e:  # synthesis is best-effort; never break the check
            result["signals"] = None
            result["signals_error"] = str(e)
    # Don't leak absolute temp paths in the echoed command
    result["command"] = " ".join(
        os.path.basename(c) if c.startswith(("/", os.path.expanduser("~"))) else c
        for c in result.get("command", [])
    )
    return result


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(app, host="0.0.0.0", port=int(os.environ.get("PORT", "8000")))
