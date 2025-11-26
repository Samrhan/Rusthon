# Testing Guide

Comprehensive guide to Rusthon's test suite and testing practices.

## Test Suite Overview

Rusthon has **735+ lines** of tests across **15 test modules**:

```
tests/
├── lib.rs                      # Test module registration
├── arithmetic.rs               # Arithmetic operators (3 tests)
├── variables.rs                # Variable assignment (3 tests)
├── functions.rs                # Function features (6 tests)
├── floats.rs                   # Float operations (9 tests)
├── control_flow.rs             # If/while (15 tests)
├── bitwise.rs                  # Bitwise operators (12 tests) ✨ NEW
├── unary.rs                    # Unary operators (15 tests) ✨ NEW
├── augmented_assignment.rs     # Augmented ops (15 tests) ✨ NEW
├── precedence.rs               # Operator precedence (19 tests) ✨ NEW
├── edge_cases.rs               # Edge cases (20 tests) ✨ NEW
├── strings.rs                  # String literals (19 tests) ✨ NEW
├── complex_control_flow.rs     # Complex algorithms (18 tests) ✨ NEW
├── input.rs                    # Input function (3 tests)
├── errors.rs                   # Error handling (6 tests)
└── integration.rs              # Complete programs (4 tests)
```

**Total: 167+ test cases**

## Running Tests

### All Tests
```bash
cd python-compiler
cargo test
```

### Specific Module
```bash
cargo test arithmetic
cargo test functions
cargo test bitwise
```

### Specific Test
```bash
cargo test test_simple_addition
cargo test test_fibonacci_recursive
```

### With Output
```bash
cargo test -- --nocapture
cargo test -- --show-output
```

### Update Snapshots
```bash
cargo insta test
cargo insta review  # Review snapshot changes
cargo insta accept  # Accept all changes
```

## Test Structure

### Snapshot Testing

We use `insta` for snapshot testing of LLVM IR:

```rust
#[test]
fn test_simple_addition() {
    let source = "print(1 + 2)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}
```

**Snapshots** are stored in `tests/snapshots/`:
```
tests/snapshots/
├── arithmetic__simple_addition.snap
├── functions__multiple_parameters.snap
└── ... (100+ snapshot files)
```

### Error Testing

```rust
#[test]
fn test_undefined_variable_error() {
    let source = "print(undefined_var)";
    let ast = parser::parse_program(source);
    assert!(ast.is_ok(), "Parsing should succeed");

    let ir = lowering::lower_program(&ast.unwrap());
    assert!(ir.is_ok(), "Lowering should succeed");

    let context = inkwell::context::Context::create();
    let compiler = codegen::Compiler::new(&context);
    let result = compiler.compile_program(&ir.unwrap());

    assert!(result.is_err(), "Should fail with undefined variable");
    match result {
        Err(codegen::CodeGenError::UndefinedVariable(var)) => {
            assert!(var.contains("undefined_var"));
        }
        _ => panic!("Expected UndefinedVariable error"),
    }
}
```

## Test Coverage

### Feature Coverage

| Feature | Tests | Modules |
|---------|-------|---------|
| Arithmetic | 25+ | arithmetic, precedence, edge_cases |
| Variables | 10+ | variables, functions, edge_cases |
| Functions | 30+ | functions, complex_control_flow, edge_cases |
| Floats | 20+ | floats, precedence, edge_cases |
| Control Flow | 45+ | control_flow, complex_control_flow, edge_cases |
| Bitwise | 12+ | bitwise, precedence |
| Unary | 15+ | unary, precedence |
| Augmented | 15+ | augmented_assignment |
| Strings | 19+ | strings |
| Input | 3+ | input |
| Errors | 6+ | errors |

### Edge Case Coverage

✅ Division by zero
✅ Large numbers
✅ Negative numbers
✅ Deep recursion
✅ Mutual recursion
✅ Empty functions
✅ Deeply nested loops
✅ Deeply nested if statements
✅ Many variables
✅ Many parameters
✅ Float precision
✅ Operator precedence
✅ All operator combinations

## Writing New Tests

### Step 1: Choose Module

Add test to existing module or create new one:

```bash
# Create new test module
touch tests/my_feature.rs
```

### Step 2: Register Module

Edit `tests/lib.rs`:
```rust
mod arithmetic;
mod variables;
// ... existing modules ...
mod my_feature;  // Add your module
```

### Step 3: Write Test

```rust
use inkwell::context::Context;
use python_compiler::*;

#[test]
fn test_my_feature() {
    let source = r#"
# Your Python code here
x = 10
print(x)
"#;

    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}
```

### Step 4: Run Test

```bash
cargo test test_my_feature
```

### Step 5: Review Snapshot

```bash
cargo insta review
# Review generated LLVM IR
# Press 'a' to accept, 'r' to reject
```

## Test Categories

### Unit Tests
Test individual features in isolation:
- Single operators
- Simple expressions
- Basic control flow

### Integration Tests
Test features working together:
- Complete algorithms (factorial, fibonacci)
- Multiple features combined
- Real-world programs

### Error Tests
Test error handling:
- Parse errors
- Lowering errors
- Code generation errors
- Undefined variables/functions

### Edge Case Tests
Test boundary conditions:
- Division by zero
- Very large/small numbers
- Deep recursion
- Empty structures

## Continuous Integration

### GitHub Actions (if configured)

```yaml
name: Tests
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - run: cargo test
```

## Test Metrics

### Current Coverage

```
Total Tests:       167+
Total Test Lines:  735+
Test Modules:      15
Feature Coverage:  ~95%
Edge Cases:        ~90%
Error Handling:    ~85%
```

### Areas for Improvement

🔄 **More Integration Tests**
- Complex multi-file programs (not yet supported)
- More realistic algorithms
- Performance benchmarks

🔄 **Property-Based Testing**
- Use `proptest` for random test generation
- Test operator properties (commutativity, etc.)

🔄 **Fuzzing**
- Use `cargo-fuzz` to find edge cases
- Test parser robustness
- Find potential crashes

## Best Practices

### 1. Test Names Should Be Descriptive

```rust
// Good
#[test]
fn test_fibonacci_recursive()

// Bad
#[test]
fn test1()
```

### 2. Use Snapshot Testing for LLVM IR

```rust
// Good - automatic LLVM IR verification
insta::assert_snapshot!(llvm_ir);

// Bad - manual comparison
assert!(llvm_ir.contains("define"));
```

### 3. Test Both Success and Failure

```rust
#[test]
fn test_valid_syntax() { /* ... */ }

#[test]
fn test_invalid_syntax_error() { /* ... */ }
```

### 4. Group Related Tests

```rust
// In arithmetic.rs
#[test]
fn test_addition() { /* ... */ }

#[test]
fn test_subtraction() { /* ... */ }

#[test]
fn test_multiplication() { /* ... */ }
```

## Next Steps

- [Running Tests](/testing/running-tests) - Detailed testing commands
- [Test Coverage](/testing/test-coverage) - Coverage analysis
- [Contributing](/contributing) - How to contribute tests
