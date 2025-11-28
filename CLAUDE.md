# AI Agent Guide for Rusthon Compiler

This document provides comprehensive guidance for AI agents working on the Rusthon Python-to-LLVM compiler project.

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

- **NaN-boxing optimization**: 50% memory reduction (16 bytes → 8 bytes per PyObject)
- **LLVM 18 new pass manager**: O2 optimization pipeline with vectorization
- **Mutual recursion support**: Two-pass compilation
- **Type system**: Runtime type checking with tagged unions
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
- Converts Python AST → Custom IR
- Handles:
  - Expression statements
  - Augmented assignments (desugar to binary ops)
  - Control flow (if/while/for)
  - Function definitions
- **Key types**: `IRExpr`, `IRStmt`

#### 3. **CodeGen** (`codegen.rs`)
- Converts IR → LLVM IR
- **NaN-boxing**: All values are `i64` (PyObject)
- **Two-pass compilation**:
  - Pass 1: Declare all function signatures
  - Pass 2: Compile function bodies (enables mutual recursion)
- **String cleanup**: Only tracks strings allocated in main entry block

#### 4. **Optimization** (LLVM 18 new pass manager)
- Pipeline: `default<O2>`
- Features: Loop vectorization, SLP vectorization, function merging
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
- `TAG_INT = 0`: Signed 48-bit integer
- `TAG_BOOL = 1`: Boolean (1-bit payload)
- `TAG_STRING = 2`: String pointer (48-bit)
- `TAG_LIST = 3`: List pointer (48-bit)
- Floats: No tag (stored as canonical float64)

**Constants** (`codegen.rs:13-21`):
```rust
const QNAN: u64 = 0x7FF8_0000_0000_0000;
const TAG_MASK: u64 = 0x0007_0000_0000_0000;
const PAYLOAD_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;
```

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
- **Length storage**: Offset 0 contains the list length as i64
- **Element storage**: Elements start at offset 1
- **Indexing**: `list[i]` accesses offset `i + 1` to skip the length header
- **len() operation**: Reads i64 at offset 0 (O(1) time)
- **Memory overhead**: +8 bytes per list (1 i64 header)

**Code locations**:
- List allocation: `codegen.rs:1644-1713` (`IRExpr::List`)
- Length extraction: `codegen.rs:225-268` (`extract_list_ptr_and_len`)
- List indexing: `codegen.rs:1730-1776` (`IRExpr::Index`)
- len() for lists: `codegen.rs:1458-1557` (`IRExpr::Len`)

---

## File Structure

### Source Files (`python-compiler/src/`)

| File | Purpose | Key Functions/Types |
|------|---------|---------------------|
| `lib.rs` | Library entry point | Exports all public modules |
| `main.rs` | CLI entry point | Argument parsing, file I/O |
| `parser.rs` | Python parsing | `parse_program()` |
| `ast.rs` | IR definitions | `IRExpr`, `IRStmt`, `BinOp`, `CmpOp`, `UnaryOp` |
| `lowering.rs` | AST → IR | `lower_program()`, `lower_statement()`, `lower_expression()` |
| `codegen.rs` | IR → LLVM IR (1800+ lines) | `Compiler`, `compile_program()`, `compile_expression()` |
| `tagged_pointer.rs` | NaN-boxing utilities | `box_int()`, `unbox_int()`, type discrimination |
| `error.rs` | Error types | `CodeGenError`, `LoweringError` |
| `compiler.rs` | Orchestration | High-level compilation interface |

### Test Files (`python-compiler/tests/`)

| File | Tests | Count |
|------|-------|-------|
| `augmented_assignment.rs` | `+=`, `-=`, `*=`, etc. | 15 |
| `arithmetic.rs` | Basic math ops | 3 |
| `bitwise.rs` | `&`, `|`, `^`, `<<`, `>>` | 12 |
| `complex_control_flow.rs` | Recursion, algorithms | 18 |
| `control_flow.rs` | If/while/for | 15 |
| `edge_cases.rs` | Mutual recursion, overflow | 20 |
| `errors.rs` | Error handling | 5 |
| `floats.rs` | Float operations | 3 |
| `functions.rs` | Function calls, defaults | 6 |
| `integration.rs` | End-to-end scenarios | 4 |
| `lists.rs` | List operations | 9 |
| `strings.rs` | String operations | 28 |
| `variables.rs` | Variable assignment | 3 |
| `unary.rs` | Unary operators | 15 |
| `precedence.rs` | Operator precedence | 15 |

**Total**: ~174 tests

### Documentation (`docs/`)

| File | Purpose |
|------|---------|
| `README.md` | Main project documentation (French) |
| `IMPLEMENTATION_SUMMARY.md` | Detailed feature documentation (755 lines) |
| `TAGGED_POINTER_INTEGRATION.md` | NaN-boxing integration details |
| `docs/architecture/` | Architecture docs (optimizations, compilation, memory, types) |
| `docs/limitations.md` | Current limitations and workarounds (423 lines) |
| `docs/testing/` | Testing guides |

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
inkwell = { version = "0.7.0", features = ["llvm18-1"] }
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

**Branch naming**: `claude/fix-description-{sessionId}`

```bash
# Create branch
git checkout -b claude/fix-issue-name-sessionId

# Stage changes
git add -A

# Commit with detailed message
git commit -m "feat: Add feature X

Detailed description...

Changes:
- File 1: Change A
- File 2: Change B

Test results: X/Y passing"

# Push with retry logic
git push -u origin branch-name
```

