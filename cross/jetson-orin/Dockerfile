FROM ubuntu:22.04

RUN apt-get update && apt-get install -y \
    build-essential \
    ca-certificates \
    curl \
    gcc-aarch64-linux-gnu \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}"
RUN rustup target add aarch64-unknown-linux-gnu

WORKDIR /workspace

