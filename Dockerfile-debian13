# Dockerfile

# use Rust to compile our crate
FROM rust:1.89.0-slim-trixie AS build
RUN apt-get update && apt-get install -y libssl-dev pkg-config
RUN rustup update stable && rustup default stable
WORKDIR /build
COPY . /build
RUN cargo build --release

# build the final container image
FROM debian:trixie-slim AS image
WORKDIR /app
COPY --from=build /build/target/release/disk_archiver /app
COPY --from=build /build/target/release/warehouse_check /app
COPY --from=build /build/target/release/workbench /app
