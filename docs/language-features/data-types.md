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

## Not Supported

### ❌ Complex Numbers

```python
z = 3 + 4j  # ❌ Not supported
```

### ❌ Lists

```python
numbers = [1, 2, 3]  # ❌ Not supported
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
