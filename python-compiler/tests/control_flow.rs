use inkwell::context::Context;
use python_compiler::{codegen::Compiler, lowering::lower_program, parser::parse_program};

fn compile_source(source: &str) -> String {
    let ast = parse_program(source).unwrap();
    let ir = lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = Compiler::new(&context);
    compiler.compile_program(&ir).unwrap()
}

#[test]
fn test_comparison_operators() {
    let source = r#"
x = 5
y = 10
print(x < y)
print(x > y)
print(x == y)
print(x <= y)
print(x >= y)
"#;
    insta::assert_snapshot!(compile_source(source));
}

#[test]
fn test_comparison_with_floats() {
    let source = r#"
x = 3.14
y = 2.71
print(x > y)
print(x == y)
"#;
    insta::assert_snapshot!(compile_source(source));
}

#[test]
fn test_simple_if_statement() {
    let source = r#"
x = 10
if x > 5:
    print(1)
"#;
    insta::assert_snapshot!(compile_source(source));
}

#[test]
fn test_if_else_statement() {
    let source = r#"
x = 3
if x > 5:
    print(1)
else:
    print(0)
"#;
    insta::assert_snapshot!(compile_source(source));
}

#[test]
fn test_nested_if_statements() {
    let source = r#"
x = 10
y = 20
if x < y:
    if x > 5:
        print(1)
    else:
        print(2)
else:
    print(3)
"#;
    insta::assert_snapshot!(compile_source(source));
}

#[test]
fn test_if_with_multiple_statements() {
    let source = r#"
x = 10
if x > 5:
    y = x * 2
    print(y)
else:
    y = x / 2
    print(y)
"#;
    insta::assert_snapshot!(compile_source(source));
}

#[test]
fn test_simple_while_loop() {
    let source = r#"
x = 0
while x < 5:
    print(x)
    x = x + 1
"#;
    insta::assert_snapshot!(compile_source(source));
}

#[test]
fn test_while_loop_with_comparison() {
    let source = r#"
x = 10
while x > 0:
    print(x)
    x = x - 1
"#;
    insta::assert_snapshot!(compile_source(source));
}

#[test]
fn test_nested_while_loops() {
    let source = r#"
i = 0
while i < 3:
    j = 0
    while j < 2:
        print(i)
        print(j)
        j = j + 1
    i = i + 1
"#;
    insta::assert_snapshot!(compile_source(source));
}

#[test]
fn test_while_with_if_inside() {
    let source = r#"
x = 0
while x < 10:
    if x > 5:
        print(x)
    x = x + 1
"#;
    insta::assert_snapshot!(compile_source(source));
}

#[test]
fn test_if_inside_while() {
    let source = r#"
x = 1
while x < 100:
    if x > 50:
        print(x)
    x = x * 2
"#;
    insta::assert_snapshot!(compile_source(source));
}

#[test]
fn test_comparison_in_expression() {
    let source = r#"
x = 5
y = 10
result = x < y
print(result)
"#;
    insta::assert_snapshot!(compile_source(source));
}

#[test]
fn test_complex_condition() {
    let source = r#"
a = 5
b = 10
c = 15
if a < b:
    if b < c:
        print(1)
    else:
        print(0)
"#;
    insta::assert_snapshot!(compile_source(source));
}

#[test]
fn test_equality_comparison() {
    let source = r#"
x = 5
y = 5
if x == y:
    print(1)
else:
    print(0)
"#;
    insta::assert_snapshot!(compile_source(source));
}

#[test]
fn test_not_equal_comparison() {
    let source = r#"
x = 5
y = 10
result = x == y
print(result)
"#;
    insta::assert_snapshot!(compile_source(source));
}
