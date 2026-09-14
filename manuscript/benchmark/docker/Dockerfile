FROM ubuntu:24.04 AS builder

ARG DEBIAN_FRONTEND=noninteractive
ARG BEDDER_REF
ARG BEDTOOLS_REF
ARG BEDTK_REF
ARG AILIST_REF
ARG COITREES_REF
ARG BEDOPS_VERSION
ARG RUST_VERSION

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    ca-certificates \
    clang \
    cmake \
    curl \
    git \
    libbz2-dev \
    libcurl4-openssl-dev \
    liblzma-dev \
    libssl-dev \
    libz-dev \
    pkg-config \
    python3-dev \
    xz-utils \
    && rm -rf /var/lib/apt/lists/*

ENV CARGO_HOME=/opt/cargo
ENV RUSTUP_HOME=/opt/rustup
ENV PATH=/opt/cargo/bin:${PATH}

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --no-modify-path --profile minimal --default-toolchain "$RUST_VERSION"

RUN git clone https://github.com/quinlan-lab/bedder-rs.git /src/bedder \
    && git -C /src/bedder checkout "$BEDDER_REF" \
    && cargo build --manifest-path /src/bedder/Cargo.toml --release \
    && mkdir -p /stage/bin /stage/provenance \
    && cp /src/bedder/target/release/bedder /stage/bin/bedder \
    && cp /src/bedder/Cargo.lock /stage/provenance/bedder-Cargo.lock

RUN git clone https://github.com/arq5x/bedtools2.git /src/bedtools2 \
    && git -C /src/bedtools2 checkout "$BEDTOOLS_REF" \
    && make -C /src/bedtools2 -j2 \
    && cp /src/bedtools2/bin/bedtools /stage/bin/bedtools

RUN git clone https://github.com/lh3/bedtk.git /src/bedtk \
    && git -C /src/bedtk checkout "$BEDTK_REF" \
    && make -C /src/bedtk -j2 \
    && cp /src/bedtk/bedtk /stage/bin/bedtk

RUN git clone https://github.com/databio/AIList.git /src/AIList \
    && git -C /src/AIList checkout "$AILIST_REF" \
    && make -C /src/AIList ailist \
    && cp /src/AIList/bin/ailist /stage/bin/ailist

RUN git clone https://github.com/dcjones/coitrees.git /src/coitrees \
    && git -C /src/coitrees checkout "$COITREES_REF" \
    && RUSTFLAGS='-C target-cpu=x86-64-v3' \
       cargo build --manifest-path /src/coitrees/Cargo.toml --release --example bed-intersect \
    && cp /src/coitrees/target/release/examples/bed-intersect /stage/bin/coitrees-bed-intersect \
    && cp /src/coitrees/Cargo.lock /stage/provenance/coitrees-Cargo.lock

# The upstream asset has a .tar.bz2 name but is an uncompressed tar.
RUN curl -fsSL \
      "https://github.com/bedops/bedops/releases/download/v${BEDOPS_VERSION}/bedops_linux_x86_64-v${BEDOPS_VERSION}.tar.bz2" \
      -o /tmp/bedops.tar.bz2 \
    && echo '3a8b4961ff80e201e2a286b91c7a0ee8fff6387ab44c3fc07c3765fdf9aedfef  /tmp/bedops.tar.bz2' \
      | sha256sum --check - \
    && mkdir -p /tmp/bedops \
    && tar -xf /tmp/bedops.tar.bz2 -C /tmp/bedops \
    && cp /tmp/bedops/bin/bedops /stage/bin/bedops \
    && cp /tmp/bedops/bin/bedmap /stage/bin/bedmap \
    && cp /tmp/bedops/bin/sort-bed /stage/bin/sort-bed

RUN printf '%s\n' \
      "bedder=${BEDDER_REF}" \
      "bedtools=${BEDTOOLS_REF}" \
      "bedtk=${BEDTK_REF}" \
      "ailist=${AILIST_REF}" \
      "coitrees=${COITREES_REF}" \
      "bedops=${BEDOPS_VERSION}" \
      "rust=${RUST_VERSION}" \
      > /stage/provenance/source-revisions.txt

FROM ubuntu:24.04

ARG DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    bcftools \
    ca-certificates \
    coreutils \
    gawk \
    libbz2-1.0 \
    libgcc-s1 \
    liblzma5 \
    libpython3.12 \
    libstdc++6 \
    python3 \
    tabix \
    time \
    zlib1g \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /stage/bin/ /usr/local/bin/
COPY --from=builder /stage/provenance/ /opt/bedder-benchmark/provenance/

ENV LC_ALL=C
ENV LANG=C
ENV OMP_NUM_THREADS=1
ENV OPENBLAS_NUM_THREADS=1
ENV MKL_NUM_THREADS=1
ENV RAYON_NUM_THREADS=1

WORKDIR /workspace/manuscript/benchmark
