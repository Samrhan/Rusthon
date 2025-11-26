# Limitations

Current limitations and unsupported features in Rusthon.

## Unsupported Language Features

### Control Flow

⚠️ **For Loops - Limited Support**
```python
# ✅ Supported: range-based for loops
for i in range(10):        # ✅ Works
    print(i)

for i in range(5, 15):     # ✅ Works
    print(i)

# ❌ Not supported: iterating over collections
for item in my_list:       # ❌ Lists not supported
    print(item)

for char in "hello":       # ❌ String iteration not supported
    print(char)

for i in range(0, 10, 2):  # ❌ Step parameter not supported
    print(i)
```

❌ **Try/Except**
```python
# Not supported
try:
    risky_operation()
except Exception:
    handle_error()

# No workaround - exceptions not supported
```

### Data Structures

❌ **Lists**
```python
# Not supported
my_list = [1, 2, 3]
my_list.append(4)

# No direct workaround
# Use separate variables or simulate with functions
```

❌ **Tuples**
```python
# Not supported
my_tuple = (1, 2, 3)

# No direct workaround
```

❌ **Dictionaries**
```python
# Not supported
my_dict = {"key": "value"}

# No direct workaround
```

❌ **Sets**
```python
# Not supported
my_set = {1, 2, 3}

# No direct workaround
```

### String Operations

❌ **String Concatenation**
```python
# Not supported
result = "Hello" + " " + "World"

# Workaround: Use separate prints
print("Hello")
print(" ")
print("World")
```

❌ **String Methods**
```python
# Not supported
text = "hello"
upper = text.upper()
length = len(text)

# No workaround
```

❌ **String Indexing**
```python
# Not supported
text = "hello"
first_char = text[0]

# No workaround
```

❌ **F-strings**
```python
# Not supported
name = "Alice"
print(f"Hello, {name}")

# Workaround: Separate prints
print("Hello,")
print(name)
```

### Functions

❌ **Default Arguments**
```python
# Not supported
def greet(name="World"):
    print("Hello", name)

# Workaround: Check inside function
def greet(name):
    if name == 0:  # Use sentinel value
        print("Hello World")
    else:
        print("Hello", name)
```

❌ **Keyword Arguments**
```python
# Not supported
def func(a, b, c):
    return a + b + c

result = func(c=3, a=1, b=2)

# Use positional arguments only
result = func(1, 2, 3)
```

❌ **Variable Arguments (*args, **kwargs)**
```python
# Not supported
def sum_all(*numbers):
    total = 0
    for n in numbers:
        total += n
    return total

# Use fixed parameters
def sum_three(a, b, c):
    return a + b + c
```

❌ **Lambda Functions**
```python
# Not supported
square = lambda x: x * x

# Use def instead
def square(x):
    return x * x
```

### Object-Oriented Programming

❌ **Classes**
```python
# Not supported
class Point:
    def __init__(self, x, y):
        self.x = x
        self.y = y

# Use functions and separate variables
def create_point(x, y):
    return x  # Can only return one value

def point_x(p):
    return p
```

❌ **Methods**
```python
# Not supported
class MyClass:
    def method(self):
        pass

# Use standalone functions
```

### Modules and Imports

❌ **Import**
```python
# Not supported
import math
from os import path

# All code must be in single file
```

❌ **Multiple Files**
```python
# Not supported
# Can't split code across multiple files

# All code must be in single .py file
```

### Advanced Features

❌ **List Comprehensions**
```python
# Not supported
squares = [x**2 for x in range(10)]

# Use loop instead
squares = 0  # Can't create list anyway
i = 0
while i < 10:
    square = i * i
    # Process square
    i += 1
```

❌ **Generators**
```python
# Not supported
def count_up_to(n):
    i = 0
    while i < n:
        yield i
        i += 1

# Use regular functions
```

❌ **Decorators**
```python
# Not supported
@decorator
def function():
    pass

# Use regular functions
```

❌ **Context Managers (with)**
```python
# Not supported
with open("file.txt") as f:
    content = f.read()

# File I/O not supported anyway
```

## Type System Limitations

### Integer Range

❌ **Large Integers**
```python
# Integers stored as doubles (±2^53)
big = 9007199254740993  # May lose precision
print(big)              # Might print wrong value

# Workaround: Use floats or stay within ±2^53
```

### Arbitrary Precision

❌ **BigInt**
```python
# Not supported
huge = 10 ** 100  # Would overflow

# No workaround
```

### Type Annotations

❌ **Type Hints**
```python
# Not supported
def add(a: int, b: int) -> int:
    return a + b

# Just remove annotations
def add(a, b):
    return a + b
```

## I/O Limitations

### Input

⚠️ **Input Returns Float**
```python
x = input()  # Always returns float
# Can't read strings or parse integers differently
```

❌ **File I/O**
```python
# Not supported
with open("file.txt") as f:
    data = f.read()

# No workaround
```

❌ **Command-Line Arguments**
```python
# Not supported
import sys
args = sys.argv

# No workaround
```

### Output

⚠️ **Print Formatting**
```python
# Limited formatting
print(3.14159)  # Prints full precision
# Can't do: print(f"{pi:.2f}")

# No workaround
```

## Memory Limitations

### String Memory Leaks

⚠️ **Strings Never Freed**
```python
# Every string literal allocates memory that's never freed
while True:  # Don't do this!
    print("leak")  # Each iteration leaks memory

# Avoid: Long-running programs with many string prints
```

### Stack Overflow

⚠️ **Deep Recursion**
```python
def factorial(n):
    if n <= 1:
        return 1
    return n * factorial(n - 1)

factorial(100000)  # Will stack overflow

# Use iteration instead
def factorial(n):
    result = 1
    i = 1
    while i <= n:
        result *= i
        i += 1
    return result
```

## Runtime Limitations

### No Runtime Library

❌ **Standard Library**
```python
# Not supported
import random
import datetime
import json

# Only built-in features available
```

### Error Handling

⚠️ **Runtime Errors Crash**
```python
x = 10 / 0  # Crashes program
# No try/except to catch

# Workaround: Check before operation
if y != 0:
    x = 10 / y
```

## Workarounds Summary

| Limitation | Workaround | Quality |
|------------|------------|---------|
| For with step | Use while | ✅ Good |
| For over collections | N/A - collections not supported | ❌ N/A |
| Lists | Separate variables | ❌ Poor |
| String concat | Separate prints | ⚠️ Okay |
| Default args | Sentinel values | ⚠️ Okay |
| Classes | Functions | ❌ Poor |
| Large integers | Stay in range | ✅ Good |
| Deep recursion | Use iteration | ✅ Good |
| String leaks | Avoid strings | ❌ Poor |

## Future Improvements

See [Roadmap](/roadmap) for planned features.

## Next Steps

- [Roadmap](/roadmap) - Planned features
- [Language Features](/language-features) - Supported features
- [Contributing](/contributing) - Help add features
