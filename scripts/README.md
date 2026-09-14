# Reproducing the experiments

The experiment scripts compare STLSat with [STLTree](https://github.com/michiari/stltree)
and [MLTLSAT](https://github.com/michiari/mltlsat). Each benchmark is placed in a
transient systemd scope so that its timeout and memory limit are enforced by Linux
cgroups. The Docker image also includes everything needed to build and check the Lean formalization in
[STLSat Proof](https://github.com/michiari/stlsat-proof).

These instructions assume that the four repositories are siblings:

```text
Repos/
├── stlsat/
├── stlsat-proof/
├── stltree/
└── mltlsat/
```

The bare-metal workflow uses these local checkouts. The Dockerfile copies the current
STLSat working tree and fetches pinned STLTree, MLTLSAT, and STLSat Proof revisions,
making the image independent of directories outside the Docker build context. Record
the STLSat commit and dirty state, as well as the final image ID, alongside published
results.

## Running on bare metal

Use a Linux host booted with systemd and cgroups v2. Install:

- a Rust toolchain supporting edition 2024;
- Python 3.11 or newer and `venv`;
- a C++ compiler and `make` for the MLTLSAT translator;
- Clang and its development library for Rust's Z3 bindings;
- GNU `time`;
- Chromium or Chrome for Plotly/Kaleido image export;
- `rsvg-convert` (usually provided by `librsvg2-bin`) for PDF conversion.

For Debian or Ubuntu, the non-Rust packages can be installed with:

```bash
sudo apt-get update
sudo apt-get install \
  build-essential chromium-browser clang libclang-dev librsvg2-bin \
  python3-venv time
```

The Chromium package name differs between distributions. On Debian it is `chromium`.

Create the Python environment first. The installed `z3-solver` wheel supplies the
pre-built Z3 4.15.8 shared library and headers used by Python and the Rust build:

```bash
cd /path/to/Repos/stlsat
python3 -m venv .venv
source .venv/bin/activate

# The pinned STLTree revision asks for 4.15.3.0; replace only that entry so
# every experiment uses the requested official Z3 4.15.8 Python wheel.
sed 's/^z3-solver==.*$/z3-solver==4.15.8.0/' \
  ../stltree/requirements.txt > /tmp/stltree-requirements-z3-4.15.8.txt
pip install \
  -r scripts/requirements.txt \
  -r /tmp/stltree-requirements-z3-4.15.8.txt
z3 --version
```

Build STLSat against that same library, then build the MLTLSAT translator. Keep
`LD_LIBRARY_PATH` set in shells that run the bare-metal experiments:

```bash
Z3_PY_DIR="$(python -c 'import pathlib, z3; print(pathlib.Path(z3.__file__).parent)')"
export LD_LIBRARY_PATH="$Z3_PY_DIR/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

Z3_SYS_Z3_HEADER="$Z3_PY_DIR/include/z3.h" \
Z3_LIBRARY_PATH_OVERRIDE="$Z3_PY_DIR/lib" \
  cargo build --release

make -C ../mltlsat/translator/src release
```

To check the formal proofs on bare metal, install
[elan](https://github.com/leanprover/elan), then run Lake from the proof checkout.
Its `lean-toolchain` and `lake-manifest.json` pin the Lean and dependency versions:

```bash
cd /path/to/Repos/stlsat-proof
lake exe cache get
lake build
```

Check that the user systemd manager is reachable:

```bash
systemctl --user is-system-running
systemd-run --user --quiet --scope --collect \
  -p MemoryMax=256M -p MemorySwapMax=0 -p RuntimeMaxSec=5s /bin/true
```

Run all commands from `stlsat/scripts`. Bare-metal runs use the user manager, which
is the default; `--systemd-manager user` is shown explicitly below:

```bash
cd /path/to/Repos/stlsat/scripts

./stl_benchmarks.py /path/to/Repos/stltree/stltree.py \
  --iters 5 --timeout 120 --systemd-manager user \
  --csv results.csv

./run_mltl_comparison.sh /path/to/Repos/mltlsat \
  --iters 5 --jobs 32 --max-mem 23000 --timeout 120 \
  --systemd-manager user \
  --output-dir results_dir_mltl \
  --stltree-path /path/to/Repos/stltree/stltree.py

./run_random_stl_comparison.sh \
  --iters 5 --jobs 32 --max-mem 23000 --timeout 120 \
  --systemd-manager user \
  --output-dir results_dir_stl \
  --stltree-path /path/to/Repos/stltree/stltree.py
```

Generate the plots after the comparison runs finish:

```bash
./make_plots.sh MLTL --output-dir plots_dir --base-dir results_dir_mltl
./make_plots.sh STL  --output-dir plots_dir --base-dir results_dir_stl
```

`--max-mem` is a per-benchmark limit, not a limit shared by all workers. For example,
`--jobs 32 --max-mem 23000` can theoretically require about 736 GB plus runner and
operating-system overhead. Select `--jobs` according to both the available CPUs and
memory.

### User versus system manager

`run_bench.py`, `stl_benchmarks.py`, and both comparison shell scripts accept:

```text
--systemd-manager {user,system}
```

`user` contacts the calling user's systemd manager and normally requires no root
privileges. `system` contacts the system manager (PID 1) and normally requires root
or suitable polkit authorization. Use `user` for ordinary bare-metal runs and
`system` in the container described below.

## Running in Docker

The image runs systemd as PID 1. Consequently it must be started with a writable
cgroup hierarchy. The command below uses a private cgroup namespace and privileged
mode; only build and run trusted source trees this way.

The Docker image currently targets `linux/amd64`. The Dockerfile fetches the two
comparison repositories and proof repository at revisions pinned by its `STLTREE_REF`,
`MLTLSAT_REF`, and `STLSAT_PROOF_REF` build arguments. It installs the proof
repository's pinned Lean toolchain and prefetches its locked dependencies and mathlib
binary cache, but does not check the proofs during `docker build`. It uses the official Z3 4.15.8
manylinux wheel for the Rust library, Python package, and command-line executable.
The wheel is protected by its published SHA-256 digest; using it directly avoids the
Rust crate's rate-limit-prone GitHub API lookup. Build the image from the STLSat root:

```bash
cd /path/to/Repos/stlsat

docker build \
  --tag stlsat-experiments .
```

To deliberately use other committed revisions, override the pins:

```bash
docker build \
  --build-arg STLTREE_REF=<commit-or-tag> \
  --build-arg MLTLSAT_REF=<commit-or-tag> \
  --build-arg STLSAT_PROOF_REF=<commit-or-tag> \
  --tag stlsat-experiments .
```

The requested revisions must be available from the corresponding GitHub repositories;
uncommitted changes in the sibling checkouts are not included in the image.

Create an output directory on the host and start the systemd container:

```bash
mkdir -p docker-results

docker run --detach \
  --name stlsat-experiments \
  --privileged \
  --cgroupns=private \
  --tmpfs /run \
  --tmpfs /run/lock \
  --mount type=bind,source="$(pwd)/docker-results",target=/results \
  stlsat-experiments
```

Verify that systemd and per-benchmark resource controls work before starting the
full experiments:

```bash
docker exec stlsat-experiments systemctl is-system-running

docker exec stlsat-experiments \
  systemd-run --system --quiet --scope --collect \
    -p MemoryMax=256M -p MemorySwapMax=0 -p RuntimeMaxSec=5s /bin/true
```

`systemctl is-system-running` may report `degraded` because hardware-related units
are unavailable in a container. The `systemd-run` smoke test must nevertheless exit
successfully.

The proof tree, pinned Lean toolchain, and prefetched dependencies are retained in the
image. Build and check all proofs when desired with:

```bash
docker exec stlsat-experiments bash -lc '
  cd /opt/stlsat-proof
  lake build
'
```

Run the experiments with the paths embedded in the image. Output is written through
the `/results` bind mount:

```bash
docker exec stlsat-experiments bash -lc '
  ./stl_benchmarks.py /opt/stltree/stltree.py \
    --iters 5 --timeout 120 --systemd-manager system \
    --csv /results/results.csv
'

docker exec stlsat-experiments bash -lc '
  ./run_mltl_comparison.sh /opt/mltlsat \
    --iters 5 --jobs 32 --max-mem 23000 --timeout 120 \
    --systemd-manager system \
    --output-dir /results/mltl \
    --stltree-path /opt/stltree/stltree.py
'

docker exec stlsat-experiments bash -lc '
  ./run_random_stl_comparison.sh \
    --iters 5 --jobs 32 --max-mem 23000 --timeout 120 \
    --systemd-manager system \
    --output-dir /results/stl \
    --stltree-path /opt/stltree/stltree.py
'
```

Generate both sets of plots inside the same container:

```bash
docker exec stlsat-experiments bash -lc '
  ./make_plots.sh MLTL --output-dir /results/plots --base-dir /results/mltl
  ./make_plots.sh STL  --output-dir /results/plots --base-dir /results/stl
'
```

The container runs the experiments as root so it can create system scopes. If the
bind-mounted output is owned by root afterward, restore ownership from the host:

```bash
docker exec stlsat-experiments \
  chown -R "$(id -u):$(id -g)" /results
```

Stop and remove the container when finished:

```bash
docker stop stlsat-experiments
docker rm stlsat-experiments
```

For meaningful timing comparisons, use a native Linux host, keep the host otherwise
idle, use the same image and Docker resource settings across runs, and record the CPU,
kernel, Docker, and image versions with the results.
