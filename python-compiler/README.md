# python-compiler

The compiler crate behind [Rusthon](../README.md) — it parses a subset of Python, lowers it to a small custom IR, and emits [LLVM 18](https://llvm.org/) IR (via [Inkwell](https://github.com/TheDan64/inkwell)) that `clang` links into a native executable.

> This is the crate-level README. For the project overview, memory model, and full docs see the [root README](../README.md) and [`docs/`](../docs/).

## Features

- Integer, float, boolean, string, and list literals
- Arithmetic (`+ - * / %`), bitwise (`& | ^ << >>`), comparison (`== != < > <= >=`), and unary (`- + ~ not`) operators
- Augmented assignment (`+=`, `-=`, `*=`, …)
- Variables and assignment
- `if`/`else`, `while`, and range-based `for` loops, with `break` and `continue`
- Function definitions with default arguments, recursion, and mutual recursion
- Built-ins: `print(...)` (multiple args), `input()`, `len(...)`, `range(...)` (in `for`)
- NaN-boxed values (single 8-byte `i64` PyObject) and an LLVM `default<O2>` optimization pass
- Detailed error messages with line/column information (via [ariadne](https://github.com/zesterer/ariadne))

## Prerequisites

- Rust 1.70 or later
- LLVM 18 with development headers
- Clang (used to link the generated LLVM IR)

### Install LLVM 18

**Ubuntu/Debian**
```bash
sudo apt-get update
sudo apt-get install -y llvm-18 llvm-18-dev clang-18 libclang-18-dev libpolly-18-dev libzstd-dev
export LLVM_SYS_181_PREFIX=/usr/lib/llvm-18
```

**macOS**
```bash
brew install llvm@18
export LLVM_SYS_181_PREFIX=$(brew --prefix llvm@18)
```

**Arch Linux**
```bash
sudo pacman -S llvm18 clang
export LLVM_SYS_181_PREFIX=/usr/lib/llvm18
```

A ready-to-use dev container is also provided — see [`.devcontainer/README.md`](../.devcontainer/README.md).

## Build & Test

```bash
cargo build            # or: cargo build --release
cargo test
```

## Usage

### Command-line compiler

The CLI takes a single `.py` file, writes the generated LLVM IR next to it, and links a native executable:

```bash
cargo run -- examples/control_flow.py   # produces control_flow.ll and ./control_flow
./control_flow
```

For `program.py` this produces `program.ll` (the generated LLVM IR) and `program` (a standalone native executable).

### As a library

```rust
use inkwell::context::Context;
use python_compiler::{parser, lowering, codegen};

let source = r#"
def add(a, b):
    return a + b

print(add(5, 3))
"#;

let ast = parser::parse_program(source).unwrap();      // Python source -> Python AST
let ir = lowering::lower_program(&ast).unwrap();       // Python AST   -> custom IR
let context = Context::create();
let compiler = codegen::Compiler::new(&context);
let llvm_ir = compiler.compile_program(&ir).unwrap();  // IR           -> LLVM IR

println!("{llvm_ir}");
```

## Supported Python Subset

### Expressions

- Integer literals: `42`, `100` (48-bit signed, ±140 trillion)
- Float literals: `3.14`, `2.5`
- Boolean literals: `True`, `False`
- String literals: `"hello"` (with escape sequences)
- List literals and indexing: `[1, 2, 3]`, `xs[0]`
- Variables: `x`, `my_var`
- Binary, comparison, and unary operations
- Function calls: `add(1, 2)`, `compute(x, y, z)`
- Built-in calls: `input()`, `len(xs)`

### Statements

```python
x = 10                      # assignment / augmented assignment (x += 5)

if x > 5:                   # if / else
    print("big")
else:
    print("small")

while x > 0:                # while loop
    x -= 1

for i in range(2, 8):       # range-based for  (range(end) or range(start, end))
    if i == 5:
        continue            # continue / break
    print(i)

def scale(value, factor=2): # functions with default arguments
    return value * factor

print(scale(21))            # print (accepts multiple arguments)
```

### Limitations

- All values are NaN-boxed into a single 64-bit `PyObject`; integers are 48-bit signed.
- `input()` reads a floating-point number from stdin.
- `range()` in `for` loops does not accept a step argument.
- No `elif`, classes, dictionaries, tuples, list comprehensions, generators, exceptions, or modules/imports.

See [`docs/limitations.md`](../docs/limitations.md) for the full list and workarounds.

## Project Structure

```
python-compiler/
├── src/
│   ├── main.rs                        # CLI entry point (file in -> .ll + executable out)
│   ├── lib.rs                         # Library exports
│   ├── parser.rs                      # Python parsing (wraps rustpython-parser)
│   ├── ast.rs                         # Custom IR: IRExpr, IRStmt, BinOp, CmpOp, UnaryOp
│   ├── lowering.rs                    # Python AST -> IR
│   ├── codegen.rs                     # Compiler driver, two-pass orchestration
│   ├── compiler/
│   │   ├── values.rs                  # NaN-boxing value system (ValueManager)
│   │   ├── runtime.rs                 # Runtime intrinsics & printf/scanf format strings
│   │   └── generators/
│   │       ├── expression.rs          # Expression codegen
│   │       └── statement.rs           # Statement codegen
│   ├── tagged_pointer.rs              # NaN-boxing reference implementation + unit tests
│   └── error.rs                       # Ariadne-based diagnostics
├── tests/                             # Snapshot tests (one file per feature area)
├── examples/                          # Sample Python programs
└── Cargo.toml
```

## Architecture

```
Python Source -> Parser -> Python AST -> Lowering -> IR -> CodeGen -> LLVM IR -> default<O2> -> Native
                   (rustpython-parser)   (custom IR)      (Inkwell)                              (clang)
```

Key design points:

1. **NaN-boxing** — every value is a single `i64`. Floats are stored directly; ints, bools, strings, and lists are packed into the payload of a quiet NaN with a 3-bit type tag.
2. **Custom IR** — a small, explicit intermediate representation sits between the Python AST and LLVM.
3. **Two-pass function compilation** — all signatures are declared before any body is compiled, which enables mutual recursion.
4. **FFI for I/O and memory** — direct calls to libc (`printf`, `scanf`, `malloc`, `memcpy`, `strlen`, `free`).
5. **Ariadne diagnostics** — parse, lowering, and codegen errors point at the offending source location.

See [`docs/architecture/`](../docs/architecture/) for the full write-up.

## Testing

Tests use snapshot testing with [insta](https://insta.rs/): each test compiles a Python snippet and asserts on the generated LLVM IR.

```bash
cargo test                              # run all tests
cargo test --test control_flow          # run one feature's tests
cargo insta review                      # review pending snapshot changes
cargo insta test --accept               # accept snapshot changes (only when correct)
```

## Contributing

1. Fork the repository and create a feature branch.
2. Make your changes with tests.
3. Ensure `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test` all pass.
4. Open a pull request.

See [`CLAUDE.md`](../CLAUDE.md) for a detailed developer guide (architecture, coding conventions, and common tasks).

## References

- [Inkwell Documentation](https://thedan64.github.io/inkwell/)
- [LLVM Language Reference](https://llvm.org/docs/LangRef.html)
- [RustPython Parser](https://github.com/RustPython/RustPython/tree/main/parser)
- [Kaleidoscope Tutorial](https://llvm.org/docs/tutorial/)
