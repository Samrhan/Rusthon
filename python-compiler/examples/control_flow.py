# Example demonstrating control flow features
# Comparison operators, if/else, and while loops

# Comparison operators
x = 10
y = 5

print(x > y)   # Should print 1.0 (true)
print(x < y)   # Should print 0.0 (false)
print(x == y)  # Should print 0.0 (false)

# If/else statement
if x > y:
    print(100)
else:
    print(200)

# While loop
counter = 0
while counter < 5:
    print(counter)
    counter = counter + 1

# Nested control flow
i = 0
while i < 3:
    if i > 0:
        print(i)
    i = i + 1
