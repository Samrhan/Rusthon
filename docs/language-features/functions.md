# Functions

Functions allow you to organize code into reusable blocks with parameters and return values.

## Definition

Define functions using the `def` keyword:

```python
def greet(name):
    return "Hello, " + name

result = greet("World")
print(result)  # "Hello, World"
```

### Syntax

```python
def function_name(param1, param2, ...):
    # function body
    return value
```

## Parameters

Functions can have zero or more parameters:

```python
# No parameters
def say_hello():
    print("Hello!")

# Single parameter
def square(x):
    return x * x

# Multiple parameters
def add(a, b):
    return a + b

# Many parameters
def calculate(x, y, z, w):
    return (x + y) * (z - w)
```

### Parameter Types

All Python types are supported as parameters:

```python
def process_data(count, scale, enabled, message):
    # count: int
    # scale: float
    # enabled: bool
    # message: string
    if enabled:
        return count * scale
    return 0

result = process_data(10, 2.5, True, "Processing")
print(result)  # 25.0
```

## Default Arguments

Functions can have default values for parameters, allowing them to be called with fewer arguments:

```python
def greet(name, greeting="Hello"):
    return greeting + ", " + name

# Call with all arguments
print(greet("Alice", "Hi"))      # "Hi, Alice"

# Call with default greeting
print(greet("Bob"))               # "Hello, Bob"
```

### Multiple Defaults

Multiple parameters can have default values:

```python
def create_message(text, prefix="INFO", suffix=""):
    return prefix + ": " + text + suffix

# All defaults
print(create_message("Started"))                    # "INFO: Started"

# Override prefix
print(create_message("Error occurred", "ERROR"))    # "ERROR: Error occurred"

# Override both
print(create_message("Done", "SUCCESS", "!"))       # "SUCCESS: Done!"
```

### Default Value Rules

- Default parameters must come after non-default parameters
- Default values are evaluated when the function is defined
- Default values can be any valid expression

```python
# Valid: defaults after required parameters
def valid(a, b, c=10, d=20):
    return a + b + c + d

# Valid: expression as default
def with_expression(x, multiplier=2):
    return x * multiplier

# Valid: all parameters have defaults
def all_defaults(a=1, b=2, c=3):
    return a + b + c
```

## Return

Functions return values using the `return` keyword:

```python
def multiply(a, b):
    return a * b

result = multiply(6, 7)
print(result)  # 42
```

### Implicit Return

Functions without a `return` statement implicitly return nothing (implementation detail: they may return a default value):

```python
def print_twice(x):
    print(x)
    print(x)

print_twice(5)  # Prints 5 twice, returns nothing
```

### Early Return

Use `return` to exit a function early:

```python
def abs_value(x):
    if x < 0:
        return -x
    return x

print(abs_value(-5))  # 5
print(abs_value(5))   # 5
```

## Recursion

Functions can call themselves recursively:

```python
def factorial(n):
    if n <= 1:
        return 1
    return n * factorial(n - 1)

print(factorial(5))  # 120
```

### Recursive Examples

```python
# Fibonacci sequence
def fibonacci(n):
    if n <= 1:
        return n
    return fibonacci(n - 1) + fibonacci(n - 2)

print(fibonacci(10))  # 55

# Sum of digits
def sum_digits(n):
    if n == 0:
        return 0
    return n % 10 + sum_digits(n / 10)

print(sum_digits(123))  # 6

# Power function
def power(base, exp):
    if exp == 0:
        return 1
    return base * power(base, exp - 1)

print(power(2, 10))  # 1024
```

## Function Calls

### Basic Calls

Call functions by name with parentheses:

```python
def add(a, b):
    return a + b

result = add(3, 4)
print(result)  # 7
```

### Nested Calls

Functions can call other functions:

```python
def double(x):
    return x * 2

def quad(x):
    return double(double(x))

print(quad(5))  # 20
```

### Passing Functions Results

Use function results as arguments:

