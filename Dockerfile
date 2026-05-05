FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y \
    build-essential \
    cmake \
    git \
    curl \
    clang \
    libclang-dev \
    libsqlite3-dev \
    python3 \
    && rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/usr/local/rustup
ENV CARGO_HOME=/usr/local/cargo
ENV PATH=/usr/local/cargo/bin:$PATH
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

WORKDIR /cFS
COPY . /cFS

RUN mkdir -p /cFS/build/leodos/tables/staging && \
    MISSIONCONFIG=leodos make O=build/leodos SIMULATION=native prep && \
    MISSIONCONFIG=leodos make O=build/leodos SIMULATION=native install

WORKDIR /cFS/build/leodos/exe/cpu1

CMD ["/bin/bash"]
