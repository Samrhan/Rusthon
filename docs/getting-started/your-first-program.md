# Your First Program

Learn Rusthon by building a complete program step by step.

## Project: Number Guessing Game

We'll build a simple number guessing game that demonstrates most of Rusthon's features.

### Step 1: Basic Structure

```python
def main():
    print("Welcome to the Number Guessing Game!")
    return 0

main()
```

Save as `game.py` and run:
```bash
cargo run -- game.py
./game
```

### Step 2: Add Game Logic

```python
def check_guess(secret, guess):
    if guess < secret:
        print("Too low!")
        return 0
    if guess > secret:
        print("Too high!")
        return 0
    print("Correct!")
    return 1

def main():
    secret = 42
    print("I'm thinking of a number between 1 and 100")
    print("Enter your guess:")

    guess = input()
    result = check_guess(secret, guess)

    return 0

main()
```

### Step 3: Add a Loop

```python
def check_guess(secret, guess):
    if guess < secret:
        print("Too low!")
        return 0
    if guess > secret:
        print("Too high!")
        return 0
    print("Correct!")
    return 1

def main():
    secret = 42
    attempts = 0
    max_attempts = 5
    won = 0

    print("I'm thinking of a number between 1 and 100")
    print("You have 5 attempts")

    while attempts < max_attempts:
        print("Enter your guess:")
        guess = input()
        attempts += 1

        won = check_guess(secret, guess)
        if won == 1:
            attempts = max_attempts  # Exit loop
        else:
            remaining = max_attempts - attempts
            if remaining > 0:
                print("Attempts remaining:")
                print(remaining)

    if won == 1:
        print("You won!")
    else:
        print("Game over! The number was:")
        print(secret)

    return 0

main()
```

## Key Concepts Demonstrated

### 1. Functions
```python
def function_name(param1, param2):
    # function body
    return value
```

Functions can:
- Take multiple parameters
- Return values
- Call other functions
- Call themselves (recursion)

### 2. Variables
```python
x = 10          # Integer
y = 3.14        # Float
result = x + y  # Mixed arithmetic
```

Variables are:
- Dynamically typed
- Automatically promoted (int → float when needed)
- Locally scoped within functions

### 3. Control Flow

**If/Else:**
```python
if condition:
    # executed if condition is true
else:
    # executed if condition is false
```

**While Loop:**
```python
while condition:
    # repeated while condition is true
```

### 4. Operators

**Arithmetic:**
```python
a + b   # Addition
a - b   # Subtraction
a * b   # Multiplication
a / b   # Division
a % b   # Modulo
```

**Comparison:**
```python
a == b  # Equal
a != b  # Not equal
a < b   # Less than
a > b   # Greater than
a <= b  # Less than or equal
a >= b  # Greater than or equal
```

**Bitwise:**
```python
a & b   # AND
a | b   # OR
a ^ b   # XOR
a << b  # Left shift
a >> b  # Right shift
```

**Unary:**
```python
-x      # Negation
+x      # Unary plus
~x      # Bitwise NOT
not x   # Logical NOT
```

**Augmented Assignment:**
```python
x += 5   # x = x + 5
x -= 3   # x = x - 3
x *= 2   # x = x * 2
x /= 4   # x = x / 4
x %= 3   # x = x % 3
x &= 7   # x = x & 7
x |= 3   # x = x | 3
x ^= 5   # x = x ^ 5
x <<= 1  # x = x << 1
x >>= 2  # x = x >> 2
```

### 5. Input/Output

**Print:**
```python
print(42)              # Print integer
print(3.14)            # Print float
print("Hello")         # Print string
print("Value:", x)     # Print multiple values
```

**Input:**
```python
x = input()  # Read a number from stdin
```

## Best Practices

### 1. Always Initialize Variables
```python
# Good
x = 0
x = calculate_value()

# Bad
print(x)  # Error if x not defined
```

### 2. Use Descriptive Names
```python
# Good
counter = 0
max_attempts = 5

# Less clear
c = 0
ma = 5
```

### 3. Keep Functions Focused
```python
# Good - single responsibility
def calculate_area(width, height):
    return width * height

def calculate_perimeter(width, height):
    return 2 * (width + height)

# Less ideal - multiple responsibilities
def calculate_everything(width, height):
    area = width * height
    perimeter = 2 * (width + height)
    # ... lots more calculations
    return area
```

### 4. Comment Complex Logic
```python
def is_prime(n):
    if n <= 1:
        return 0
    if n <= 3:
        return 1

    # Check divisibility up to sqrt(n)
    i = 2
    while i * i <= n:
        if n % i == 0:
            return 0
        i += 1

    return 1
```

## More Examples

### Fibonacci Sequence
```python
def fibonacci(n):
    if n <= 1:
        return n
    return fibonacci(n - 1) + fibonacci(n - 2)

i = 0
while i < 10:
    print(fibonacci(i))
    i += 1
```

### Prime Number Check
```python
def is_prime(n):
    if n <= 1:
        return 0
    if n <= 3:
        return 1

    i = 2
    while i * i <= n:
        if n % i == 0:
            return 0
        i += 1

    return 1

print(is_prime(17))  # 1 (true)
print(is_prime(18))  # 0 (false)
```

### Greatest Common Divisor
```python
def gcd(a, b):
    while b != 0:
        temp = b
        b = a % b
        a = temp
    return a

print(gcd(48, 18))  # 6
```

## Next Steps

- [Language Features](/language-features) - Complete feature reference
- [Limitations](/limitations) - What's not yet supported
- [Examples](https://github.com/Samrhan/Rusthon/tree/main/python-compiler/examples) - More example programs