```python
def add(a, b):
    return a + b

def multiply(x, y):
    return x * y

result = multiply(add(2, 3), add(4, 5))
print(result)  # 45 (5 * 9)
```

## Variable Scope

### Local Variables

Variables defined in a function are local to that function:

```python
def calculate():
    x = 10  # Local variable
    return x * 2

result = calculate()
print(result)  # 20
# print(x)  # Error: x not defined
```

### Function Parameters

Parameters are local variables initialized with argument values:

```python
def greet(name):
    # name is a local variable
    message = "Hello, " + name
    return message

print(greet("Alice"))  # "Hello, Alice"
```

## Advanced Examples

### Mathematical Functions

```python
def gcd(a, b):
    while b != 0:
        temp = b
        b = a % b
        a = temp
    return a

print(gcd(48, 18))  # 6

def is_prime(n):
    if n <= 1:
        return False
    i = 2
    while i * i <= n:
        if n % i == 0:
            return False
        i += 1
    return True

print(is_prime(17))  # True
print(is_prime(20))  # False
```

### Working with Lists

```python
def sum_list(numbers):
    total = 0
    for i in range(len(numbers)):
        total += numbers[i]
    return total

# Note: This would require len() to work on lists
# which is planned for future implementation

def get_first(lst):
    return lst[0]

def get_last(lst):
    return lst[len(lst) - 1]

data = [10, 20, 30, 40]
print(get_first(data))  # 10
```

### Default Arguments Use Cases

```python
# Configuration with sensible defaults
def configure(debug=False, verbose=False, timeout=30):
    print("Debug:", debug)
    print("Verbose:", verbose)
    print("Timeout:", timeout)

configure()                          # All defaults
configure(True)                      # debug=True, others default
configure(True, True)                # debug=True, verbose=True
configure(True, True, 60)            # All specified

# Flexible formatting
def format_number(value, decimals=2, padding=8):
    # Format number with defaults
    return value

# Range-like functions
def generate_sequence(start, end=10, step=1):
    result = []
    current = start
    while current < end:
        # Add to result
        current += step
    return result
```

## Not Supported

### ❌ Keyword Arguments

```python
# Named arguments not supported
result = add(a=5, b=3)  # ❌ Not supported
```

### ❌ Variable Arguments

```python
# *args and **kwargs not supported
def varargs(*args):     # ❌ Not supported
    pass

def keywords(**kwargs): # ❌ Not supported
    pass
```

### ❌ Lambda Functions

```python
# Anonymous functions not supported
square = lambda x: x * x  # ❌ Not supported
```

### ❌ Decorators

```python
# Function decorators not supported
@decorator              # ❌ Not supported
def my_function():
    pass
```

### ❌ Generators

```python
# Yield and generators not supported
def gen():
    yield 1  # ❌ Not supported
```

### ❌ Closures

```python
# Nested function definitions and closures
def outer():
    x = 10
    def inner():  # ❌ Not supported
        return x
    return inner
```

## Best Practices

### Keep Functions Focused

Each function should do one thing well:

```python
# Good: single responsibility
def calculate_area(width, height):
    return width * height

def calculate_perimeter(width, height):
    return 2 * (width + height)

# Less good: doing too much
def calculate_everything(width, height):
    area = width * height
    perimeter = 2 * (width + height)
    # ... many more calculations
    return area
```

### Use Descriptive Names

Choose clear, descriptive function names:

```python
# Good
def calculate_tax(amount, rate):
    return amount * rate

def is_valid_email(email):
    # validation logic
    return True

# Less clear
def calc(a, r):
    return a * r

def check(e):
    return True
```

### Provide Sensible Defaults

Default arguments should represent common use cases:

```python
# Good: common default
def connect(host, port=80, timeout=30):
    pass

# Less useful: no obvious default
def connect(host, port=12345, timeout=1):
    pass
```

## See Also

- [Control Flow](/language-features/control-flow) - Using functions with if/while
- [Data Types](/language-features/data-types) - Parameter and return types
- [Examples](https://github.com/Samrhan/Rusthon/tree/main/python-compiler/examples) - More function examples
