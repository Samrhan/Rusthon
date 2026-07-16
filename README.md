# Rusthon

> A Python-to-LLVM compiler written in Rust that compiles a subset of Python straight to native machine code.

[![Rust CI](https://github.com/Samrhan/Rusthon/actions/workflows/rust-ci.yml/badge.svg)](https://github.com/Samrhan/Rusthon/actions/workflows/rust-ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](#license)
[![LLVM 18](https://img.shields.io/badge/LLVM-18-orange.svg)](https://llvm.org/)

Rusthon parses Python with [`rustpython-parser`](https://github.com/RustPython/RustPython), lowers it to a small custom IR, and emits [LLVM 18](https://llvm.org/) IR through [Inkwell](https://github.com/TheDan64/inkwell). The result is linked by `clang` into a standalone native executable — no interpreter, no runtime GC, no CPython.

---

## Table of Contents

- [Highlights](#highlights)
- [Quick Example](#quick-example)
- [Installation](#installation)
- [Usage](#usage)
- [Supported Python Subset](#supported-python-subset)
- [How It Works](#how-it-works)
- [Memory Model: NaN-Boxing](#memory-model-nan-boxing)
- [Project Structure](#project-structure)
- [Testing](#testing)
- [Documentation](#documentation)
- [Limitations](#limitations)
- [Contributing](#contributing)
- [License](#license)

---

## Highlights

- **Native compilation** — Python source → LLVM IR → native executable via `clang`.
- **NaN-boxed values** — every value is a single 64-bit `PyObject`, halving memory versus a tagged struct (16 → 8 bytes).
- **LLVM 18 optimization** — codegen runs the new pass manager's `default<O2>` pipeline (loop/SLP vectorization, function merging).
- **Two-pass function compilation** — signatures are declared before bodies, so mutual recursion works out of the box.
- **Dynamic typing with automatic promotion** — integers and floats mix freely; types are discriminated at runtime.
- **Rich operator support** — arithmetic, bitwise, comparison, unary, and augmented assignment.
- **Control flow** — `if`/`else`, `while`, range-based `for`, plus `break` and `continue`.
- **Heap-allocated lists** with an O(1) `len()` length header.
- **Friendly diagnostics** — parse, lowering, and codegen errors are rendered with [ariadne](https://github.com/zesterer/ariadne), pointing at the offending line and column.
- **~174 snapshot tests** covering every language feature via [insta](https://insta.rs/).

## Quick Example

```python
# fibonacci.py
def fibonacci(n):
    if n <= 1:
        return n
    return fibonacci(n - 1) + fibonacci(n - 2)

for i in range(10):
    print(fibonacci(i))
```

Compile and run it:

```bash
cd python-compiler
cargo run -- fibonacci.py   # produces fibonacci.ll and a native ./fibonacci
./fibonacci
```

```
0
1
1
2
3
5
8
13
21
34
```

## Installation

### Prerequisites

- **Rust** 1.70 or later ([rustup.rs](https://rustup.rs))
- **LLVM 18** with development headers
- **Clang** (used to link the generated LLVM IR into an executable)

### Install LLVM 18

**Ubuntu / Debian**

```bash
sudo apt-get update
sudo apt-get install -y llvm-18 llvm-18-dev llvm-18-runtime \
  libllvm18 libpolly-18-dev clang-18 libclang-18-dev libzstd-dev cmake
export LLVM_SYS_181_PREFIX=/usr/lib/llvm-18
```

**macOS (Homebrew)**

```bash
brew install llvm@18
export LLVM_SYS_181_PREFIX=$(brew --prefix llvm@18)
```

**Arch Linux**

```bash
sudo pacman -S llvm18 clang
export LLVM_SYS_181_PREFIX=/usr/lib/llvm18
```

> `LLVM_SYS_181_PREFIX` must point at your LLVM 18 install so the `inkwell`/`llvm-sys` build can find the libraries. Add it to your shell profile to make it permanent.

### Build

```bash
cd python-compiler
cargo build --release
```

### Dev Container (optional)

A ready-to-use [Dev Container](https://containers.dev/) is provided with LLVM 18 preinstalled. Open the repo in VS Code with the *Dev Containers* extension and choose **Reopen in Container** — see [`.devcontainer/README.md`](.devcontainer/README.md).

## Usage

### As a command-line compiler

The CLI takes a single `.py` file, writes the generated LLVM IR next to it, and links a native executable:

```bash
cd python-compiler
cargo run -- path/to/program.py
```

For `program.py` this produces:

- `program.ll` — the generated LLVM IR (inspect it to see what the compiler emits)
- `program` — a native, standalone executable

Then just run it:

```bash
./program
```

Ready-made samples live in [`python-compiler/examples/`](python-compiler/examples/):

```bash
cargo run -- examples/control_flow.py && ./control_flow
cargo run -- examples/strings.py && ./strings
```

### As a library

The compilation pipeline is also exposed as a library:

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

### Literals & values

| Kind | Examples |
|------|----------|
| Integers | `42`, `-7`, `1_000` (48-bit signed, ±140 trillion) |
| Floats | `3.14`, `2.5` |
| Booleans | `True`, `False` |
| Strings | `"hello"`, with escape sequences |
| Lists | `[1, 2, 3]`, indexed with `xs[i]` |

### Operators

- **Arithmetic:** `+` `-` `*` `/` `%`
- **Bitwise:** `&` `|` `^` `<<` `>>`
- **Comparison:** `==` `!=` `<` `>` `<=` `>=`
- **Unary:** `-x` `+x` `~x` `not x`
- **Augmented assignment:** `+=` `-=` `*=` `/=` `%=` `&=` `|=` `^=` `<<=` `>>=` (desugared to the matching binary op)

### Statements

```python
x = 10                      # assignment
y = x + 5

if x > y:                   # if / else
    print("bigger")
else:
    print("smaller")

while x > 0:                # while loop
    x -= 1

for i in range(2, 8):       # range-based for  (range(end) or range(start, end))
    if i == 5:
        continue            # continue
    if i == 7:
        break               # break
    print(i)

def scale(value, factor=2): # functions with default arguments
    return value * factor

print(scale(21))            # print (accepts multiple arguments)
print(len([1, 2, 3]))       # built-in len()
name = input()              # read a value from stdin
```

Supported built-ins: `print(...)`, `input()`, `len(...)`, and `range(...)` (inside `for`). Functions support recursion, mutual recursion, multiple parameters, and default arguments.

## How It Works

Rusthon follows a classic multi-pass pipeline:

```
Python Source
     │  parser.rs            (rustpython-parser)
     ▼
Python AST
     │  lowering.rs          (AST → custom IR)
     ▼
Custom IR  (ast.rs: IRExpr / IRStmt)
     │  codegen.rs           (Inkwell / LLVM 18)
     ▼
LLVM IR
     │  LLVM new pass manager: default<O2>
     ▼
Optimized LLVM IR
     │  clang -lm
     ▼
Native executable
```

1. **Parse** — `rustpython-parser` turns source into a Python AST.
2. **Lower** — the AST is desugared into a small, explicit IR (`IRExpr`, `IRStmt`). Augmented assignments become plain binary ops; `for i in range(...)` becomes a bounded loop.
3. **Codegen** — a two-pass walk over the IR emits LLVM IR through Inkwell. Pass 1 declares every function signature; pass 2 fills in the bodies, which is what makes mutual recursion possible.
4. **Optimize** — the module is run through LLVM's `default<O2>` pipeline via `Module::run_passes`.
5. **Link** — the CLI shells out to `clang` to produce a native executable.

## Memory Model: NaN-Boxing

Every runtime value is a single `i64` (`PyObject`). Floats are stored as canonical IEEE-754 doubles; every other type is encoded inside the payload of a quiet NaN, using a 3-bit tag and a 48-bit payload.

```text
Floats: [ sign ][  exponent  ][            mantissa            ]
        [1 bit ][   11 bits   ][            52 bits             ]

Tagged: [1][11111111111][1][ tag (3 bits) ][   payload (48 bits)   ]
         ▲       ▲        ▲        ▲                  ▲
      sign   NaN exponent │     type tag        value / pointer
                     quiet-NaN bit
```

| Tag | Type | Payload |
|-----|------|---------|
| `0` | Integer | 48-bit signed value (±140 trillion) |
| `1` | Boolean | 1-bit value |
| `2` | String | 48-bit pointer |
| `3` | List | 48-bit pointer |
| — | Float | stored directly as `f64` |

This cuts each value from 16 bytes (tag + payload struct) to 8, keeps values cache-friendly, and reduces float type checks to a single bit test. Lists are heap-allocated with a length header at offset 0, so `len()` is O(1). See [`docs/architecture/`](docs/architecture/) for the full write-up.

## Project Structure

```
Rusthon/
├── python-compiler/
│   ├── src/
│   │   ├── main.rs            # CLI entry point (file in → .ll + executable out)
│   │   ├── lib.rs            # Library exports
│   │   ├── parser.rs         # Python parsing (wraps rustpython-parser)
│   │   ├── ast.rs            # Custom IR: IRExpr, IRStmt, BinOp, CmpOp, UnaryOp
│   │   ├── lowering.rs       # Python AST → IR
│   │   ├── codegen.rs        # IR → LLVM IR (NaN-boxing, two-pass, O2)
│   │   ├── tagged_pointer.rs # NaN-boxing helpers
│   │   ├── compiler/         # Codegen internals: generators, runtime, values
│   │   └── error.rs          # Ariadne-based diagnostics
│   ├── tests/                # ~174 snapshot tests (one file per feature)
│   └── examples/             # Sample Python programs
├── docs/                     # Architecture, getting started, limitations, roadmap
└── CLAUDE.md                 # Contributor / agent guide
```

## Testing

Tests use snapshot testing with [insta](https://insta.rs/): each test compiles a Python snippet and asserts on the generated LLVM IR.

```bash
cd python-compiler

# Run the whole suite
cargo test

# Run a single feature's tests
cargo test --test control_flow

# Review pending snapshot changes
cargo insta review

# Accept snapshot changes (only when the new output is correct)
cargo insta test --accept
```

Before opening a PR, mirror what CI checks:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## Documentation

More detailed docs live in [`docs/`](docs/):

- [Getting Started](docs/getting-started/) — install and first program
- [Architecture](docs/architecture/) — compilation, memory model, optimizations, types
- [Language Features](docs/language-features/) — full feature reference
- [Limitations](docs/limitations.md) — what isn't supported (and why)
- [Roadmap](docs/roadmap.md) — planned work
- [Testing](docs/testing/) — how to run and write tests

## Limitations

Rusthon is an educational compiler for a deliberately restricted subset of Python. It does **not** currently support:

- Classes, objects, and methods
- Dictionaries and tuples
- `elif` chains (use nested `if`/`else`)
- List comprehensions, generators, and lambdas
- Exceptions (`try`/`except`)
- Modules and imports
- String concatenation and string methods
- Loops with a `range` step argument (`range(0, 10, 2)`)

Strings and lists are heap-allocated and freed conservatively, so long-running programs that allocate heavily may leak. See [`docs/limitations.md`](docs/limitations.md) for the complete list and workarounds.

## Contributing

Contributions are welcome!

1. Fork the repository and create a feature branch.
2. Make your changes **with tests** (add or update snapshot tests for new behavior).
3. Make sure `cargo fmt`, `cargo clippy`, and `cargo test` all pass.
4. Open a pull request describing the change.

See [`CLAUDE.md`](CLAUDE.md) for a deep dive into the architecture, coding conventions, and common tasks (adding an operator, adding a statement type, updating snapshots, etc.).

## License

Rusthon is released under the [MIT License](https://opensource.org/licenses/MIT).

## Acknowledgements

Built on the shoulders of:

- [Inkwell](https://github.com/TheDan64/inkwell) — safe Rust bindings to LLVM
- [rustpython-parser](https://github.com/RustPython/RustPython) — the Python parser
- [ariadne](https://github.com/zesterer/ariadne) — beautiful diagnostics
- [insta](https://insta.rs/) — snapshot testing

Inspired by prior art in Python compilation such as [Numba](https://numba.pydata.org/), [Codon](https://github.com/exaloop/codon), and the [LLVM Kaleidoscope](https://llvm.org/docs/tutorial/) tutorial.
