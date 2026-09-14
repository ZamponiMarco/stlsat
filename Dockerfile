# syntax=docker/dockerfile:1

ARG RUST_VERSION=1.90.0
ARG STLTREE_REF=cec92165abbe6bf241fc99f73717754a2f0d4a76
ARG MLTLSAT_REF=ccd6ec667ff01ce4ceea80aa273be1923dc49359
ARG Z3_WHEEL_URL=https://github.com/Z3Prover/z3/releases/download/z3-4.15.8/z3_solver-4.15.8.0-py3-none-manylinux_2_27_x86_64.whl
ARG Z3_WHEEL_SHA256=9cd56da5d4946e5f877736386b15d8ec7616c9dac6bced1d5862a22f890ccd13

FROM debian:bookworm-slim AS experiment_sources

ARG STLTREE_REF
ARG MLTLSAT_REF
RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates git \
    && rm -rf /var/lib/apt/lists/* \
    && git clone https://github.com/michiari/stltree.git /src/stltree \
    && git -C /src/stltree checkout --detach "${STLTREE_REF}" \
    && git clone https://github.com/michiari/mltlsat.git /src/mltlsat \
    && git -C /src/mltlsat checkout --detach "${MLTLSAT_REF}" \
    && rm -rf /src/stltree/.git /src/mltlsat/.git


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
    && cp /tmp/z3-wheel/z3_solver-4.15.8.0.data/data/bin/z3 /opt/z3/bin/z3 \
    && chmod +x /opt/z3/bin/z3 \
    && mv /tmp/z3.whl /z3_solver-4.15.8.0-py3-none-manylinux_2_27_x86_64.whl

FROM rust:${RUST_VERSION}-bookworm AS stlsat_builder

RUN apt-get update \
    && apt-get install --yes --no-install-recommends clang libclang-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src/stlsat
COPY . .
COPY --from=z3_binary /opt/z3 /opt/z3
# Link against the checksum-pinned official Z3 4.15.8 wheel.
RUN Z3_SYS_Z3_HEADER=/opt/z3/include/z3.h \
    Z3_LIBRARY_PATH_OVERRIDE=/opt/z3/lib \
    cargo build --release


FROM debian:bookworm-slim AS mltlsat_builder

RUN apt-get update \
    && apt-get install --yes --no-install-recommends build-essential zlib1g-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src/mltlsat
COPY --from=experiment_sources /src/mltlsat .
RUN make -C translator/src release


FROM debian:bookworm-slim

ENV container=docker \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    PATH=/opt/z3/bin:/opt/venv/bin:${PATH} \
    LD_LIBRARY_PATH=/opt/z3/lib \
    BROWSER_PATH=/usr/bin/chromium

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        chromium \
        dbus \
        fonts-dejavu-core \
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
COPY --from=experiment_sources /src/stltree/requirements.txt /tmp/stltree-requirements.txt
COPY --from=z3_binary /z3_solver-4.15.8.0-py3-none-manylinux_2_27_x86_64.whl /tmp/
# The pinned STLTree revision locks z3-solver 4.15.3.0. Replace that one
# requirement so both Python tools use the same 4.15.8 build as STLSat.
RUN sed --in-place 's/^z3-solver==.*$/z3-solver==4.15.8.0/' /tmp/stltree-requirements.txt \
    && python3 -m venv /opt/venv \
    && pip install --no-cache-dir \
        --requirement /tmp/stlsat-requirements.txt \
        --requirement /tmp/stltree-requirements.txt \
        /tmp/z3_solver-4.15.8.0-py3-none-manylinux_2_27_x86_64.whl \
    && rm /tmp/stlsat-requirements.txt \
        /tmp/stltree-requirements.txt \
        /tmp/z3_solver-4.15.8.0-py3-none-manylinux_2_27_x86_64.whl

COPY --from=z3_binary /opt/z3 /opt/z3
COPY . /opt/stlsat
COPY --from=stlsat_builder /src/stlsat/target/release/stlsat /opt/stlsat/target/release/stlsat
COPY --from=experiment_sources /src/stltree /opt/stltree
COPY --from=experiment_sources /src/mltlsat /opt/mltlsat
COPY --from=mltlsat_builder /src/mltlsat/translator/src/MLTLConvertor /opt/mltlsat/translator/src/MLTLConvertor

RUN mkdir -p /results \
    && chmod +x \
        /opt/stlsat/scripts/*.py \
        /opt/stlsat/scripts/*.sh \
        /opt/stltree/stltree.py \
        /opt/mltlsat/translator/src/MLTLConvertor

WORKDIR /opt/stlsat/scripts

STOPSIGNAL SIGRTMIN+3
CMD ["/sbin/init"]
