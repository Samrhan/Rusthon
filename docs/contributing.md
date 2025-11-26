# Contributing

Thank you for your interest in contributing to Rusthon!

## Getting Started

1. **Fork the repository**
   ```bash
   # Click "Fork" on GitHub
   git clone https://github.com/YOUR_USERNAME/Rusthon.git
   cd Rusthon
   ```

2. **Set up development environment**
   ```bash
   # Using devcontainer (recommended)
   code .  # Open in VS Code
   # Then: "Reopen in Container"

   # Or manually install dependencies
   # See: /getting-started/installation
   ```

3. **Create a branch**
   ```bash
   git checkout -b feature/my-new-feature
   ```

## Development Workflow

### 1. Make Changes

Edit the relevant files:
- `src/parser.rs` - Parser wrapper
- `src/lowering.rs` - AST → IR conversion
- `src/codegen.rs` - IR → LLVM code generation
- `src/error.rs` - Error handling

### 2. Write Tests

Add tests in `tests/`:
```rust
#[test]
fn test_my_feature() {
    let source = "print(42)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}
```

### 3. Run Tests

```bash
cd python-compiler
cargo test
cargo clippy
cargo fmt
```

### 4. Update Documentation

Update docs in `docs/` as needed.

### 5. Commit Changes

```bash
git add .
git commit -m "feat: Add support for my feature"
git push origin feature/my-new-feature
```

### 6. Create Pull Request

Go to GitHub and create a PR from your branch.

## Coding Standards

### Rust Style

Follow standard Rust conventions:
```rust
// Use snake_case for functions and variables
fn compile_expression() { }
let my_variable = 42;

// Use CamelCase for types
struct MyStruct { }
enum MyEnum { }

// Use SCREAMING_SNAKE_CASE for constants
const MAX_VALUE: i32 = 100;
```

### Documentation

Document public APIs:
```rust
/// Compiles a Python expression to LLVM IR.
///
/// # Arguments
/// * `expr` - The expression to compile
///
/// # Returns
/// The compiled LLVM value
pub fn compile_expression(&self, expr: &IRExpr) -> Result<BasicValueEnum<'ctx>> {
    // ...
}
```

### Error Handling

Use `Result` for fallible operations:
```rust
// Good
pub fn parse_program(source: &str) -> Result<Program, ParseError> {
    // ...
}

// Bad - don't panic in library code
pub fn parse_program(source: &str) -> Program {
    // ...
    panic!("oh no!");  // ❌
}
```

### Testing

- Write tests for new features
- Use snapshot testing for LLVM IR
- Test both success and failure cases
- Add edge case tests

```rust
#[test]
fn test_feature_success() {
    // Test normal case
}

#[test]
fn test_feature_error() {
    // Test error case
    assert!(result.is_err());
}

#[test]
fn test_feature_edge_case() {
    // Test boundary conditions
}
```

## Adding Language Features

### Example: Adding Support for Boolean Literals

#### 1. Extend IR (`src/ast.rs`)

```rust
pub enum IRExpr {
    Constant(i64),
    Float(f64),
    BoolLiteral(bool),  // ← Add this
    // ...
}
```

#### 2. Extend Lowering (`src/lowering.rs`)

```rust
fn lower_expression(expr: &Expr) -> Result<IRExpr, LoweringError> {
    match expr {
        Expr::Constant { value: Constant::Bool(b), .. } => {
            Ok(IRExpr::BoolLiteral(*b))  // ← Add this
        }
        // ...
    }
}
```

#### 3. Extend Code Generation (`src/codegen.rs`)

```rust
fn compile_expression(&mut self, expr: &IRExpr) -> Result<BasicValueEnum<'ctx>> {
    match expr {
        IRExpr::BoolLiteral(b) => {
            let value = if *b { 1.0 } else { 0.0 };
            Ok(self.create_pyobject_bool(value).into())  // ← Add this
        }
        // ...
    }
}
```

#### 4. Add Tests (`tests/booleans.rs`)

```rust
#[test]
fn test_bool_true() {
    let source = "print(True)";
    // ... test code ...
}

#[test]
fn test_bool_false() {
    let source = "print(False)";
    // ... test code ...
}
```

#### 5. Update Documentation (`docs/language-features/data-types.md`)

```markdown
## Booleans

Rusthon supports boolean literals:

​```python
x = True
y = False
​```
```

## Project Areas

### Good First Issues

- ✅ Add more tests
- ✅ Fix documentation typos
- ✅ Add examples
- ✅ Improve error messages

### Intermediate

- 🔨 Add `elif` support
- 🔨 Add `for` loops
- 🔨 Add `break`/`continue`
- 🔨 Improve type inference

### Advanced

- 🚀 Add garbage collection
- 🚀 Add list support
- 🚀 Add module system
- 🚀 Add optimization passes

## Code Review Process

1. **Automated Checks**
   - Tests must pass
   - Code must compile
   - Clippy lints must pass
   - Code must be formatted

2. **Human Review**
   - Code quality
   - Test coverage
   - Documentation
   - Design decisions

3. **Merge**
   - Once approved, maintainer will merge
   - Your contribution is live!

## Community Guidelines

- Be respectful and professional
- Provide constructive feedback
- Help others learn
- Have fun!

## Getting Help

- 💬 GitHub Discussions - Ask questions
- 🐛 GitHub Issues - Report bugs
- 📧 Email maintainers
- 📚 Read the docs

## License

By contributing, you agree your contributions will be licensed under the MIT License.

## Recognition

Contributors are recognized in:
- GitHub contributors page
- Release notes
- Project README

Thank you for contributing to Rusthon! 🎉
