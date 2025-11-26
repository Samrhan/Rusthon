# Data Types

Rusthon supports Python's basic data types with some limitations.

## Integers

Whole numbers stored as 64-bit signed integers.

### Literals

```python
x = 42
y = -17
z = 0
```

### Range

Integers are stored internally as doubles for the unified type system:
- Safe range: `-2^53` to `2^53` (±9,007,199,254,740,992)
- Beyond this range, precision may be lost

### Operations

```python
a = 10 + 5      # 15
b = 10 - 3      # 7
c = 10 * 4      # 40
d = 10 / 3      # 3.333... (becomes float)
e = 10 % 3      # 1 (modulo)
```

### Type Promotion

Integer division automatically promotes to float:

```python
result = 7 / 2   # 3.5 (float)
```

## Floats

Floating-point numbers (double precision).

### Literals

```python
x = 3.14
y = -0.5
z = 2.0
scientific = 1.5e10  # Scientific notation
```

### Precision

- 64-bit IEEE 754 double precision
- Approximately 15-17 decimal digits of precision

### Operations

```python
a = 3.14 + 2.86  # 6.0
b = 10.5 - 0.5   # 10.0
c = 2.5 * 4.0    # 10.0
d = 7.5 / 2.5    # 3.0
```

### Mixed Integer/Float

Operations with mixed types return float:

```python
x = 10 + 3.14    # 13.14 (float)
y = 5 * 2.0      # 10.0 (float)
```

## Booleans

Logical values: `True` and `False`.

### Literals

```python
x = True
y = False
```

### From Comparisons

```python
is_greater = 10 > 5      # True
is_equal = 3 == 3        # True
is_different = 5 != 5    # False
```

### Logical Operations

```python
a = True and False   # False
b = True or False    # True
c = not True         # False
```

### In Conditionals

```python
active = True

if active:
    print("System is active")

if not active:
    print("System is inactive")
```

### Type Representation

Internally, booleans are stored as PyObjects with a BOOL tag:
- `True` → 1.0 in the payload
- `False` → 0.0 in the payload

### Truthy/Falsy Values

In conditions, values are converted to boolean:
- `0` and `0.0` are falsy
- Non-zero numbers are truthy

```python
if 5:           # Truthy (non-zero)
    print("yes")

if 0:           # Falsy
    print("no")  # Won't execute
```

## Strings

String literals for text manipulation and output.

### Literals

```python
message = "Hello, World!"
name = 'Alice'
empty = ""
```

### Usage

Strings can be:
- Assigned to variables
- Printed with `print()`
- Concatenated with `+`
- Measured with `len()`
- Used as function arguments

```python
greeting = "Hello"
print(greeting)
```

### String Concatenation

Combine strings using the `+` operator:

```python
# Basic concatenation
first = "Hello"
second = " World"
result = first + second
print(result)  # "Hello World"

# Chained concatenation
message = "Hello" + " " + "World"
print(message)  # "Hello World"

# Empty strings
s = "" + "Hello"
print(s)  # "Hello"
```

### String Length

Get the length of a string using the `len()` function:

```python
# Basic usage
text = "Hello"
n = len(text)
print(n)  # 5

# Empty strings
empty = ""
print(len(empty))  # 0

# Inline usage
print(len("Hello World"))  # 11

# With concatenation
s1 = "Hello"
s2 = " World"
combined = s1 + s2
print(len(combined))  # 11
```

### String Operations

✅ **Supported operations:**
```python
# Concatenation
result = "Hello" + " " + "World"  # ✅ Supported

# Length
length = len("hello")             # ✅ Supported

# Multiple arguments in print
print("Hello", "World")           # ✅ Supported
```

❌ **Not yet supported:**
```python
# Indexing
char = "hello"[0]                 # ❌ Not supported

# Methods
upper = "hello".upper()           # ❌ Not supported

# Iteration
for char in "hello":              # ❌ Not supported
    print(char)

# Slicing
substr = "hello"[1:3]             # ❌ Not supported
```

### Memory Management

✅ **Arena Allocation:** Strings are managed using an arena allocator. All allocated strings are automatically freed when the program exits, preventing memory leaks.

```python
# Safe - strings are automatically cleaned up
for i in range(1000):
    s = "Iteration: " + "test"
    print(s)
# All strings freed at program exit
```

**How it works:**
- Each string is allocated with `malloc()`
- Pointers are tracked in a global arena
- All strings are freed at the end of `main()`
- Concatenation creates new strings that are also tracked

## Type System Architecture

### PyObject Structure

All values are stored as PyObjects:
```
struct PyObject {
    tag: i8,      // Type tag (INT=0, FLOAT=1, BOOL=2, STRING=3)
    payload: f64  // Value or pointer
}
```

### Type Tags

