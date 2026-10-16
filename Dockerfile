# syntax=docker/dockerfile:1

ARG RUST_VERSION=1.90.0
ARG STLTREE_REF=cec92165abbe6bf241fc99f73717754a2f0d4a76
ARG MLTLSAT_REF=ccd6ec667ff01ce4ceea80aa273be1923dc49359
ARG STLSAT_PROOF_REF=67e0f3c6b712675d5a5947b5338fbff022ae8624
ARG Z3_WHEEL_URL=https://github.com/Z3Prover/z3/releases/download/z3-5.3.1/z3_solver-5.3.1.0-py3-none-manylinux_2_27_x86_64.whl
ARG Z3_WHEEL_SHA256=3e2e00bc80e776849d917daa194f33759ffa61d6528a859cfc4923ee3277a386

FROM debian:bookworm-slim AS experiment_sources

ARG STLTREE_REF
ARG MLTLSAT_REF
ARG STLSAT_PROOF_REF
RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates git \
    && rm -rf /var/lib/apt/lists/* \
    && git clone https://github.com/michiari/stltree.git /src/stltree \
    && git -C /src/stltree checkout --detach "${STLTREE_REF}" \
    && git clone https://github.com/michiari/mltlsat.git /src/mltlsat \
    && git -C /src/mltlsat checkout --detach "${MLTLSAT_REF}" \
    && git clone https://github.com/michiari/stlsat-proof.git /src/stlsat-proof \
    && git -C /src/stlsat-proof checkout --detach "${STLSAT_PROOF_REF}" \
    && rm -rf /src/stltree/.git /src/mltlsat/.git /src/stlsat-proof/.git


FROM debian:bookworm-slim AS z3_binary

ARG Z3_WHEEL_URL
ARG Z3_WHEEL_SHA256
RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates curl unzip \
    && rm -rf /var/lib/apt/lists/* \
    && curl --fail --location "${Z3_WHEEL_URL}" --output /tmp/z3.whl \
    && echo "${Z3_WHEEL_SHA256}  /tmp/z3.whl" | sha256sum --check - \
    && mkdir -p /tmp/z3-wheel /opt/z3/bin \
    && unzip -q /tmp/z3.whl -d /tmp/z3-wheel \
    && cp -a /tmp/z3-wheel/z3/include /opt/z3/include \
    && cp -a /tmp/z3-wheel/z3/lib /opt/z3/lib \
    && cp /tmp/z3-wheel/z3_solver-5.3.1.0.data/data/bin/z3 /opt/z3/bin/z3 \
    && chmod +x /opt/z3/bin/z3 \
    && mv /tmp/z3.whl /z3_solver-5.3.1.0-py3-none-manylinux_2_27_x86_64.whl

FROM debian:bookworm-slim AS webui_assets

RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /assets \
    && curl --fail --location --retry 3 https://cdnjs.cloudflare.com/ajax/libs/viz.js/2.1.2/viz.js --output /assets/viz.js \
    && curl --fail --location --retry 3 https://cdnjs.cloudflare.com/ajax/libs/viz.js/2.1.2/full.render.js --output /assets/full.render.js \
    && echo 'f111f22be005ceaf625f06c5a36ba7aa27703dbc0753914559edd1c24715b6e5  /assets/viz.js' | sha256sum --check - \
    && echo '3a0aace75d0b1677a4afda3b38b7697a569a02636cb3d7a04114f2cc2a95d8d4  /assets/full.render.js' | sha256sum --check -

FROM rust:${RUST_VERSION}-bookworm AS stlsat_builder

RUN apt-get update \
    && apt-get install --yes --no-install-recommends clang libclang-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src/stlsat
COPY . .
COPY --from=z3_binary /opt/z3 /opt/z3
# Link against the checksum-pinned official Z3 5.3.1 wheel.
RUN Z3_SYS_Z3_VERSION=5.3.1 \
    Z3_SYS_Z3_HEADER=/opt/z3/include/z3.h \
    Z3_LIBRARY_PATH_OVERRIDE=/opt/z3/lib \
    cargo build --release


FROM debian:bookworm-slim AS mltlsat_builder

RUN apt-get update \
    && apt-get install --yes --no-install-recommends build-essential zlib1g-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src/mltlsat
COPY --from=experiment_sources /src/mltlsat .
RUN make -C translator/src release


FROM debian:bookworm-slim AS stlsat_proof_dependencies

ENV ELAN_HOME=/root/.elan
RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        ca-certificates curl elan git unzip zstd \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src/stlsat-proof
COPY --from=experiment_sources /src/stlsat-proof .
# The repository pins both Lean and mathlib. Install that exact toolchain and
# prefetch the locked dependencies and mathlib binary cache without checking
# the STLSat proofs; users can run `lake build` in the final image.
RUN elan toolchain install "$(cat lean-toolchain)" \
    && lake exe cache get


FROM stlsat_proof_dependencies

ENV container=docker \
    ELAN_HOME=/root/.elan \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    PATH=/opt/z3/bin:/opt/venv/bin:${PATH} \
    LD_LIBRARY_PATH=/opt/z3/lib \
    BROWSER_PATH=/usr/bin/chromium

ENV STLSAT_BIN=/opt/stlsat/target/release/stlsat

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        chromium \
        dbus \
        elan \
        fonts-dejavu-core \
        git \
        libstdc++6 \
        librsvg2-bin \
        procps \
        python3 \
        python3-pip \
        python3-venv \
        systemd \
        systemd-sysv \
        time \
    && rm -rf /var/lib/apt/lists/* \
    && truncate --size 0 /etc/machine-id

COPY scripts/requirements.txt /tmp/stlsat-requirements.txt
COPY webui/requirements.txt /tmp/webui-requirements.txt
COPY --from=experiment_sources /src/stltree/requirements.txt /tmp/stltree-requirements.txt
COPY --from=z3_binary /z3_solver-5.3.1.0-py3-none-manylinux_2_27_x86_64.whl /tmp/
# The pinned STLTree revision locks z3-solver 4.15.3.0. Replace that one
# requirement so both Python tools use the same 5.3.1 build as STLSat.
RUN sed --in-place 's/^z3-solver==.*$/z3-solver==5.3.1.0/' /tmp/stltree-requirements.txt \
    && python3 -m venv /opt/venv \
    && pip install --no-cache-dir \
        --requirement /tmp/stlsat-requirements.txt \
        --requirement /tmp/webui-requirements.txt \
        --requirement /tmp/stltree-requirements.txt \
        /tmp/z3_solver-5.3.1.0-py3-none-manylinux_2_27_x86_64.whl \
    && rm /tmp/stlsat-requirements.txt \
        /tmp/webui-requirements.txt \
        /tmp/stltree-requirements.txt \
        /tmp/z3_solver-5.3.1.0-py3-none-manylinux_2_27_x86_64.whl

COPY --from=z3_binary /opt/z3 /opt/z3
COPY . /opt/stlsat
COPY --from=webui_assets /assets /opt/stlsat/webui/static/vendor/viz.js/2.1.2
COPY --from=stlsat_builder /src/stlsat/target/release/stlsat /opt/stlsat/target/release/stlsat
COPY --from=experiment_sources /src/stltree /opt/stltree
COPY --from=experiment_sources /src/mltlsat /opt/mltlsat
COPY --from=mltlsat_builder /src/mltlsat/translator/src/MLTLConvertor /opt/mltlsat/translator/src/MLTLConvertor
RUN ln --symbolic /src/stlsat-proof /opt/stlsat-proof

RUN mkdir -p /results \
    && chmod +x \
        /opt/stlsat/scripts/*.py \
        /opt/stlsat/scripts/*.sh \
        /opt/stltree/stltree.py \
        /opt/mltlsat/translator/src/MLTLConvertor

WORKDIR /opt/stlsat/scripts

# Start the optional web interface by overriding CMD; see webui/README.md.
EXPOSE 8000

STOPSIGNAL SIGRTMIN+3
CMD ["/sbin/init"]
