# NumPy (compiled subset)

Rusthon compiles a **subset of NumPy** natively. There is no CPython and no
`libnumpy` involved: `import numpy` is recognised by the compiler and the array
operations below are lowered directly to LLVM IR. Element-wise loops are emitted
in the canonical shape LLVM's `O2` pipeline auto-vectorises, so array arithmetic
becomes SIMD code.

Arrays (`ndarray`) are heap objects with an **unboxed, typed, contiguous** data
buffer, unlike Rusthon lists whose elements are individually NaN-boxed.

## Importing

```python
import numpy as np      # alias resolves to the built-in numpy module
import numpy            # np-less form also works (numpy.array(...))
from numpy import array # bound name calls into numpy
```

## Constructors

```python
import numpy as np

a = np.array([1.0, 2.0, 3.0, 4.0])   # from a list
z = np.zeros(3)                       # [0.0, 0.0, 0.0]
o = np.ones(4)                        # [1.0, 1.0, 1.0, 1.0]
r = np.arange(5)                      # [0.0, 1.0, 2.0, 3.0, 4.0]
```

All arrays are 1-dimensional.

## dtypes (`int64` / `float64`)

Arrays are either `int64` or `float64`, inferred the way NumPy does:

```python
import numpy as np

i = np.array([1, 2, 3])     # int64  -> prints [1 2 3]
f = np.array([1.0, 2.0])    # float64 -> prints [1.000000 2.000000]
r = np.arange(5)            # int64   (like NumPy)
z = np.zeros(3)             # float64
```

Arithmetic **promotes** like NumPy: `int op int` stays int, mixing a float makes
the result float, and true division `/` is always float:

```python
a = np.arange(4)            # int64
print(a + 1)                # [1 2 3 4]        (int)
print(a * a)                # [0 1 4 9]        (int)
print(a / 2)                # [0. 0.5 1. 1.5]  (float — true division)
print(a + 0.5)              # [0.5 1.5 2.5 3.5](float — promoted)
print(a.sum())              # 6               (int)
print(a.mean())             # 1.5             (float)
print(np.sqrt(a))           # float           (ufuncs always return float)
```

The dtype is resolved at compile time (including across function boundaries), so
int and float arrays generate separate, fast code. A value that is an int array
on one path and a float array on another (indeterminate dtype) is not supported.

## Element-wise arithmetic and broadcasting

```python
import numpy as np

a = np.array([1.0, 2.0, 3.0, 4.0])
b = np.array([10.0, 20.0, 30.0, 40.0])

c = a + b        # [11.0, 22.0, 33.0, 44.0]
d = a * b        # element-wise product
e = a - 1        # scalar broadcast: [0.0, 1.0, 2.0, 3.0]
f = 2 * a        # scalar broadcast (either side)
g = a / 2        # [0.5, 1.0, 1.5, 2.0]
```

Supported operators: `+`, `-`, `*`, `/`, `%`. Scalars broadcast against arrays
on either side.

## Indexing and item assignment

```python
a = np.array([10.0, 20.0, 30.0])
print(a[0])      # 10.0
print(a[2])      # 30.0

a[1] = 99.0      # in-place item assignment
print(a[1])      # 99.0
```

## Slicing

Slicing returns a **copy** (a new array). Omitted bounds default to the start
and the length; out-of-range bounds are clamped, as in NumPy. A step is not
supported.

```python
a = np.arange(6)       # [0. 1. 2. 3. 4. 5.]
print(a[1:4])          # [1. 2. 3.]
print(a[:2])           # [0. 1.]
print(a[3:])           # [3. 4. 5.]
print(a[:])            # full copy
print(a[2:100])        # clamped -> [2. 3. 4. 5.]
```

## Reductions and size

```python
a = np.array([3.0, 1.0, 4.0, 1.0, 5.0])

print(a.sum())   # 14.0
print(a.mean())  # 2.8
print(a.max())   # 5.0
print(a.min())   # 1.0
print(a.prod())  # 60.0
print(np.sum(a)) # 14.0  (free-function form)
print(np.max(a)) # 5.0
print(len(a))    # 5
print(a.size)    # 5
```

## Element-wise math (ufuncs)

Unary universal functions apply element-wise and return a new array. They lower
to LLVM intrinsics, so the loop auto-vectorizes (e.g. `vsqrtpd`). Applied to a
scalar, they return a scalar — just like NumPy.

```python
import numpy as np

a = np.array([1.0, 4.0, 9.0, 16.0])
print(np.sqrt(a))        # [1. 2. 3. 4.]
print(np.abs(np.array([-1.0, 2.0])))   # [1. 2.]
print(np.exp(np.zeros(3)))             # [1. 1. 1.]
print(np.sqrt(2.0))                    # 1.414214  (scalar)
```

Available: `np.sqrt`, `np.abs`, `np.exp`, `np.log`, `np.sin`, `np.cos`,
`np.floor`, `np.ceil`.

## Linear algebra

```python
x = np.array([1.0, 2.0, 3.0])
y = np.array([4.0, 5.0, 6.0])
print(np.dot(x, y))      # 32.0  (1-D dot product)
```

## Printing

```python
a = np.array([1.0, 2.0, 3.0])
print(a)         # [1.000000 2.000000 3.000000]
```

## Arrays through functions

Arrays can be passed to and returned from user-defined functions, including
transitively and through recursion. The compiler figures out which parameters
and return values are arrays with a whole-program analysis, so no annotations
are needed.

```python
import numpy as np

def scale(v, k):
    return v * k          # element-wise, v is an array parameter

def normalize(v):
    return v / v.sum()    # returns an array

a = np.array([1.0, 2.0, 3.0, 4.0])
b = scale(a, 2.0)         # b is an array
print(b.sum())            # 20.0
print(normalize(a))       # [0.100000 0.200000 0.300000 0.400000]
```

## Constants

```python
print(np.pi)     # 3.141592...
print(np.e)      # 2.718281...
```

## Current boundaries

The subset keeps growing. Not yet supported:

- **dtypes other than `int64`/`float64`** (e.g. `bool`, `float32`), and an
  explicit `dtype=` argument.
- **Multiple dimensions** — arrays are 1-D (the header already carries `ndim`
  so this can grow without a layout change).
- **Slice assignment** (`a[1:3] = ...`), fancy/boolean indexing, negative
  indices, and slice steps. Single-element item assignment and copy-slicing
  *are* supported.
- The `@` matmul operator and 2-D `np.dot`; higher-dimensional linear algebra.
- Much of the wider NumPy API (`reshape`, `.T`, `np.concatenate`, more ufuncs
  such as `np.tanh`/`np.power`, …).

See [Limitations](/limitations) for the full list and the
[Roadmap](/roadmap) for what comes next.