| Type | Tag | Payload |
|------|-----|---------|
| Integer | 0 | Value as f64 |
| Float | 1 | IEEE 754 double |
| Boolean | 2 | 0.0 or 1.0 |
| String | 3 | Pointer to C string |

### Type Coercion

Operations automatically handle type promotion:

```python
# Integer + Float → Float
x = 10 + 3.14      # 13.14 (FLOAT)

# Integer + Integer → Integer
y = 10 + 5         # 15 (INT)

# Comparison → Boolean
z = 10 > 5         # True (BOOL)
```

## Lists

Fixed-size lists for storing collections of values.

### Literals

```python
numbers = [1, 2, 3]
mixed = [1, 2.5, 3]
empty = []
```

### Indexing

Access individual elements using zero-based indexing:

```python
numbers = [10, 20, 30, 40]
first = numbers[0]   # 10
second = numbers[1]  # 20
last = numbers[3]    # 40
```

### Usage

Lists can be:
- Assigned to variables
- Indexed with integers
- Printed with `print()`
- Created with mixed types (int, float, bool, string)
- Passed to functions

```python
# Basic usage
x = [1, 2, 3]
print(x)  # [1, 2, 3]

# Mixed types
mixed = [1, 2.5, True, "hello"]
print(mixed)  # [1, 2, True, hello]

# Indexing
value = x[1]
print(value)  # 2
```

### Memory Management

Lists are heap-allocated with automatic memory management:
- List data is allocated with `malloc()`
- Fixed size (cannot grow or shrink after creation)
- Pointers are tracked for cleanup
- Each list element is a PyObject

**Implementation details:**
- Lists are stored as contiguous arrays of PyObject structs
- List pointer and length are encoded in a single PyObject
- Maximum list size: 65,535 elements
- Supports 48-bit pointers (common on x86-64)

### Operations

✅ **Supported operations:**
```python
# Creation
x = [1, 2, 3]                     # ✅ Supported

# Indexing
value = x[0]                      # ✅ Supported

# Printing
print(x)                          # ✅ Supported ([1, 2, 3])

# Mixed types
mixed = [1, 2.5, "hello", True]   # ✅ Supported
```

❌ **Not yet supported:**
```python
# Methods
x.append(4)                       # ❌ Not supported

# Slicing
subset = x[1:3]                   # ❌ Not supported

# Iteration
for item in x:                    # ❌ Not supported
    print(item)

# List comprehensions
squares = [x*x for x in range(5)] # ❌ Not supported

# Negative indexing
last = x[-1]                      # ❌ Not supported

# Assignment to index
x[0] = 10                         # ❌ Not supported

# Length function
n = len(x)                        # ❌ Not supported (yet)
```

### Examples

```python
# Simple list
numbers = [1, 2, 3, 4, 5]
print(numbers)       # [1, 2, 3, 4, 5]
print(numbers[2])    # 3

# List with expressions
a = 5
b = 10
computed = [a, a + b, b * 2]
print(computed)      # [5, 15, 20]

# Passing to functions
def first_element(lst):
    return lst[0]

result = first_element([10, 20, 30])
print(result)        # 10
```

## Not Supported

### ❌ Complex Numbers

```python
z = 3 + 4j  # ❌ Not supported
```

### ❌ Tuples

```python
point = (10, 20)  # ❌ Not supported
```

### ❌ Dictionaries

```python
data = {"key": "value"}  # ❌ Not supported
```

### ❌ Sets

```python
unique = {1, 2, 3}  # ❌ Not supported
```

### ❌ None Type

```python
value = None  # ❌ Not supported
```

## Best Practices

### Use Integers for Counting

```python
# Good
count = 0
for i in range(100):
    count += 1

# Less efficient
count = 0.0
for i in range(100):
    count += 1.0
```

### Use Floats for Scientific

```python
# Good
pi = 3.14159
area = pi * radius * radius

# May lose precision with large values
area = 3 * radius * radius  # Integer 3
```

### Be Explicit with Booleans

```python
# Good
is_valid = True
if is_valid:
    process()

# Less clear
is_valid = 1
if is_valid:
    process()
```

### String Usage is Safe

```python
# Safe - strings are automatically cleaned up at program exit
print("Result:", result)

# Also safe - all strings freed when program ends
for i in range(1000):
    message = "Processing " + "iteration"
    print(message)
```

## Type Checking

Rusthon is dynamically typed - no type annotations needed:

```python
# All valid
x = 42          # Integer
x = 3.14        # Now float
x = True        # Now boolean
x = "hello"     # Now string
```

## See Also

- [Operators](/language-features/operators) - Operations on types
- [Variables](/language-features/variables) - Type inference and assignment
- [Type System Architecture](/architecture/type-system) - Implementation details
