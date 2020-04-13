FROM rust:1.42 AS build

# create a new empty shell project
RUN USER=root cargo new --bin kubes-cd
WORKDIR /kubes-cd

# copy over your manifests
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

# this build step will cache your dependencies
RUN cargo build --release
RUN rm src/*.rs

# copy your source tree
COPY ./src ./src

# build for release
RUN rm ./target/release/deps/kubes_cd*
RUN cargo build --release

RUN mkdir -p /build-out

RUN cp target/release/kubes-cd /build-out/

FROM ubuntu@sha256:5f4bdc3467537cbbe563e80db2c3ec95d548a9145d64453b06939c4592d67b6d

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get -y install ca-certificates libssl-dev && rm -rf /var/lib/apt/lists/*

COPY --from=build /build-out/kubes-cd /

CMD /kubes-cd
