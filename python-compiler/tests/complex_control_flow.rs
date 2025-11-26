use inkwell::context::Context;
use python_compiler::*;

#[test]
fn test_nested_loops_with_conditions() {
    let source = r#"
i = 0
while i < 3:
    j = 0
    while j < 3:
        if i == j:
            print(1)
        else:
            print(0)
        j += 1
    i += 1
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_loop_with_multiple_conditions() {
    let source = r#"
x = 0
while x < 10:
    if x < 3:
        print(1)
    else:
        if x < 7:
            print(2)
        else:
            print(3)
    x += 1
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_factorial_iterative() {
    let source = r#"
n = 5
result = 1
i = 1
while i <= n:
    result *= i
    i += 1
print(result)
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_factorial_recursive() {
    let source = r#"
def factorial(n):
    if n <= 1:
        return 1
    return n * factorial(n - 1)

print(factorial(5))
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_fibonacci_iterative() {
    let source = r#"
n = 10
a = 0
b = 1
counter = 0
while counter < n:
    print(a)
    temp = a
    a = b
    b = temp + b
    counter += 1
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_fibonacci_recursive() {
    let source = r#"
def fib(n):
    if n <= 1:
        return n
    return fib(n - 1) + fib(n - 2)

print(fib(8))
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_gcd_euclidean() {
    let source = r#"
def gcd(a, b):
    while b != 0:
        temp = b
        b = a % b
        a = temp
    return a

print(gcd(48, 18))
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_prime_check() {
    let source = r#"
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

print(is_prime(17))
print(is_prime(18))
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_collatz_sequence() {
    let source = r#"
n = 10
while n != 1:
    print(n)
    if n % 2 == 0:
        n = n / 2
    else:
        n = 3 * n + 1
print(n)
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_power_function() {
    let source = r#"
def power(base, exp):
    result = 1
    while exp > 0:
        result *= base
        exp -= 1
    return result

print(power(2, 10))
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_sum_of_digits() {
    let source = r#"
def sum_digits(n):
    sum = 0
    while n > 0:
        sum += n % 10
        n = n / 10
    return sum

print(sum_digits(12345))
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_nested_function_calls() {
    let source = r#"
def add(a, b):
    return a + b

def multiply(a, b):
    return a * b

def compute(x, y, z):
    return add(multiply(x, y), z)

print(compute(2, 3, 4))
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_ackermann_function() {
    let source = r#"
def ackermann(m, n):
    if m == 0:
        return n + 1
    if n == 0:
        return ackermann(m - 1, 1)
    return ackermann(m - 1, ackermann(m, n - 1))

print(ackermann(2, 2))
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_binary_search() {
    let source = r#"
def binary_search_helper(target, low, high):
    if low > high:
        return -1
    mid = (low + high) / 2
    if mid == target:
        return mid
    if mid < target:
        return binary_search_helper(target, mid + 1, high)
    return binary_search_helper(target, low, mid - 1)

print(binary_search_helper(7, 0, 10))
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_multiple_returns_in_function() {
    let source = r#"
def classify(x):
    if x < 0:
        return -1
    if x == 0:
        return 0
    return 1

print(classify(-5))
print(classify(0))
print(classify(5))
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_complex_boolean_logic() {
    let source = r#"
x = 5
y = 10
z = 15

if x < y:
    if y < z:
        print(1)
    else:
        print(2)
else:
    if x < z:
        print(3)
    else:
        print(4)
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_loop_with_early_condition() {
    let source = r#"
i = 0
found = 0
while i < 100:
    if i == 42:
        found = 1
        i = 100
    i += 1
print(found)
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_countdown_with_print() {
    let source = r#"
def countdown(n):
    while n > 0:
        print(n)
        n -= 1
    return 0

countdown(5)
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}
