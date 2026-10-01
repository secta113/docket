# The development image (compose.yaml). The toolchain comes from rust-toolchain.toml; the base image only has to carry
# the same version, so installing adds the components and nothing else
FROM rust:1.98.1
COPY rust-toolchain.toml /tmp/toolchain/
RUN cd /tmp/toolchain && rustup toolchain install && rm -r /tmp/toolchain
