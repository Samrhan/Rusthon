# Roadmap

Planned features and improvements for Rusthon.

## Short Term (Next Release)

### Control Flow Enhancements

**Elif Support**
```python
if x < 0:
    print("negative")
elif x == 0:
    print("zero")
else:
    print("positive")
```
- Status: 🔄 Planned
- Difficulty: Easy
- Impact: High

**Break/Continue**
```python
while True:
    if condition:
        break
    if other_condition:
        continue
```
- Status: 🔄 Planned
- Difficulty: Medium
- Impact: High

**For Loops (range only)**
```python
for i in range(10):
    print(i)
```
- Status: 🔄 Planned
- Difficulty: Medium
- Impact: High

### Type System Improvements

**Boolean Literals**
```python
x = True
y = False
if x:
    print("yes")
```
- Status: 🔄 Planned
- Difficulty: Easy
- Impact: Medium

**Better Type Inference**
- Deduce types when possible
- Generate optimized code for known types
- Status: 🔄 Planned
- Difficulty: Hard
- Impact: Medium

### Memory Management

**Arena Allocation for Strings**
```rust
// Allocate strings from arena
// Free all at program end
```
- Status: 🔄 Planned
- Difficulty: Medium
- Impact: High (fixes memory leaks)

## Medium Term

### NumPy (compiled subset)

A native, compiled subset of NumPy — no CPython, no `libnumpy`. Arrays are
unboxed, typed, contiguous buffers whose element-wise loops auto-vectorize.

**Phase 1 — 1-D float64 arrays** ✅ Done
```python
import numpy as np
a = np.array([1.0, 2.0, 3.0, 4.0])
b = a + 1                 # element-wise + scalar broadcasting (auto-vectorized)
print(a.sum(), a.mean(), a[0], len(a), a.size)
# constructors: np.array / np.zeros / np.ones / np.arange
# constants:    np.pi / np.e
```
Delivered via a generic module system (`import` resolves aliases; codegen owns
the module registry) and a pay-as-you-go "may-be-array" analysis so scalar code
is unchanged.

**Next phases** 🔮 Planned
- Additional dtypes (`int64`) tracked in the array header.
- Slicing (`a[1:3]`), item assignment (`a[0] = x`), boolean/fancy indexing.
- Arrays across user-defined function parameters and return values.
- Multi-dimensional arrays (`ndim`/shape/strides), `reshape`, `.T`.
- Linear algebra (`np.dot`, `@`), more reductions/ufuncs, array printing.

### Data Structures

**Lists (Fixed Size)**
```python
my_list = [1, 2, 3, 4, 5]
print(my_list[0])
my_list[1] = 10
```
- Status: 🔮 Future
- Difficulty: Hard
- Dependencies: Memory management

**Tuples**
```python
point = (10, 20)
x, y = point
```
- Status: 🔮 Future
- Difficulty: Medium
- Dependencies: Multiple return values

### String Operations

**String Concatenation**
```python
result = "Hello" + " " + "World"
```
- Status: 🔮 Future
- Difficulty: Medium
- Dependencies: Arena allocation

**String Methods**
```python
text = "hello"
upper = text.upper()
length = len(text)
```
- Status: 🔮 Future
- Difficulty: Hard
- Dependencies: Methods, dynamic dispatch

### Function Enhancements

**Default Arguments**
```python
def greet(name="World"):
    print("Hello", name)
```
- Status: 🔮 Future
- Difficulty: Medium

**Multiple Return Values**
```python
def min_max(a, b):
    return a, b if a < b else b, a

min_val, max_val = min_max(5, 10)
```
- Status: 🔮 Future
- Difficulty: Medium
- Dependencies: Tuples

## Long Term

### Object-Oriented Programming

**Classes (Basic)**
```python
class Point:
    def __init__(self, x, y):
        self.x = x
        self.y = y

    def distance(self):
        return (self.x ** 2 + self.y ** 2) ** 0.5
```
- Status: 🔮 Future
- Difficulty: Very Hard
- Dependencies: Memory management, methods

