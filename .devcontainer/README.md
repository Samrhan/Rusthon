# Development Container for Rusthon

This directory contains the development container configuration for the Rusthon Python-to-LLVM compiler project.

## Prerequisites

- [Docker](https://www.docker.com/get-started)
- [Visual Studio Code](https://code.visualstudio.com/)
- [Remote - Containers extension](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers)

## What's Included

The development container includes:

- **Rust toolchain**: Latest stable Rust with `clippy` and `rustfmt`
- **LLVM 18**: Full LLVM 18 installation with development headers
- **Clang 18**: C/C++ compiler for LLVM wrapper compilation
- **Build tools**: CMake, Git, and other essential development tools

## Using the Development Container

### Option 1: VS Code (Recommended)

1. Open the Rusthon repository in VS Code
2. When prompted, click "Reopen in Container"
   - Or use Command Palette (F1) → "Remote-Containers: Reopen in Container"
3. Wait for the container to build (first time only, takes 2-5 minutes)
4. Start developing!

### Option 2: Manual Docker Build

```bash
# From the repository root
cd .devcontainer
docker build -t rusthon-dev .
docker run -it -v $(pwd)/..:/workspaces/Rusthon rusthon-dev
```

## Building the Project

Once inside the container:

```bash
cd python-compiler
cargo build
cargo test
```

## Environment Variables

The following environment variables are pre-configured:

- `LLVM_SYS_181_PREFIX=/usr/lib/llvm-18` - LLVM installation prefix
- `PATH` includes `/usr/lib/llvm-18/bin` - LLVM tools in PATH

## Troubleshooting

### LLVM not found

If you see errors about missing LLVM headers, verify the installation:

```bash
llvm-config-18 --version
ls -la /usr/lib/llvm-18/include/llvm-c/
```

### Container won't build

Try rebuilding without cache:

```bash
docker build --no-cache -t rusthon-dev .
```

## Local Development (Without Container)

If you prefer to develop locally, install these dependencies:

### Ubuntu/Debian

```bash
sudo apt-get update
sudo apt-get install -y llvm-18 llvm-18-dev llvm-18-runtime \
    libllvm18 clang-18 libclang-18-dev cmake git
export LLVM_SYS_181_PREFIX=/usr/lib/llvm-18
```

### macOS

```bash
brew install llvm@18
export LLVM_SYS_181_PREFIX=$(brew --prefix llvm@18)
```

### Arch Linux

```bash
sudo pacman -S llvm18 clang cmake git
export LLVM_SYS_181_PREFIX=/usr/lib/llvm18
```
