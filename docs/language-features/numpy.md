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

All arrays are 1-dimensional and hold `float64` elements.

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
print(np.sum(a)) # 14.0  (free-function form)
print(np.max(a)) # 5.0
print(len(a))    # 5
print(a.size)    # 5
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

- **dtypes other than `float64`** — every array is `float64`.
- **Multiple dimensions** — arrays are 1-D (the header already carries `ndim`
  so this can grow without a layout change).
- **Slice assignment** (`a[1:3] = ...`), fancy/boolean indexing, negative
  indices, and slice steps. Single-element item assignment and copy-slicing
  *are* supported.
- Most of the wider NumPy API (`reshape`, `dot`/`@`, ufuncs like `np.sqrt`, …).

See [Limitations](/limitations) for the full list and the
[Roadmap](/roadmap) for what comes next.
