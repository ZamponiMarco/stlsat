# STLSat web interface and demo case studies

The web interface provides a formula editor, satisfiability checking, unsat
cores, witness traces, signal plots, and tableau graphs. The instructions below
reproduce the Pump, Water tank, and Thermostat examples in the tool demonstration
paper, *STLSat: A Satisfiability Checker for Discrete-Time Signal Temporal Logic*
(Sections 3–4 and the case-study table).

## Run the Docker image

Build from the repository root (the image targets `linux/amd64`):

```bash
docker build --tag stlsat-experiments .
```

Start the web server, publishing it on the host's loopback interface:

```bash
docker run --rm --init \
  --name stlsat-web \
  --publish 127.0.0.1:8000:8000 \
  --stop-signal SIGTERM \
  --workdir /opt/stlsat/webui \
  stlsat-experiments \
  uvicorn app:app --host 0.0.0.0 --port 8000
```

Open <http://localhost:8000> in a browser on the host. The header should report
**solver ready**. API documentation is available at <http://localhost:8000/docs>;
<http://localhost:8000/api/health> should return `"ok": true`.
Press Ctrl-C in the terminal to stop the server and remove its container.
If host port 8000 is occupied, publish `127.0.0.1:8080:8000` and visit
<http://localhost:8080> instead.


## Reproduce the paper's examples

For all examples, select **Engine: tableau**, **Solver: auto**, leave **MLTL
semantics** unchecked, keep **Timeout (s): 60**, and leave **Max depth** empty.
Select an example in **Load example**, enable the requested diagnostics, then
press **Check satisfiability** (or Ctrl-Enter / Cmd-Enter).

The formulas below must each be entered on a single line.
See the root [README](../README.md) for formula syntax.

If graph rendering fails, use **download .dot** and render the graph with Graphviz,
for example `dot -Tsvg tableau.dot -o tableau.svg`.


### Pump: diagnose a contradiction

Load **Pump (unsat)**, enable **Unsat core**, and check:

```text
(G[0,10] (x > 5)) && (F[2,8] (x < 3))
```

Expect **UNSAT**. The conflicting requirements demand `x > 5` throughout
`[0,10]` and `x < 3` at some point in `[2,8]`. Inspect **Unsat core** and expand
**Raw solver output** to see the underlying checker output.

### Water tank: find and repair inconsistent requirements

Load **Water Tank (original)** and enable **Unsat core**:

```text
(G[0,100] (level <= 90)) && (G[0,100] ((demand) -> (F[0,10] (level >= 95)))) && (G[30,40] (demand)) && (G[0,100] (level >= 40))
```

The four conjuncts are the paper's requirements R1–R4: an upper bound of 90,
refill to at least 95 within 10 time units of demand, scheduled demand during
`[30,40]`, and a lower bound of 40. Expect **UNSAT**. The conflict is between
R1–R3: demand is forced, but the requested refill exceeds the upper bound. R4
does not cause this conflict. The displayed core may use normalized subformulas
rather than preserve the original formatting; it is not guaranteed to be minimal.

Change the refill threshold from `95` to `80`, or load **Water Tank (repaired)**:

```text
(G[0,100] (level <= 90)) && (G[0,100] ((demand) -> (F[0,10] (level >= 80)))) && (G[30,40] (demand)) && (G[0,100] (level >= 40))
```

Enable **Trace + signals** and check again. Expect **SAT**. Inspect the level
plot, demand lane, and **Satisfying trace** table. A witness supplies values
consistent with the specification; unspecified behavior is not a controller
simulation. To compare engines, rerun with **Engine: smt** and **Engine: fol**;
the verdict should agree, although performance can differ and diagnostics are
available only for the tableau engine.

### Thermostat: explore a witness

Load **Thermostat**, select **Engine: tableau**, and enable **Trace + signals**:

```text
(G[0,40] (x1 <= 21)) && (G[0,10] ((x2 > 20) U[0,5] (on1))) && (G[0,20] ((x2 > 20) R[2,12] (x1 < 10))) && G[0,20] ((off1 && off2) -> (G[0,5] (on1 || on2))) && F[0,20] (off1 && off2)
```

Expect **SAT**. This is the paper's screenshot example, including the
reactivation requirement and the eventual occurrence of both heaters being
off. Inspect the `x1` and `x2` step plots and the available heater Boolean lanes
(`on1`, `off1`, `off2`, and possibly `on2`, depending on the witness). Move the
cursor over the plots to inspect values at a shared
time point, and compare them with the literal obligations in **Satisfying
trace**. Unconstrained Boolean values are distinguished from false values;
constraints the signal synthesizer cannot resolve are reported below the plots.

Enable **Tableau graph** and rerun to inspect or download the proof tree.
Graphs with more than 600 nodes are offered as DOT downloads instead of being
rendered inline; graphs exceeding the server's 4 MiB transfer limit are omitted.


## REST API

The same server accepts JSON requests at `POST /api/check`. For example, reproduce
the Pump check from a host terminal:

```bash
curl --fail-with-body http://localhost:8000/api/check \
  --header 'Content-Type: application/json' \
  --data '{"formula":"(G[0,10] (x > 5)) && (F[2,8] (x < 3))","engine":"tableau","solver":"auto","unsat_core":true,"timeout":60}'
```

The response should contain `"status":"ok"`, `"result":"unsat"`, and an
`unsat_core`. The built-in formulas are also available at `GET /api/examples`.
For satisfiable checks, set `"trace":true` to obtain the trace and synthesized
signals; set `"graph":true` to request DOT. Each request's timeout is capped at
600 seconds by default. Pass `--env STLSAT_MAX_TIMEOUT=120` to `docker run`
before the image name to change that cap.

## Run without Docker

The local Viz.js copies are bundled by the Docker build. For offline graph
rendering in a bare-metal checkout, download them once from the repository root (optional):

```bash
mkdir -p webui/static/vendor/viz.js/2.1.2
curl --fail --location https://cdnjs.cloudflare.com/ajax/libs/viz.js/2.1.2/viz.js \
  --output webui/static/vendor/viz.js/2.1.2/viz.js
curl --fail --location https://cdnjs.cloudflare.com/ajax/libs/viz.js/2.1.2/full.render.js \
  --output webui/static/vendor/viz.js/2.1.2/full.render.js
```

Install STLSat following the root README, then run from the repository root:

```bash
python3 -m venv webui/.venv
webui/.venv/bin/pip install --requirement webui/requirements.txt
STLSAT_BIN="$(pwd)/target/release/stlsat" ./webui/run.sh
```

This assumes `cargo build --release` has produced the binary and its Z3 shared
library can be found by the dynamic loader. If using a Cargo-installed binary,
use `STLSAT_BIN="$HOME/.cargo/bin/stlsat"` instead. The run script listens on
all network interfaces; for host-only access, run Uvicorn directly instead:

```bash
STLSAT_BIN="$(pwd)/target/release/stlsat" \
  webui/.venv/bin/uvicorn app:app --app-dir webui --host 127.0.0.1 --port 8000
```
