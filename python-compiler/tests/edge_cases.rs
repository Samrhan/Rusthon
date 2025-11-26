use inkwell::context::Context;
use python_compiler::*;

#[test]
fn test_division_by_zero_literal() {
    // Division by zero should compile (runtime error)
    let source = "print(10 / 0)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_division_by_zero_variable() {
    let source = r#"
x = 0
print(10 / x)
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_modulo_by_zero() {
    let source = "print(10 % 0)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_large_integers() {
    // Test with large integer values
    let source = "print(999999999 + 999999999)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_negative_large_integers() {
    let source = "print(-999999999 - 999999999)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_very_small_float() {
    let source = "print(0.0000001)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_very_large_float() {
    let source = "print(999999999.999999)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_zero_operations() {
    let source = r#"
print(0 + 0)
print(0 - 0)
print(0 * 0)
print(0 & 0)
print(0 | 0)
print(0 ^ 0)
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_negative_shifts() {
    // Shifting by negative amounts (undefined behavior in many languages)
    let source = r#"
x = -2
print(8 << x)
print(8 >> x)
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_large_shifts() {
    // Shifting by amounts larger than the bit width
    let source = r#"
print(1 << 100)
print(1000 >> 100)
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_deep_recursion() {
    // Function that calls itself many times
    let source = r#"
def countdown(n):
    if n > 0:
        print(n)
        countdown(n - 1)
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

#[test]
fn test_mutual_recursion() {
    // Two functions calling each other
    let source = r#"
def is_even(n):
    if n == 0:
        return 1
    return is_odd(n - 1)

def is_odd(n):
    if n == 0:
        return 0
    return is_even(n - 1)

print(is_even(4))
print(is_odd(4))
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_empty_function() {
    // Function with no statements
    let source = r#"
def empty():
    return 0

print(empty())
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_function_with_only_return() {
    let source = r#"
def get_value():
    return 42

print(get_value())
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_deeply_nested_loops() {
    let source = r#"
i = 0
while i < 2:
    j = 0
    while j < 2:
        k = 0
        while k < 2:
            print(i + j + k)
            k += 1
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
fn test_deeply_nested_if() {
    let source = r#"
x = 5
if x > 0:
    if x > 2:
        if x > 4:
            if x > 3:
                print(1)
            else:
                print(2)
        else:
            print(3)
    else:
        print(4)
else:
    print(5)
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_infinite_loop_structure() {
    // While true should compile (will run forever at runtime)
    let source = r#"
x = 1
while x > 0:
    x += 1
print(x)
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_float_precision() {
    let source = r#"
x = 0.1 + 0.2
print(x)
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_many_variables() {
    let source = r#"
a = 1
b = 2
c = 3
d = 4
e = 5
f = 6
g = 7
h = 8
i = 9
j = 10
print(a + b + c + d + e + f + g + h + i + j)
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_many_function_parameters() {
    let source = r#"
def sum_many(a, b, c, d, e, f, g, h):
    return a + b + c + d + e + f + g + h

print(sum_many(1, 2, 3, 4, 5, 6, 7, 8))
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}
