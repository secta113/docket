# The development image (compose.yaml). The toolchain comes from rust-toolchain.toml; the base image only has to carry
# the same version, so installing adds the components and targets and nothing else
FROM rust:1.98.1
COPY rust-toolchain.toml /tmp/toolchain/
RUN cd /tmp/toolchain && rustup toolchain install && rm -r /tmp/toolchain
# Packaging: maturin and cargo-xwin from requirements-build.txt. cargo-xwin links the Windows binary with clang's
# linker, and downloads the MSVC runtime and the Windows SDK on first use (under Microsoft's license)
RUN apt-get update \
    && apt-get install -y --no-install-recommends clang lld llvm python3-venv \
    && rm -rf /var/lib/apt/lists/*
COPY requirements-build.txt /tmp/
RUN python3 -m venv /opt/build && /opt/build/bin/pip install --no-cache-dir -r /tmp/requirements-build.txt
ENV PATH=/opt/build/bin:$PATH
