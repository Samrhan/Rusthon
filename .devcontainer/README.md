# Development Container Setup

This directory contains the development container configuration for the Rusthon Python-to-LLVM compiler project.

## Requirements

The devcontainer includes:
- **Rust toolchain** (latest stable)
- **LLVM 18** with full development libraries
- **Clang 18** compiler
- **Polly** optimization library
- **C++ standard libraries**
- **CMake** and **Git**

## LLVM Dependencies

The following LLVM 18 packages are installed:
- `llvm-18` - LLVM core
- `llvm-18-dev` - Development headers
- `llvm-18-runtime` - Runtime libraries
- `llvm-18-tools` - LLVM tools
- `libllvm18` - LLVM shared library
- `libpolly-18-dev` - Polly optimization library (required for linking)
- `clang-18` - Clang compiler
- `libclang-18-dev` - Clang development headers
- `libc++-18-dev` - C++ standard library
- `libc++abi-18-dev` - C++ ABI library

## Environment Variables

The container sets:
```bash
LLVM_SYS_181_PREFIX=/usr/lib/llvm-18
PATH=/usr/lib/llvm-18/bin:$PATH
```

## Rebuilding the Container

If you've updated the Dockerfile and need to rebuild:

### In VS Code:
1. Press `Ctrl+Shift+P` (or `Cmd+Shift+P` on Mac)
2. Select "Dev Containers: Rebuild Container"
3. Wait for the rebuild to complete

### Using Docker CLI:
```bash
# From the project root
docker build -t rusthon-dev .devcontainer/
docker run -it -v $(pwd):/workspaces/Rusthon rusthon-dev
```

## Verification

After the container is built, verify the installation:

```bash
# Check Rust
rustc --version
cargo --version

# Check LLVM
llvm-config-18 --version
clang-18 --version

# Check libraries
llvm-config-18 --libs all
```

## Building the Project

```bash
cd python-compiler
cargo build
cargo test
```

## Troubleshooting

### "could not find native static library \`Polly\`"

This means the Polly library is not installed. Ensure \`libpolly-18-dev\` is in the Dockerfile and rebuild the container.

### "LLVM_SYS_181_PREFIX not set"

The environment variable should be set automatically in the Dockerfile. If not, manually set it:
```bash
export LLVM_SYS_181_PREFIX=/usr/lib/llvm-18
```

### Library linking errors

Check that all LLVM libraries are available:
```bash
llvm-config-18 --libdir
ls -la $(llvm-config-18 --libdir)
```

## Notes

- The container uses Debian Bookworm as the base
- LLVM 18 is specifically required for the \`llvm-sys\` crate compatibility
- The \`inkwell\` crate uses LLVM 18 bindings
- All Rust tools (clippy, rustfmt) are pre-installed
