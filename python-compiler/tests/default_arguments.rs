use inkwell::context::Context;
use python_compiler::*;

#[test]
fn test_default_argument_simple() {
    let source = r#"
def greet(name, greeting="Hello"):
    return greeting

print(greet("World"))
print(greet("World", "Hi"))
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();

    // Check that function has defaults
    assert_eq!(ir.len(), 3);
    match &ir[0] {
        ast::IRStmt::FunctionDef {
            name,
            params,
            defaults,
            body: _,
        } => {
            assert_eq!(name, "greet");
            assert_eq!(params.len(), 2);
            assert_eq!(defaults.len(), 2);
            assert!(defaults[0].is_none()); // name has no default
            assert!(defaults[1].is_some()); // greeting has default

            // Check default value is "Hello"
            if let Some(ast::IRExpr::StringLiteral(s)) = &defaults[1] {
                assert_eq!(s, "Hello");
            } else {
                panic!("Expected StringLiteral for default value");
            }
        }
        _ => panic!("Expected FunctionDef"),
    }

    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_default_argument_multiple() {
    let source = r#"
def add(a, b=10, c=20):
    return a + b + c

print(add(5))
print(add(5, 15))
print(add(5, 15, 25))
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();

    // Check that function has defaults
    match &ir[0] {
        ast::IRStmt::FunctionDef {
            name,
            params,
            defaults,
            body: _,
        } => {
            assert_eq!(name, "add");
            assert_eq!(params.len(), 3);
            assert_eq!(defaults.len(), 3);
            assert!(defaults[0].is_none()); // a has no default
            assert!(defaults[1].is_some()); // b has default 10
            assert!(defaults[2].is_some()); // c has default 20

            // Check default values
            if let Some(ast::IRExpr::Constant(n)) = &defaults[1] {
                assert_eq!(*n, 10);
            } else {
                panic!("Expected Constant 10 for b default");
            }

            if let Some(ast::IRExpr::Constant(n)) = &defaults[2] {
                assert_eq!(*n, 20);
            } else {
                panic!("Expected Constant 20 for c default");
            }
        }
        _ => panic!("Expected FunctionDef"),
    }

    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_default_argument_expression() {
    let source = r#"
def multiply(x, factor=2):
    return x * factor

print(multiply(5))
print(multiply(5, 3))
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();

    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}
