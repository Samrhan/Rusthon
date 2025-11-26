# Rusthon Documentation

Welcome to the **Rusthon** documentation! Rusthon is a Python-to-LLVM compiler written in Rust that compiles a subset of Python directly to native machine code.

## What is Rusthon?

Rusthon demonstrates the feasibility of a high-performance Python compiler that:
- Compiles Python source code directly to LLVM IR
- Generates native executables via Clang
- Provides performance similar to compiled languages like C
- Uses a tagged union type system for dynamic typing
- Requires no runtime garbage collection (with some memory trade-offs)

## Key Features

- ✅ **Native Compilation**: Compiles Python to native machine code via LLVM
- ✅ **Rich Operator Support**: Full arithmetic, bitwise, unary, and comparison operators
- ✅ **Control Flow**: If/else statements and while loops
- ✅ **Functions**: Function definitions, calls, recursion, and multiple parameters
- ✅ **Type System**: Dynamic typing with automatic type promotion (int ↔ float)
- ✅ **I/O Operations**: Print statements and input() function
- ✅ **String Literals**: Full support for string literals with escape sequences

## Quick Links

- [Getting Started](/getting-started) - Installation and quick start guide
- [Architecture Overview](/architecture) - Understanding how Rusthon works
- [Language Features](/language-features) - Complete feature reference
- [Implementation Details](/implementation) - Deep dive into internals
- [Testing Guide](/testing) - How to run and write tests

## Project Status

Rusthon is an **educational project** demonstrating Python compilation techniques. It supports a useful subset of Python suitable for numerical computation and algorithmic programming.

See [Limitations](/limitations) for features not yet supported and [Roadmap](/roadmap) for planned enhancements.

## Quick Example

```python
def fibonacci(n):
    if n <= 1:
        return n
    return fibonacci(n - 1) + fibonacci(n - 2)

print(fibonacci(10))
```

Compile and run:
```bash
cargo run -- examples/fibonacci.py
./examples/fibonacci
```

## Contributing

Rusthon welcomes contributions! See [Contributing](/contributing) for guidelines.

## License

MIT License - see repository for details.
