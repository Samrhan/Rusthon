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

**Phase 2 — mutation, slicing & I/O** ✅ Done
```python
import numpy as np
a = np.arange(6)
a[0] = 9.0                # item assignment
s = a[1:4]                # copy-slicing (a[:2], a[3:], a[:] too)
print(a)                  # [9. 1. 2. 3. 4. 5.]
print(a.max(), a.min())   # reductions
```

**Phase 3 — arrays through functions** ✅ Done
```python
import numpy as np
def normalize(v):
    return v / v.sum()        # array parameter in, array out
print(normalize(np.arange(4)))
```
A whole-program "arrayness" analysis (monotonic fixpoint over
`array_returning` / `array_params`) propagates array-ness through function
parameters and return values — transitively and through recursion — with no
annotations. Scalar code stays untouched (pay-as-you-go).

**Phase 4 — element-wise math & linear algebra** ✅ Done
```python
import numpy as np
a = np.array([1.0, 4.0, 9.0, 16.0])
print(np.sqrt(a))                 # vectorized ufunc (llvm.sqrt.v2f64)
print(np.dot(a, a))               # 1-D dot product
print(a.prod())                   # product reduction
```
Unary ufuncs (`sqrt`/`abs`/`exp`/`log`/`sin`/`cos`/`floor`/`ceil`) lower to
LLVM intrinsics and vectorize; they mirror their argument (array→array,
scalar→scalar). Plus `np.dot` and `prod`.

**Phase 5 — int64 dtype & promotion** ✅ Done
```python
import numpy as np
a = np.arange(4)          # int64: [0 1 2 3]
print(a + 1, a * a)       # int stays int
print(a / 2, a.mean())    # `/` and mean promote to float
```
Arrays are `int64` or `float64`; the dtype is carried in the header and tracked
at compile time (including through functions), so int and float arrays get
separate fast code with NumPy-style promotion. `ArrayDtype::Unknown` (a value
that is int on one path, float on another) is a compile-time error.

**Phase 6 — 2-D arrays (matrices)** ✅ Done
```python
import numpy as np
m = np.array([[1.0, 2.0], [3.0, 4.0]])
print(m[0, 1], m.T)          # indexing and transpose
print(m + m, np.matmul(m, m))# element-wise and matrix product
```
A fixed 5-word header `[dtype, ndim, size, rows, cols]` carries the shape.
2-D construction from nested literals, `a[i, j]`, `.T`, `np.matmul`, and
element-wise ops/reductions (which run over the flat buffer) all work for
int and float matrices.

**Phase 7 — tuples & multiple return values** ✅ Done
```python
def minmax(a, b):
    return (a, b) if a < b else (b, a)  # multiple return values

t = (10, 20, 30)
lo, hi = minmax(8, 3)      # unpacking assignment
print(t[0], t[2], len(t))  # indexing and len
```
Tuples reuse the list heap layout (`[len][e0][e1]…]`) with a distinct tag,
enabling literals, indexing, `len`, unpacking (`a, b = t`) and multiple
return values (`return a, b`). This is the prerequisite for `.shape`,
`reshape`, and `np.zeros((m, n))`.

**Next phases** 🔮 Planned
- Shape as a tuple: `.shape`, `reshape`, `np.zeros((m, n))`.
- Row indexing of a matrix (`m[i]` → 1-D), 2-D item/slice assignment.
- >2 dimensions (variable-length shape), `@` matmul operator.
- More ufuncs/reductions and dtypes (`np.std`, `bool`, `float32`, `dtype=`).

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
- Status: ✅ Completed (see NumPy Phase 7)
- Difficulty: Medium

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
    return (a, b) if a < b else (b, a)

min_val, max_val = min_max(5, 10)
```
- Status: ✅ Completed (see NumPy Phase 7)
- Difficulty: Medium

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