**Inheritance**
```python
class ColoredPoint(Point):
    def __init__(self, x, y, color):
        super().__init__(x, y)
        self.color = color
```
- Status: 🔮 Future
- Difficulty: Very Hard
- Dependencies: Classes, vtables

### Advanced Features

**List Comprehensions**
```python
squares = [x**2 for x in range(10)]
```
- Status: 🔮 Future
- Difficulty: Hard
- Dependencies: Lists, for loops

**Lambda Functions**
```python
square = lambda x: x * x
```
- Status: 🔮 Future
- Difficulty: Medium

**Generators**
```python
def count_up_to(n):
    i = 0
    while i < n:
        yield i
        i += 1
```
- Status: 🔮 Future
- Difficulty: Very Hard
- Dependencies: Advanced control flow

### Module System

**Imports**
```python
import math
from mymodule import myfunction
```
- Status: 🔮 Future
- Difficulty: Very Hard
- Dependencies: Multiple files, linking

### Error Handling

**Try/Except**
```python
try:
    result = risky_operation()
except ValueError:
    print("Error occurred")
```
- Status: 🔮 Future
- Difficulty: Very Hard
- Dependencies: Exception tables, unwinding

### Optimization

**LLVM Optimization Passes**
- Enable LLVM's optimization passes
- Dead code elimination
- Constant folding
- Inlining
- Status: 🔮 Future
- Difficulty: Medium

**Whole Program Optimization**
- Link-time optimization
- Cross-function inlining
- Status: 🔮 Future
- Difficulty: Hard

**JIT Compilation**
- Compile at runtime
- Hot code optimization
- Status: 🔮 Future
- Difficulty: Very Hard

## Performance Goals

| Benchmark | Current | Target |
|-----------|---------|--------|
| Fibonacci(30) | TBD | < 100ms |
| Factorial(1000) | TBD | < 10ms |
| Prime(10000) | TBD | < 1s |
| Compilation Time | TBD | < 1s |

## Infrastructure

### Tooling

**Language Server Protocol (LSP)**
- IDE support
- Auto-completion
- Go to definition
- Status: 🔮 Future
- Difficulty: Very Hard

**Debugger Support**
- DWARF debug info
- GDB/LLDB integration
- Breakpoints, stepping
- Status: 🔮 Future
- Difficulty: Hard

**Package Manager**
- Dependency management
- Version resolution
- Status: 🔮 Future
- Difficulty: Very Hard

### Documentation

**Interactive Playground**
- Try Rusthon in browser
- WASM compilation
- Status: 🔮 Future
- Difficulty: Hard

**Video Tutorials**
- Getting started
- Advanced features
- Architecture deep-dive
- Status: 🔮 Future
- Difficulty: Easy

### Community

**Discord Server**
- Real-time chat
- Help and support
- Status: 🔄 Planned

**Monthly Blog Posts**
- Progress updates
- Technical deep-dives
- Status: 🔄 Planned

## Timeline

### Q1 2025
- ✅ Comprehensive test suite
- ✅ Complete documentation
- 🔄 Elif support
- 🔄 Break/continue
- 🔄 Boolean literals

### Q2 2025
- For loops (range)
- Arena allocation
- String concatenation
- Default arguments

### Q3 2025
- Fixed-size lists
- Tuples
- Better type inference
- Optimization passes

### Q4 2025
- Basic classes
- Module system
- Error handling
- LSP support

## Contributing

Want to help implement these features? See [Contributing](/contributing).

### High Priority

1. **Elif Support** - Easy, high impact
2. **Break/Continue** - Medium, high impact
3. **Arena Allocation** - Medium, fixes memory leaks
4. **For Loops** - Medium, high impact

### Medium Priority

5. Boolean literals
6. String concatenation
7. Default arguments
8. Lists (fixed size)

### Low Priority (Long Term)

9. Classes
10. Module system
11. Error handling
12. Advanced optimizations

## Legend

- ✅ Completed
- 🔄 In Progress
- 🔮 Planned
- ❓ Under Consideration
- ❌ Declined

## Next Steps

- [Contributing](/contributing) - Help build these features
- [Limitations](/limitations) - Current limitations
- [GitHub Issues](https://github.com/Samrhan/Rusthon/issues) - Track progress
