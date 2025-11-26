use inkwell::context::Context;
use python_compiler::*;

#[test]
fn test_complete_program_with_all_features() {
    let source = r#"
# Test all features together
def add(a, b):
    return a + b

def multiply(x, y):
    return x * y

# Variables with integers
a = 5
b = 10

# Variables with floats
pi = 3.14
radius = 2.5

# Function calls
sum_result = add(a, b)
product = multiply(3, 4)

# Arithmetic with mixed types
area = pi * radius * radius
mixed = a + pi

# Nested arithmetic
complex_expr = add(a, b) + multiply(2, 3) - 1.5

# Print results
print(sum_result)
print(product)
print(area)
print(mixed)
print(complex_expr)
"#;

    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();

    // Verify LLVM IR contains expected functions and operations
    assert!(llvm_ir.contains("define double @add"), "Should have add function");
    assert!(llvm_ir.contains("define double @multiply"), "Should have multiply function");
    assert!(llvm_ir.contains("define i32 @main"), "Should have main function");
    assert!(llvm_ir.contains("fadd"), "Should have float addition");
    assert!(llvm_ir.contains("fmul"), "Should have float multiplication");
    assert!(llvm_ir.contains("fsub"), "Should have float subtraction");
    assert!(llvm_ir.contains("@printf"), "Should have printf calls");
}

#[test]
fn test_fibonacci_like_function() {
    let source = r#"
def compute(n):
    a = n * 2
    b = a + 10
    return b

x = 5
result = compute(x)
print(result)
"#;

    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();

    assert!(llvm_ir.contains("define double @compute"), "Should have compute function");
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_input_with_computation() {
    let source = r#"
def double(x):
    return x * 2

a = input()
b = double(a)
c = b + 5
print(c)
"#;

    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();

    assert!(llvm_ir.contains("@scanf"), "Should have scanf call");
    assert!(llvm_ir.contains("define double @double"), "Should have double function");
    assert!(llvm_ir.contains("@printf"), "Should have printf call");
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_multiple_function_calls() {
    let source = r#"
def f1(x):
    return x + 1

def f2(x):
    return f1(x) + 1

def f3(x):
    return f2(x) + 1

result = f3(10)
print(result)
"#;

    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();

    assert!(llvm_ir.contains("define double @f1"), "Should have f1 function");
    assert!(llvm_ir.contains("define double @f2"), "Should have f2 function");
    assert!(llvm_ir.contains("define double @f3"), "Should have f3 function");
    insta::assert_snapshot!(llvm_ir);
}
