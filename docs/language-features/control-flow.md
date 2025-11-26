# Control Flow

Rusthon supports standard Python control flow structures for conditional execution and loops.

## If/Else/Elif

### Basic If Statement

Execute code conditionally:

```python
x = 10

if x > 5:
    print("x is greater than 5")
```

### If/Else

Execute one block or another:

```python
x = 3

if x > 5:
    print("x is greater than 5")
else:
    print("x is 5 or less")
```

### Elif Chains

Test multiple conditions in sequence:

```python
score = 85

if score >= 90:
    print("A")
elif score >= 80:
    print("B")
elif score >= 70:
    print("C")
elif score >= 60:
    print("D")
else:
    print("F")
```

**How it works:** `elif` is compiled as nested if statements in the else clause, providing efficient conditional branching.

### Nested If Statements

You can nest if statements arbitrarily:

```python
x = 10
y = 20

if x > 5:
    if y > 15:
        print("Both conditions met")
    else:
        print("Only x > 5")
else:
    print("x <= 5")
```

## While Loops

Execute code repeatedly while a condition is true:

### Basic While Loop

```python
i = 0
while i < 5:
    print(i)
    i += 1
# Prints: 0, 1, 2, 3, 4
```

### Infinite Loop with Break

```python
count = 0
while True:
    count += 1
    if count == 10:
        break
    print(count)
# Prints: 1 through 9
```

### Using Continue

```python
i = 0
while i < 10:
    i += 1
    if i % 2 == 0:
        continue  # Skip even numbers
    print(i)
# Prints: 1, 3, 5, 7, 9
```

### Nested While Loops

```python
i = 1
while i <= 3:
    j = 1
    while j <= 3:
        print(i, j)
        j += 1
    i += 1
```

## For Loops

Range-based for loops for iterating over sequences of numbers.

### For with range(end)

Iterate from 0 to end-1:

```python
for i in range(5):
    print(i)
# Prints: 0, 1, 2, 3, 4
```

### For with range(start, end)

Iterate from start to end-1:

```python
for i in range(2, 7):
    print(i)
# Prints: 2, 3, 4, 5, 6
```

### For with Break and Continue

```python
for i in range(10):
    if i == 3:
        continue  # Skip 3
    if i == 7:
        break     # Stop at 7
    print(i)
# Prints: 0, 1, 2, 4, 5, 6
```

### Nested For Loops

```python
for i in range(3):
    for j in range(3):
        print(i, j)
```

### Countdown Example

```python
for i in range(10, 0, -1):  # ❌ Step not supported
    print(i)

# Use this instead:
i = 10
while i > 0:
    print(i)
    i -= 1
```

## Break and Continue

### Break

Exit the current loop immediately:

```python
# Find first number divisible by 7
for i in range(100):
    if i % 7 == 0 and i != 0:
        print(i)
        break
# Prints: 7
```

Break works in both while and for loops:

```python
i = 0
while True:
    if i >= 10:
        break
    print(i)
    i += 1
```

### Continue

Skip to the next iteration:

```python
# Print odd numbers only
for i in range(10):
    if i % 2 == 0:
        continue
    print(i)
# Prints: 1, 3, 5, 7, 9
```

### Break in Nested Loops

Break only exits the innermost loop:

```python
for i in range(3):
    for j in range(3):
        if j == 1:
            break  # Only breaks inner loop
        print(i, j)
# Prints: (0,0), (1,0), (2,0)
```

## Common Patterns

### Sum of Numbers

```python
total = 0
for i in range(1, 101):
    total += i
print(total)  # 5050
```

### Factorial

```python
def factorial(n):
    result = 1
    for i in range(1, n + 1):
        result *= i
    return result

print(factorial(5))  # 120
```

### Finding Maximum

```python
max_val = 0
for i in range(10):
    value = i * i
    if value > max_val:
        max_val = value
print(max_val)  # 81
```

### Early Exit on Condition

```python
found = False
for i in range(100):
    if i * i == 64:
        print("Found:", i)
        found = True
        break

if not found:
    print("Not found")
```

### Skip Invalid Values

```python
for i in range(-5, 6):
    if i == 0:
        continue  # Skip division by zero
    print(10 / i)
```

## Limitations

### ❌ For Loop Step Parameter

```python
# Not supported
for i in range(0, 10, 2):
    print(i)

# Use while instead
i = 0
while i < 10:
    print(i)
    i += 2
```

### ❌ For Loop Over Collections

```python
# Not supported - no list/string iteration
for item in my_list:
    print(item)

for char in "hello":
    print(char)
```

### ❌ Else Clause on Loops

```python
# Not supported
for i in range(10):
    if i == 5:
        break
else:
    print("Completed without break")
```

## Best Practices

### Use For Loops for Known Ranges

```python
# Good - known iteration count
for i in range(10):
    print(i * i)

# Less clear
i = 0
while i < 10:
    print(i * i)
    i += 1
```

### Use While for Unknown Iterations

```python
# Good - condition-based termination
while not found:
    # search logic
    if condition:
        found = True
```

### Avoid Infinite Loops

```python
# Bad - may run forever
while True:
    # no break condition

# Good - clear termination
max_iterations = 1000
count = 0
while True:
    count += 1
    if count >= max_iterations:
        break
    # loop body
```

### Clear Break Conditions

```python
# Good - clear purpose
for i in range(1000):
    if found_answer:
        break
    # search logic

# Less clear
for i in range(1000):
    if x == y and a > b or c != d:
        break
```

## Performance Notes

- For loops are compiled to while loops internally
- Break/continue compile to direct LLVM branches
- No performance difference between for and while
- Nested loops are fully optimized by LLVM

## See Also

- [Functions](/language-features/functions) - Using control flow in functions
- [Operators](/language-features/operators) - Comparison and logical operators
- [Examples](https://github.com/Samrhan/Rusthon/tree/main/python-compiler/examples) - More control flow examples
