FROM ubuntu:24.04 AS build
RUN apt-get update && apt-get install -y curl build-essential && \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.85.0
ENV PATH="/root/.cargo/bin:${PATH}"
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
RUN cargo build --release

FROM scratch
COPY --from=build /src/target/release/diaper /diaper
