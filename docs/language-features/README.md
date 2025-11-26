# Language Features

Complete reference of supported Python features in Rusthon.

## Overview

Rusthon supports a useful subset of Python focused on:
- Numerical computation
- Algorithmic programming
- Control flow
- Functions and recursion

## Feature Matrix

| Feature | Status | Details |
|---------|--------|---------|
| **Data Types** |  |  |
| Integers | ✅ Full | `/language-features/data-types#integers` |
| Floats | ✅ Full | `/language-features/data-types#floats` |
| Booleans | ✅ Full (True/False) | `/language-features/data-types#booleans` |
| Strings | ✅ Literals only | `/language-features/data-types#strings` |
| Lists | ❌ Not supported | See `/limitations` |
| Dictionaries | ❌ Not supported | See `/limitations` |
| **Operators** |  |  |
| Arithmetic | ✅ Full (+, -, *, /, %) | `/language-features/operators#arithmetic` |
| Comparison | ✅ Full (==, !=, <, >, <=, >=) | `/language-features/operators#comparison` |
| Bitwise | ✅ Full (&, \|, ^, <<, >>) | `/language-features/operators#bitwise` |
| Unary | ✅ Full (-, +, ~, not) | `/language-features/operators#unary` |
| Augmented | ✅ Full (+=, -=, etc.) | `/language-features/operators#augmented` |
| **Control Flow** |  |  |
| If/Else/Elif | ✅ Full | `/language-features/control-flow#if-else-elif` |
| While | ✅ Full | `/language-features/control-flow#while` |
| For loops | ✅ Range-based | `/language-features/control-flow#for-loops` |
| Break/Continue | ✅ Full | `/language-features/control-flow#break-continue` |
| **Functions** |  |  |
| Definition | ✅ Full | `/language-features/functions#definition` |
| Parameters | ✅ Multiple | `/language-features/functions#parameters` |
| Return | ✅ Full | `/language-features/functions#return` |
| Recursion | ✅ Full | `/language-features/functions#recursion` |
| Default args | ❌ Not supported | See `/limitations` |
| **I/O** |  |  |
| print() | ✅ Full | `/language-features/input-output#print` |
| input() | ✅ Numbers only | `/language-features/input-output#input` |

## Quick Examples

### Arithmetic
```python
x = 10 + 20 * 3  # 70
y = x / 2        # 35.0
z = y % 7        # 0.0
```

### Functions
```python
def factorial(n):
    if n <= 1:
        return 1
    return n * factorial(n - 1)

print(factorial(5))  # 120
```

### Loops
```python
# For loop with range
for i in range(10):
    print(i)

# While loop
sum = 0
i = 0
while i < 10:
    sum += i
    i += 1
print(sum)  # 45
```

### Control Flow
```python
# elif support
x = 5
if x < 0:
    print("negative")
elif x == 0:
    print("zero")
else:
    print("positive")

# break and continue
for i in range(10):
    if i == 3:
        continue  # skip 3
    if i == 7:
        break     # stop at 7
    print(i)      # prints 0,1,2,4,5,6
```

### Boolean Literals
```python
x = True
y = False

if x and not y:
    print("x is True and y is False")
```

### Bitwise
```python
x = 12 & 10  # 8
y = 12 | 10  # 14
z = 5 << 2   # 20
```

## Documentation Sections

- [Data Types](/language-features/data-types) - Integers, floats, booleans, strings
- [Operators](/language-features/operators) - All supported operators
- [Variables](/language-features/variables) - Assignment and scoping
- [Functions](/language-features/functions) - Definitions and calls
- [Control Flow](/language-features/control-flow) - If/else and while
- [Input/Output](/language-features/input-output) - Print and input

## Next Steps

- [Your First Program](/getting-started/your-first-program) - Learn by example
- [Limitations](/limitations) - What's not supported
- [Examples](https://github.com/Samrhan/Rusthon/tree/main/python-compiler/examples) - Browse examples
