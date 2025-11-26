FROM mcr.microsoft.com/devcontainers/rust:1-bookworm

# Install LLVM 18 and development files
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends \
        llvm-18 \
        llvm-18-dev \
        llvm-18-runtime \
        llvm-18-tools \
        libllvm18 \
        libpolly-18-dev \
        clang-18 \
        libclang-18-dev \
        libc++-18-dev \
        libc++abi-18-dev \
        cmake \
        git \
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/*

# Set LLVM environment variables
ENV LLVM_SYS_181_PREFIX=/usr/lib/llvm-18
ENV PATH="/usr/lib/llvm-18/bin:${PATH}"

# Verify LLVM installation
RUN llvm-config-18 --version

# Install additional Rust tools
RUN rustup component add clippy rustfmt

# Set working directory
WORKDIR /workspaces/Rusthon
