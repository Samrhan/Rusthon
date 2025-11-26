# Installation

This guide covers installing Rusthon and its dependencies on various platforms.

## Using Docker Devcontainer (Recommended)

The simplest way to use Rusthon is with the provided devcontainer:

```bash
git clone https://github.com/Samrhan/Rusthon.git
cd Rusthon
```

Then open in VS Code with the Remote-Containers extension and select "Reopen in Container".

All dependencies are pre-installed and configured.

## Manual Installation

### Ubuntu / Debian

```bash
# Install system dependencies
sudo apt-get update
sudo apt-get install -y \
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
    git

# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Set environment variables
export LLVM_SYS_181_PREFIX=/usr/lib/llvm-18
export PATH="/usr/lib/llvm-18/bin:${PATH}"

# Add to ~/.bashrc or ~/.zshrc for persistence
echo 'export LLVM_SYS_181_PREFIX=/usr/lib/llvm-18' >> ~/.bashrc
echo 'export PATH="/usr/lib/llvm-18/bin:${PATH}"' >> ~/.bashrc

# Clone and build
git clone https://github.com/Samrhan/Rusthon.git
cd Rusthon/python-compiler
cargo build --release
```

### macOS

```bash
# Install Homebrew (if not already installed)
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install dependencies
brew install llvm@18 rust cmake

# Set environment variables
export LLVM_SYS_181_PREFIX="$(brew --prefix llvm@18)"
export PATH="$(brew --prefix llvm@18)/bin:${PATH}"

# Add to ~/.zshrc for persistence
echo 'export LLVM_SYS_181_PREFIX="$(brew --prefix llvm@18)"' >> ~/.zshrc
echo 'export PATH="$(brew --prefix llvm@18)/bin:${PATH}"' >> ~/.zshrc

# Clone and build
git clone https://github.com/Samrhan/Rusthon.git
cd Rusthon/python-compiler
cargo build --release
```

### Arch Linux

```bash
# Install dependencies
sudo pacman -S llvm clang rust cmake git

# Set environment variables
export LLVM_SYS_181_PREFIX=/usr
export PATH="/usr/bin:${PATH}"

# Clone and build
git clone https://github.com/Samrhan/Rusthon.git
cd Rusthon/python-compiler
cargo build --release
```

## Verifying Installation

After installation, verify everything works:

```bash
# Check Rust
rustc --version
cargo --version

# Check LLVM
llvm-config-18 --version  # or llvm-config on some systems

# Check Clang
clang-18 --version  # or clang on some systems

# Build Rusthon
cd python-compiler
cargo build

# Run tests
cargo test
```

If all tests pass, you're ready to go!

## Troubleshooting

### LLVM Headers Not Found

If you see errors about missing LLVM headers:

```bash
# Ensure llvm-dev package is installed
sudo apt-get install llvm-18-dev libclang-18-dev

# Verify LLVM_SYS_181_PREFIX is set correctly
echo $LLVM_SYS_181_PREFIX
llvm-config-18 --prefix  # Should match LLVM_SYS_181_PREFIX
```

### Clang Not Found at Runtime

If compilation succeeds but running fails:

```bash
# Ensure clang-18 is in PATH
which clang-18

# Or create a symlink
sudo ln -s /usr/bin/clang-18 /usr/bin/clang
```

### Cargo Build Fails

```bash
# Clean and rebuild
cargo clean
cargo build

# Update Rust toolchain
rustup update stable
```

## Next Steps

- [Quick Start](/getting-started/quick-start) - Compile your first program
- [Your First Program](/getting-started/your-first-program) - Learn the language
