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

## Indexing

```python
a = np.array([10.0, 20.0, 30.0])
print(a[0])      # 10.0
print(a[2])      # 30.0
```

## Reductions and size

```python
a = np.array([1.0, 2.0, 3.0, 4.0])

print(a.sum())   # 10.0
print(a.mean())  # 2.5
print(np.sum(a)) # 10.0  (free-function form)
print(np.mean(a))# 2.5
print(len(a))    # 4
print(a.size)    # 4
```

## Constants

```python
print(np.pi)     # 3.141592...
print(np.e)      # 2.718281...
```

## Current boundaries

This is a first, deliberately small slice. Not yet supported:

- **dtypes other than `float64`** — every array is `float64`.
- **Multiple dimensions** — arrays are 1-D (the header already carries `ndim`
  so this can grow without a layout change).
- **Slicing** (`a[1:3]`), fancy/boolean indexing, and item assignment
  (`a[0] = 5`).
- **Arrays across user-defined function boundaries** — arrays are not tracked
  through function parameters or return values yet, so passing an array into a
  user `def` (or returning one) is not supported. NumPy calls, operators,
  indexing and reductions on locals all work.
- **Printing an array** — print a scalar element or a reduction instead.
- Most of the wider NumPy API (`reshape`, `dot`/`@`, `max`/`min`, ufuncs, …).

See [Limitations](/limitations) for the full list and the
[Roadmap](/roadmap) for what comes next.
