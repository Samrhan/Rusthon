# Python to LLVM Compiler

A Python-to-LLVM compiler written in Rust, implementing a subset of Python that compiles to native code via LLVM.

## Features

- ✅ Integer arithmetic (`+`, `-`, `*`, `/`)
- ✅ Floating-point numbers and mixed arithmetic
- ✅ Variables and assignments
- ✅ Function definitions with parameters
- ✅ Function calls
- ✅ Return statements
- ✅ Print statements
- ✅ Input from stdin (`input()`)
- ✅ Detailed error messages with line/column information
- 🚧 More features coming soon...

## Quick Start

### Using Development Container (Recommended)

The easiest way to get started is using the provided development container:

1. Install [VS Code](https://code.visualstudio.com/) and [Docker](https://www.docker.com/get-started)
2. Install the [Remote - Containers extension](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers)
3. Open the repository in VS Code
4. Click "Reopen in Container" when prompted
5. Build and test:
   ```bash
   cargo build
   cargo test
   ```

See [.devcontainer/README.md](../.devcontainer/README.md) for more details.

### Local Development

#### Prerequisites

- Rust 1.70 or later
- LLVM 18 with development headers
- Clang (for building LLVM wrappers)

#### Installation

**Ubuntu/Debian:**
```bash
sudo apt-get update
sudo apt-get install -y llvm-18 llvm-18-dev clang-18 libclang-18-dev
export LLVM_SYS_181_PREFIX=/usr/lib/llvm-18
```

**macOS:**
```bash
brew install llvm@18
export LLVM_SYS_181_PREFIX=$(brew --prefix llvm@18)
```

**Arch Linux:**
```bash
sudo pacman -S llvm18 clang
export LLVM_SYS_181_PREFIX=/usr/lib/llvm18
```

#### Building

```bash
cargo build
```

#### Running Tests

```bash
cargo test
```

## Usage

```rust
use inkwell::context::Context;
use python_compiler::*;

let source = r#"
def add(a, b):
    return a + b

x = add(5, 3)
print(x)
"#;

// Parse Python source
let ast = parser::parse_program(source).unwrap();

// Lower to IR
let ir = lowering::lower_program(&ast).unwrap();

// Generate LLVM IR
let context = Context::create();
let compiler = codegen::Compiler::new(&context);
let llvm_ir = compiler.compile_program(&ir).unwrap();

println!("{}", llvm_ir);
```

## Project Structure

```
python-compiler/
├── src/
│   ├── ast.rs          # Intermediate Representation definitions
│   ├── codegen.rs      # LLVM IR code generation
│   ├── compiler.rs     # Compiler orchestration
│   ├── lowering.rs     # Python AST → IR lowering
│   ├── parser.rs       # Python parsing (wraps rustpython-parser)
│   ├── error.rs        # Error reporting with ariadne
│   ├── lib.rs          # Library exports
│   └── main.rs         # CLI entry point
├── tests/
│   ├── arithmetic.rs   # Arithmetic operation tests
│   ├── variables.rs    # Variable assignment tests
│   ├── functions.rs    # Function definition and call tests
│   ├── floats.rs       # Floating-point and mixed arithmetic tests
│   ├── input.rs        # Input from stdin tests
│   ├── errors.rs       # Error handling tests
│   └── integration.rs  # Integration tests combining all features
└── Cargo.toml
```

## Supported Python Subset

### Expressions

- Integer literals: `42`, `100`
- Float literals: `3.14`, `2.5`
- Variables: `x`, `my_var`
- Binary operations: `a + b`, `x * y`, `a - b`, `x / y`
- Function calls: `add(1, 2)`, `compute(x, y, z)`
- Input calls: `input()`

### Statements

- Assignments: `x = 10`, `y = x + 5`, `z = input()`
- Function definitions:
  ```python
  def func_name(param1, param2):
      return param1 + param2
  ```
- Return statements: `return x + y`
- Print statements: `print(x)`

### Limitations

- All numeric values are promoted to 64-bit floats (f64) for mixed arithmetic
- The `input()` function reads floating-point numbers from stdin
- No support for:
  - Strings and booleans
  - Lists, tuples, dictionaries
  - Classes and objects
  - Control flow (if/else, loops)
  - Exceptions
  - Modules and imports
  - Multiple arguments to `print()` or `input()`

## Architecture

The compiler follows a traditional multi-pass architecture:

```
Python Source → Parser → Python AST → Lowering → IR → Code Generation → LLVM IR
                  ↓                      ↓              ↓                   ↓
           rustpython-parser     Custom IR AST    Inkwell         Native Code
```

### Key Design Decisions

1. **Float-first Types**: All numeric values are promoted to f64 for simplicity and mixed arithmetic
2. **Custom IR**: A simplified intermediate representation between Python AST and LLVM
3. **Inkwell**: Safe Rust bindings to LLVM for code generation
4. **Function-first**: Functions are compiled before top-level code
5. **FFI for I/O**: Direct calls to libc `printf` and `scanf` for input/output
6. **Ariadne Error Reporting**: Beautiful error messages with source location context

## Testing

The project uses snapshot testing with [insta](https://insta.rs/):

```bash
# Run all tests
cargo test

# Review snapshots
cargo insta review

# Update snapshots
cargo insta accept
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes with tests
4. Ensure `cargo test` and `cargo clippy` pass
5. Submit a pull request

## License

This project is part of the Rusthon compiler project.

## References

- [Inkwell Documentation](https://thedan64.github.io/inkwell/)
- [LLVM Language Reference](https://llvm.org/docs/LangRef.html)
- [RustPython Parser](https://github.com/RustPython/RustPython/tree/main/parser)
- [Kaleidoscope Tutorial](https://llvm.org/docs/tutorial/)