**Commit message format**:
- Prefix: `feat:`, `fix:`, `refactor:`, `style:`, `docs:`, `test:`
- First line: Concise summary (50-72 chars)
- Body: Detailed changes, files modified, test results

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
1. `src/ast.rs`: Add variant to `BinOp` enum
2. `src/lowering.rs`: Add case in `lower_binop()`
3. `src/codegen.rs`: Add case in `compile_expression()` for `BinaryOp`
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

// 3. codegen.rs
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
1. `src/ast.rs`: Add variant to `IRStmt` enum
2. `src/lowering.rs`: Add case in `lower_statement()`
3. `src/codegen.rs`: Add case in `compile_statement()`

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

// 3. codegen.rs
IRStmt::ExprStmt(expr) => {
    self.compile_expression(expr)?;  // Evaluate and discard
}
```

### Migrating to New LLVM APIs

When LLVM APIs change:

1. **Check inkwell version**: `Cargo.toml` → `inkwell` features
2. **Review inkwell tests**: `/root/.cargo/registry/src/.../inkwell-X.Y.Z/tests/`
3. **Update imports**: New types, changed namespaces
4. **Replace deprecated methods**:
   - Old: `PassManager::create()` + individual passes
   - New: `Module::run_passes("default<O2>", ...)`
5. **Update documentation**: `docs/architecture/optimizations.md`
6. **Accept new snapshots**: `cargo insta test --accept`

### Fixing Dominance Issues

**Problem**: Value doesn't dominate its uses (allocated in conditional block).

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
3. **Allocate in entry block**, set conditionally

### Updating Integer Range Limits

**Files to update**:
1. `docs/limitations.md`: Document new range
2. `docs/architecture/optimizations.md`: Update type encoding table
3. `IMPLEMENTATION_SUMMARY.md`: Update NaN-boxing section
4. `TAGGED_POINTER_INTEGRATION.md`: Update limitations

**Constants** (`codegen.rs:15`):
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
1. Check if function is declared before use
2. For mutual recursion: ensure two-pass compilation is working
3. Verify function is in `self.functions` HashMap

### Issue: Snapshot Tests Failing

**Symptoms**:
```
snapshot assertion for 'test_name' failed
```

**Solutions**:
1. Review changes: `cargo insta review`
2. If correct: `cargo insta test --accept`
3. If incorrect: Fix code, don't accept bad snapshots

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
1. Formatting: Run `cargo fmt --all`
2. Clippy warnings: Run `cargo clippy --fix`
3. Test failures: Run `cargo test` and fix issues
4. Snapshot mismatches: Accept correct snapshots

**Debug steps**:
```bash
# Check what CI runs
cat .github/workflows/rust-ci.yml

# Run same checks locally
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --verbose
```

---

## Performance Considerations

### Memory Optimization

- **NaN-boxing**: Reduces PyObject from 16→8 bytes (50% savings)
- **Stack allocation**: Prefer stack over heap where possible
- **Arena allocation**: Batch allocate strings (partially implemented)

### Compilation Performance

- **Two-pass functions**: Slight overhead but enables mutual recursion
- **LLVM O2**: Balances compile time vs runtime performance
- **Optimization passes**: ~20-30% longer compile time, significant runtime gains

### Runtime Performance

- **Type checking**: Single bit test for float vs non-float (~40% faster than struct)
- **Value extraction**: Bitwise operations (no pointer chasing)
- **Vectorization**: Enabled via LLVM SLP and loop vectorization

---

## Best Practices

### DO:
✅ Run tests before committing
✅ Format code with `cargo fmt`
✅ Accept snapshots only if output is correct
✅ Document complex algorithms
✅ Use descriptive commit messages
✅ Test edge cases (overflow, recursion depth, type mixing)
✅ Update documentation when adding features

### DON'T:
❌ Commit without running tests
❌ Accept snapshot changes without reviewing
❌ Add dead code (clippy will warn)
❌ Use `unsafe` without thorough justification
❌ Skip formatting checks
❌ Push to main branch directly
❌ Optimize prematurely (measure first)

---

## Quick Reference

### File Locations

```
/home/user/Rusthon/
├── python-compiler/
│   ├── src/
│   │   ├── codegen.rs          (1800+ lines, IR → LLVM)
│   │   ├── lowering.rs         (AST → IR)
│   │   ├── ast.rs              (IR types)
│   │   └── ...
│   ├── tests/
│   │   └── *.rs                (174 tests)
│   └── Cargo.toml
├── docs/
│   ├── architecture/
│   └── limitations.md
├── IMPLEMENTATION_SUMMARY.md   (755 lines)
└── AGENT.md                    (this file)
```

### Key Constants

```rust
// codegen.rs
const QNAN: u64 = 0x7FF8_0000_0000_0000;
const PAYLOAD_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;  // 48-bit
const TAG_INT: u64 = 0;
const TAG_BOOL: u64 = 1;
const TAG_STRING: u64 = 2;

// Integer range
const MAX_INT: i64 = 140_737_488_355_328;   // 2^47
const MIN_INT: i64 = -140_737_488_355_328;  // -2^47
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
git push -u origin claude/branch-name-sessionId
```

---

## Additional Resources

- **LLVM 18 Docs**: https://llvm.org/docs/
- **Inkwell Docs**: https://docs.rs/inkwell/0.7.1/
- **RustPython Parser**: https://docs.rs/rustpython-parser/
- **Insta (Snapshots)**: https://insta.rs/

---

**Last Updated**: 2025-11-28
**Version**: 1.1
**Branch**: `claude/implement-list-allocation-011WAnqDHpZqBFVuMe8Rfe9e`
**Recent Changes**: Added list allocation with length header implementation
