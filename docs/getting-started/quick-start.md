# Quick Start

Get up and running with Rusthon in under 5 minutes.

## Hello, World!

Create a file `hello.py`:

```python
print("Hello, World!")
```

Compile and run:

```bash
cd python-compiler
cargo run -- hello.py
./hello
```

Output:
```
Hello, World!
```

## How It Works

When you run Rusthon:

1. **Parsing**: Python source is parsed into an AST using `rustpython-parser`
2. **Lowering**: Python AST is converted to a custom intermediate representation (IR)
3. **Code Generation**: IR is compiled to LLVM IR using `inkwell`
4. **Native Compilation**: LLVM IR is written to a `.ll` file
5. **Executable Creation**: `clang` compiles the LLVM IR to a native executable

## More Examples

### Simple Arithmetic

```python
x = 10
y = 20
result = x + y * 2
print(result)  # 50
```

### Functions

```python
def add(a, b):
    return a + b

def multiply(a, b):
    return a * b

x = add(5, 3)
y = multiply(x, 2)
print(y)  # 16
```

### Control Flow

```python
def factorial(n):
    if n <= 1:
        return 1
    return n * factorial(n - 1)

print(factorial(5))  # 120
```

### Loops

```python
i = 0
sum = 0
while i < 10:
    sum += i
    i += 1
print(sum)  # 45
```

## Command Line Options

```bash
# Basic usage
cargo run -- input.py

# The output executable will be named after the input file
# input.py -> input (executable)

# View generated LLVM IR
cat input.ll
```

## Development Workflow

```bash
# Run in debug mode (faster compilation)
cargo run -- myprogram.py

# Build optimized compiler
cargo build --release

# Run tests
cargo test

# Run linter
cargo clippy

# Format code
cargo fmt
```

## Common Issues

### Undefined Variable

```python
print(x)  # Error: undefined variable 'x'
```

**Solution**: Define variables before use:
```python
x = 42
print(x)
```

### Unsupported Feature

```python
my_list = [1, 2, 3]  # Error: lists not supported
```

**Solution**: See [Limitations](/limitations) for unsupported features.

### Parse Error

```python
def invalid syntax  # Error: invalid syntax
```

**Solution**: Use valid Python syntax:
```python
def my_function():
    return 0
```

## Next Steps

- [Your First Program](/getting-started/your-first-program) - Learn the language features
- [Language Features](/language-features) - Complete reference
- [Examples](https://github.com/Samrhan/Rusthon/tree/main/python-compiler/examples) - Browse example programs
