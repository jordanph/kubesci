FROM rust:1.42

RUN rustup component add clippy --toolchain 1.42.0-x86_64-unknown-linux-gnu
RUN rustup component add rustfmt --toolchain 1.42.0-x86_64-unknown-linux-gnu

RUN mkdir /.cargo-cache

ENV CARGO_TARGET_DIR=/.cargo-cache

RUN USER=root cargo new --bin kubesci

WORKDIR /kubesci

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

RUN cargo build

WORKDIR /

RUN rm -r kubesci
