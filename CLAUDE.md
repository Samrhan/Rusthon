# Rusthon Developer Guide

This document is the developer/contributor guide for the Rusthon Python-to-LLVM compiler. It explains the architecture, where things live, and how to build, test, and extend the compiler.

## Table of Contents

- [Project Overview](#project-overview)
- [Architecture](#architecture)
- [File Structure](#file-structure)
- [Dependencies](#dependencies)
- [Development Workflow](#development-workflow)
- [Code Documentation Guidelines](#code-documentation-guidelines)
- [Testing Guidelines](#testing-guidelines)
- [Common Tasks](#common-tasks)
- [Troubleshooting](#troubleshooting)

---

## Project Overview

**Rusthon** is a Python-to-LLVM compiler written in Rust that compiles a subset of Python to native code using LLVM 18. Key features:

- **NaN-boxing**: single 64-bit `PyObject` (8 bytes instead of a 16-byte tagged struct)
- **LLVM 18 new pass manager**: `default<O2>` optimization pipeline with vectorization
- **Mutual recursion support**: two-pass compilation
- **Type system**: runtime type checking with tagged values
- **Integer range**: ±140 trillion (48-bit signed integers)

**Location**: `/home/user/Rusthon/python-compiler/`

---

## Architecture

### Compilation Pipeline

```
Python Source → Parser → AST → Lowering → IR → CodeGen → LLVM IR → Optimization → Native Code
```

#### 1. **Parser** (`parser.rs`)
- Uses `rustpython-parser` to parse Python source
- Produces `rustpython_parser::ast` (Python AST)

#### 2. **Lowering** (`lowering.rs`)
- Converts Python AST → custom IR
- Handles:
  - Expression statements
  - Augmented assignments (desugar to binary ops)
  - Control flow (if/while/for)
  - Function definitions
- **Key types**: `IRExpr`, `IRStmt`

#### 3. **CodeGen** (`codegen.rs` + `compiler/`)
- Converts IR → LLVM IR
- **NaN-boxing**: all values are `i64` (PyObject)
- **Two-pass compilation**:
  - Pass 1: declare all function signatures
  - Pass 2: compile function bodies (enables mutual recursion)
- **String cleanup**: only tracks strings allocated in the main entry block
- Codegen is split across modules:
  - `codegen.rs` — the `Compiler` driver and two-pass orchestration
  - `compiler/arrayness.rs` — interprocedural analysis flowing NumPy array-ness through function params/returns
  - `compiler/values.rs` — the NaN-boxing value system (`ValueManager`)
  - `compiler/runtime.rs` — external C functions and format strings
  - `compiler/generators/expression.rs` — expression compilation
  - `compiler/generators/statement.rs` — statement compilation
  - `compiler/generators/module.rs` — module/method/attribute dispatch (e.g. `np.array`, `arr.sum()`)
  - `compiler/generators/ndarray.rs` — NumPy `ndarray` codegen (unboxed float64 buffers, element-wise loops)

#### 4. **Optimization** (LLVM 18 new pass manager)
- Pipeline: `default<O2>`
- Features: loop vectorization, SLP vectorization, function merging
- Method: `Module::run_passes()`

### Memory Model

#### NaN-Boxing Layout (64-bit)

```text
Floats: [  sign  ][  exponent  ][        mantissa        ]
        [ 1 bit  ][ 11 bits    ][     52 bits            ]

Tagged: [1][11111111111][1][ tag (3 bits) ][ payload (48 bits) ]
         ^      ^         ^       ^                 ^
         |      |         |       |                 +-- Value/pointer
         |      |         |       +-- Type tag
         |      |         +-- Quiet NaN bit
         |      +-- All ones (NaN exponent)
         +-- Sign bit
```

**Type Tags**:
- `TAG_INT = 0`: signed 48-bit integer
- `TAG_BOOL = 1`: boolean (1-bit payload)
- `TAG_STRING = 2`: string pointer (48-bit)
- `TAG_LIST = 3`: list pointer (48-bit)
- `TAG_ARRAY = 4`: NumPy `ndarray` pointer (48-bit); detected via `ValueManager::is_array`
- Floats: no tag (stored as canonical float64)

**Constants** (`compiler/values.rs`):
```rust
const QNAN: u64 = 0x7FF8_0000_0000_0000;
const TAG_MASK: u64 = 0x0007_0000_0000_0000;
const PAYLOAD_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;
```

> `src/tagged_pointer.rs` holds a standalone, unit-tested reference implementation of the same NaN-boxing scheme. The constants used by codegen live in `compiler/values.rs`.

### List Memory Layout

Lists are heap-allocated with a **length header** for O(1) `len()` operations:

```text
Memory Layout: [length: i64][element_0: i64][element_1: i64]...[element_n: i64]

Example - 3-element list [10, 20, 30]:
Offset:  0         1           2           3
Value:   3         10          20          30
         ^length   ^elem[0]    ^elem[1]    ^elem[2]
```

**Implementation Details**:
- **Allocation**: `malloc((n + 1) * sizeof(i64))` where n = number of elements
- **Length storage**: offset 0 contains the list length as i64
- **Element storage**: elements start at offset 1
- **Indexing**: `list[i]` accesses offset `i + 1` to skip the length header
- **len() operation**: reads i64 at offset 0 (O(1) time)
- **Memory overhead**: +8 bytes per list (1 i64 header)

**Code locations**:
- List allocation & indexing (`IRExpr::List`, `IRExpr::Index`): `compiler/generators/expression.rs`
- `len()` for lists (`IRExpr::Len`): `compiler/generators/expression.rs`
- Length extraction (`extract_list_ptr_and_len`): `compiler/values.rs`

---

## File Structure

### Source Files (`python-compiler/src/`)

| File | Purpose | Key Functions/Types |
|------|---------|---------------------|
| `lib.rs` | Library entry point | Exports all public modules |
| `main.rs` | CLI entry point | Argument parsing, file I/O, invokes `clang` |
| `parser.rs` | Python parsing | `parse_program()` |
| `ast.rs` | IR definitions | `IRExpr`, `IRStmt`, `BinOp`, `CmpOp`, `UnaryOp` |
| `lowering.rs` | AST → IR | `lower_program()`, `lower_statement()`, `lower_expression()` |
| `codegen.rs` | Compilation driver | `Compiler`, `compile_program()`, two-pass orchestration |
| `compiler/mod.rs` | Codegen submodule root | Re-exports arrayness/generators/runtime/values |
| `compiler/arrayness.rs` | Interprocedural arrayness analysis | `analyze`, `ArraynessInfo` (arrays through function params/returns) |
| `compiler/values.rs` | NaN-boxing value system | `ValueManager`, type tags & constants |
| `compiler/runtime.rs` | Runtime intrinsics | `Runtime`, `FormatStrings` (printf/scanf/malloc/…) |
| `compiler/generators/expression.rs` | Expression codegen | `compile_binary_op`, `compile_comparison`, list/index/len/call helpers |
| `compiler/generators/statement.rs` | Statement codegen | Statement compilation helpers |
| `compiler/generators/module.rs` | Module/method/attribute dispatch | `compile_module_call`, `compile_method_call`, `compile_attribute` |
| `compiler/generators/ndarray.rs` | NumPy ndarray codegen | `from_list`, `zeros`/`ones`/`arange`, `binop`, `reduce_sum`/`mean`/`reduce_max`/`reduce_min`, `store_index`, `slice`, `print_array` |
| `tagged_pointer.rs` | NaN-boxing reference impl | `box_int()`, `unbox_int()`, type discrimination + unit tests |
| `error.rs` | Error types & reporting | `CodeGenError`, `LoweringError`, ariadne diagnostics |

### Test Files (`python-compiler/tests/`)

| File | Tests | Count |
|------|-------|-------|
| `arithmetic.rs` | Basic math ops | 3 |
| `augmented_assignment.rs` | `+=`, `-=`, `*=`, etc. | 15 |
| `bitwise.rs` | `&`, `|`, `^`, `<<`, `>>` | 12 |
| `complex_control_flow.rs` | Recursion, algorithms | 18 |
| `control_flow.rs` | If/while/for | 15 |
| `default_arguments.rs` | Function default arguments | 3 |
| `edge_cases.rs` | Mutual recursion, overflow | 20 |
| `errors.rs` | Error handling | 5 |
| `floats.rs` | Float operations | 9 |
| `functions.rs` | Function calls, defaults | 6 |
| `input.rs` | `input()` from stdin | 3 |
| `integration.rs` | End-to-end scenarios | 4 |
| `lists.rs` | List operations | 6 |
| `minimal_test.rs` | Smoke test | 1 |
| `numpy.rs` | NumPy subset (arrays, module system) | 21 |
| `precedence.rs` | Operator precedence | 18 |
| `strings.rs` | String operations | 28 |
| `unary.rs` | Unary operators | 15 |
| `variables.rs` | Variable assignment | 3 |

**Total**: 205 tests

### Documentation (`docs/`)

| Path | Purpose |
|------|---------|
| `docs/README.md` | Documentation home |
| `docs/getting-started/` | Installation and quick start |
| `docs/architecture/` | Compilation pipeline, memory model, optimizations, type system |
| `docs/language-features/` | Feature reference |
| `docs/limitations.md` | Current limitations and workarounds |
| `docs/roadmap.md` | Planned features |
| `docs/testing/` | Testing guides |
| `docs/BUILD.md` | Build instructions |
| `docs/contributing.md` | Contribution guidelines |

---

## Dependencies

### Required System Packages

```bash
# Install LLVM 18 development headers
apt-get install -y llvm-18 llvm-18-dev llvm-18-runtime \
  libllvm18 libpolly-18-dev clang-18 libclang-18-dev \
  libzstd-dev cmake
```

### Rust Dependencies (`Cargo.toml`)

```toml
[dependencies]
inkwell = { version = "0.7.0", features = ["llvm18-1"], default-features = false }
rustpython-parser = "0.4"
thiserror = "1.0"
ariadne = "0.4"      # Error messages
num-traits = "0.2"

[dev-dependencies]
insta = "1.34"       # Snapshot testing
```

### Environment Variables

```bash
export LLVM_SYS_181_PREFIX=/usr/lib/llvm-18
```

### Installation Script

```bash
# Fix permissions if needed
chmod 1777 /tmp

# Install LLVM 18
apt-get update
apt-get install -y llvm-18-dev libpolly-18-dev clang-18 libzstd-dev

# Install cargo-insta for snapshot testing
cargo install cargo-insta

# Verify installation
llvm-config-18 --version  # Should output: 18.1.3
```

---

## Development Workflow

### 1. Initial Setup

```bash
cd /home/user/Rusthon/python-compiler
cargo build
```

### 2. Running Tests

```bash
# Run all tests
cargo test

# Run specific test file
cargo test --test augmented_assignment

# Run single test
cargo test test_mutual_recursion -- --nocapture

# Run with full backtrace
RUST_BACKTRACE=full cargo test

# Accept all snapshot changes
cargo insta test --accept

# Accept specific test snapshots
cargo insta test --test augmented_assignment --accept
```

### 3. Code Formatting

```bash
# Format all code
cargo fmt --all

# Check formatting without modifying
cargo fmt --all -- --check
```

### 4. Linting

```bash
# Run clippy
cargo clippy --all-targets --all-features -- -D warnings
```

### 5. Git Workflow

Use short, descriptive branch names, e.g. `feature/list-slicing` or `fix/string-dominance`.

```bash
# Create branch
git checkout -b feature/my-change

# Stage changes
git add -A

# Commit with a detailed message
git commit -m "feat: Add feature X

Detailed description...

Changes:
- File 1: Change A
- File 2: Change B

Test results: X/Y passing"

# Push
git push -u origin feature/my-change
```

**Commit message format**:
- Prefix: `feat:`, `fix:`, `refactor:`, `style:`, `docs:`, `test:`
- First line: concise summary (50-72 chars)
- Body: detailed changes, files modified, test results

---

## Code Documentation Guidelines

### 1. Module-Level Documentation

Use `///` for module/file documentation at the top:

```rust
/// Tagged Pointer Implementation for PyObject Optimization
///
/// This module implements a NaN-boxing scheme to reduce PyObject size.
///
/// ## Memory Layout (64-bit value)
///
/// ```text
/// [ASCII diagram here]
/// ```
///
/// ## Advantages
/// - 50% memory reduction
/// - Cache-friendly
```

**Note**: Use ` ```text` for ASCII diagrams, not ` ``` ` alone (prevents doctest errors)

### 2. Function Documentation

Document all public functions and complex private functions:

```rust
/// Declares a function signature without compiling the body.
/// This is the first pass for supporting mutual recursion.
///
/// # Arguments
///
/// * `name` - Function name
/// * `params` - Parameter names
/// * `defaults` - Default values for parameters
///
/// # Returns
///
/// The LLVM `FunctionValue` for the declared function
fn declare_function(
    &mut self,
    name: &str,
    params: &[String],
    defaults: &[Option<IRExpr>],
) -> FunctionValue<'ctx> {
    // Implementation...
}
```

### 3. Inline Comments

Use inline comments for:
- Complex algorithms
- Non-obvious optimizations
- TODOs and FIXMEs
- Workarounds

```rust
// Two-pass compilation for mutual recursion support:
// Pass 1: Declare all function signatures
for func_stmt in &functions {
    // ...
}
```

### 4. Type Documentation

Document enums and structs:

```rust
/// A simplified Intermediate Representation for expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum IRExpr {
    /// A constant integer value.
    Constant(i64),
    /// A constant float value.
    Float(f64),
    // ...
}
```

### 5. Error Documentation

Document error cases:

```rust
/// # Errors
///
/// Returns `CodeGenError::UndefinedVariable` if:
/// - Variable is not in scope
/// - Function is not declared
///
/// Returns `CodeGenError::ModuleVerification` if:
/// - LLVM IR verification fails
/// - Function signature is invalid
```

---

## Testing Guidelines

### 1. Test Structure

Tests use **snapshot testing** with `insta`:

```rust
#[test]
fn test_feature_name() {
    let source = r#"
def example(n):
    return n * 2

print(example(5))
"#;

    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();

    // Snapshot the LLVM IR
    insta::assert_snapshot!(llvm_ir);
}
```

### 2. Test File Organization

**One file per feature area**:
- Arithmetic operations → `arithmetic.rs`
- Control flow → `control_flow.rs`
- Edge cases → `edge_cases.rs`

### 3. Test Naming Convention

```rust
#[test]
fn test_<feature>_<scenario>() {
    // Pattern: test_<what>_<condition>
    // Examples:
    // - test_add_assign()
    // - test_augmented_with_floats()
    // - test_mutual_recursion()
}
```

### 4. Snapshot Management

**When snapshots change**:
```bash
# Review changes
cargo insta review

# Accept all changes
cargo insta test --accept

# Accept specific test
cargo insta test --test augmented_assignment --accept
```

**When to update snapshots**:
- Adding new features (new IR instructions)
- Optimization changes (IR format changes)
- Bug fixes (corrected IR output)

**Never update snapshots for**:
- Broken functionality
- Incorrect IR generation
- Failed compilations

### 5. Test Assertions

**Good assertions**:
```rust
// Check for key components, not implementation details
assert!(llvm_ir.contains("@main"), "Should have main function");
assert!(llvm_ir.contains("define i64 @compute"), "Should have compute function");
```

**Avoid**:
```rust
// Don't check for specific optimized instructions (may be optimized away)
assert!(llvm_ir.contains("fadd"));  // ❌ May be constant-folded
assert!(llvm_ir.contains("define double"));  // ❌ Uses i64 now
```

### 6. Error Tests

Test error handling explicitly:

```rust
#[test]
fn test_undefined_variable_error() {
    let source = "print(undefined_var)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let result = compiler.compile_program(&ir);

    assert!(result.is_err(), "Should fail with undefined variable");
    match result {
        Err(CodeGenError::UndefinedVariable(var)) => {
            assert!(var.contains("undefined_var"));
        }
        _ => panic!("Expected UndefinedVariable error"),
    }
}
```

---

## Common Tasks

### Adding a New Binary Operator

**Files to modify**:
1. `src/ast.rs`: add variant to `BinOp` enum
2. `src/lowering.rs`: add case in `lower_binop()`
3. `src/compiler/generators/expression.rs`: add case in `compile_binary_op()`
4. Create test file: `tests/new_operator.rs`

**Example**:
```rust
// 1. ast.rs
pub enum BinOp {
    // ... existing variants
    FloorDiv,  // //
}

// 2. lowering.rs
ast::Operator::FloorDiv => Ok(BinOp::FloorDiv),

// 3. compiler/generators/expression.rs
BinOp::FloorDiv => {
    // Convert to float, divide, floor, convert back to int
}

// 4. tests/floor_div.rs
#[test]
fn test_floor_div() {
    let source = "print(7 // 2)";  // Should print 3
    // ... test implementation
}
```

### Adding a New Statement Type

**Files to modify**:
1. `src/ast.rs`: add variant to `IRStmt` enum
2. `src/lowering.rs`: add case in `lower_statement()`
3. `src/compiler/generators/statement.rs`: add case in statement compilation

**Example (expression statements)**:
```rust
// 1. ast.rs
pub enum IRStmt {
    // ... existing variants
    ExprStmt(IRExpr),  // Evaluate and discard result
}

// 2. lowering.rs
ast::Stmt::Expr(ast::StmtExpr { value, .. }) => {
    let expr = lower_expression(value)?;
    Ok(IRStmt::ExprStmt(expr))
}

// 3. compiler/generators/statement.rs
IRStmt::ExprStmt(expr) => {
    self.compile_expression(expr)?;  // Evaluate and discard
}
```

### Migrating to New LLVM APIs

When LLVM APIs change:

1. **Check inkwell version**: `Cargo.toml` → `inkwell` features
2. **Review inkwell tests**: `~/.cargo/registry/src/.../inkwell-X.Y.Z/tests/`
3. **Update imports**: new types, changed namespaces
4. **Replace deprecated methods**:
   - Old: `PassManager::create()` + individual passes
   - New: `Module::run_passes("default<O2>", ...)`
5. **Update documentation**: `docs/architecture/optimizations.md`
6. **Accept new snapshots**: `cargo insta test --accept`

### Fixing Dominance Issues

**Problem**: value doesn't dominate its uses (allocated in a conditional block).

**Solution patterns**:

1. **Only track unconditional allocations**:
```rust
if let Some(main_entry) = self.main_entry_block {
    if self.builder.get_insert_block() == Some(main_entry) {
        self.arena.push(value);
    }
}
```

2. **Use phi nodes** to merge values from branches
3. **Allocate in the entry block**, set conditionally

### Updating Integer Range Limits

**Files to update**:
1. `docs/limitations.md`: document new range
2. `docs/architecture/optimizations.md`: update type encoding table
3. `docs/architecture/type-system.md`: update NaN-boxing section

**Constants** (`compiler/values.rs`):
```rust
const PAYLOAD_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;  // 48-bit = ±140 trillion
```

---

## Troubleshooting

### Issue: Tests Failing with Module Verification Errors

**Symptoms**:
```
ModuleVerification("Main function verification failed")
Both operands to ICmp instruction are not of the same type!
```

**Solutions**:
1. Check type consistency in comparisons (i8 vs i64)
2. Verify function signatures match calls
3. Run with `--nocapture` to see LLVM errors

### Issue: Undefined Function/Variable Errors

**Symptoms**:
```
UndefinedVariable("function 'foo'")
```

**Solutions**:
1. Check if the function is declared before use
2. For mutual recursion: ensure two-pass compilation is working
3. Verify the function is in `self.functions`

### Issue: Snapshot Tests Failing

**Symptoms**:
```
snapshot assertion for 'test_name' failed
```

**Solutions**:
1. Review changes: `cargo insta review`
2. If correct: `cargo insta test --accept`
3. If incorrect: fix the code, don't accept bad snapshots

### Issue: LLVM Headers Not Found

**Symptoms**:
```
fatal error: llvm-c/Target.h: No such file or directory
```

**Solutions**:
```bash
apt-get install -y llvm-18-dev libpolly-18-dev
export LLVM_SYS_181_PREFIX=/usr/lib/llvm-18
```

### Issue: Linker Errors (libzstd, etc.)

**Symptoms**:
```
rust-lld: error: unable to find library -lzstd
```

**Solutions**:
```bash
apt-get install -y libzstd-dev
```

### Issue: Doctest Failures

**Symptoms**:
```
error: expected item
src/tagged_pointer.rs - tagged_pointer::QNAN (line 14)
```

**Solutions**:
Change ASCII diagrams from ` ``` ` to ` ```text`

### Issue: CI Failing

**Common causes**:
1. Formatting: run `cargo fmt --all`
2. Clippy warnings: run `cargo clippy --fix`
3. Test failures: run `cargo test` and fix issues
4. Snapshot mismatches: accept correct snapshots

**Debug steps**:
```bash
# Check what CI runs
cat .github/workflows/rust-ci.yml

# Run the same checks locally
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --verbose
```

---

## Performance Considerations

### Memory Optimization

- **NaN-boxing**: reduces PyObject from 16→8 bytes (50% savings)
- **Stack allocation**: prefer stack over heap where possible
- **Arena allocation**: batch-free strings at program exit

### Compilation Performance

- **Two-pass functions**: slight overhead but enables mutual recursion
- **LLVM O2**: balances compile time vs runtime performance
- **Optimization passes**: ~20-30% longer compile time, significant runtime gains

### Runtime Performance

- **Type checking**: single bit test for float vs non-float
- **Value extraction**: bitwise operations (no pointer chasing)
- **Vectorization**: enabled via LLVM SLP and loop vectorization

---

## Best Practices

### DO:
- ✅ Run tests before committing
- ✅ Format code with `cargo fmt`
- ✅ Accept snapshots only if output is correct
- ✅ Document complex algorithms
- ✅ Use descriptive commit messages
- ✅ Test edge cases (overflow, recursion depth, type mixing)
- ✅ Update documentation when adding features

### DON'T:
- ❌ Commit without running tests
- ❌ Accept snapshot changes without reviewing
- ❌ Add dead code (clippy will warn)
- ❌ Use `unsafe` without thorough justification
- ❌ Skip formatting checks
- ❌ Push directly to `main`
- ❌ Optimize prematurely (measure first)

---

## Quick Reference

### File Locations

```
/home/user/Rusthon/
├── python-compiler/
│   ├── src/
│   │   ├── codegen.rs                       # Compiler driver, two-pass orchestration
│   │   ├── compiler/
│   │   │   ├── values.rs                    # NaN-boxing value system
│   │   │   ├── runtime.rs                   # Runtime intrinsics & format strings
│   │   │   └── generators/
│   │   │       ├── expression.rs            # Expression codegen
│   │   │       └── statement.rs             # Statement codegen
│   │   ├── lowering.rs                      # AST → IR
│   │   ├── ast.rs                           # IR types
│   │   ├── tagged_pointer.rs                # NaN-boxing reference impl + tests
│   │   └── ...
│   ├── tests/                               # 184 tests
│   └── Cargo.toml
├── docs/
│   ├── architecture/
│   └── limitations.md
├── README.md
└── CLAUDE.md                                # this file
```

### Key Constants

```rust
// compiler/values.rs
const QNAN: u64 = 0x7FF8_0000_0000_0000;
const PAYLOAD_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;  // 48-bit
const TAG_INT: u64 = 0;
const TAG_BOOL: u64 = 1;
const TAG_STRING: u64 = 2;
const TAG_LIST: u64 = 3;

// Integer range
// MAX_INT =  140_737_488_355_328   (2^47)
// MIN_INT = -140_737_488_355_328  (-2^47)
```

### Common Commands

```bash
# Build
cargo build

# Test
cargo test
cargo test --test augmented_assignment
cargo insta test --accept

# Format
cargo fmt --all

# Lint
cargo clippy

# Git
git add -A
git commit -m "feat: description"
git push -u origin <branch-name>
```

---

## Additional Resources

- **LLVM 18 Docs**: https://llvm.org/docs/
- **Inkwell Docs**: https://docs.rs/inkwell/0.7.1/
- **RustPython Parser**: https://docs.rs/rustpython-parser/
- **Insta (Snapshots)**: https://insta.rs/
