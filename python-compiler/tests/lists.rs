use inkwell::context::Context;
use python_compiler::*;

#[test]
fn test_list_literal() {
    let source = r#"
x = [1, 2, 3]
print(x)
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();

    // Check that the list literal was lowered correctly
    match &ir[0] {
        ast::IRStmt::Assign { target, value } => {
            assert_eq!(target, "x");
            match value {
                ast::IRExpr::List(elements) => {
                    assert_eq!(elements.len(), 3);
                    assert_eq!(elements[0], ast::IRExpr::Constant(1));
                    assert_eq!(elements[1], ast::IRExpr::Constant(2));
                    assert_eq!(elements[2], ast::IRExpr::Constant(3));
                }
                _ => panic!("Expected List expression"),
            }
        }
        _ => panic!("Expected Assign statement"),
    }

    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_list_indexing() {
    let source = r#"
x = [10, 20, 30]
print(x[0])
print(x[1])
print(x[2])
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();

    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_list_mixed_types() {
    let source = r#"
x = [1, 2.5, 3]
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
fn test_list_assignment_and_access() {
    let source = r#"
numbers = [5, 10, 15, 20]
first = numbers[0]
last = numbers[3]
print(first)
print(last)
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();

    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_list_empty() {
    let source = r#"
x = []
print(x)
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();

    // Check that empty list was lowered correctly
    match &ir[0] {
        ast::IRStmt::Assign { target, value } => {
            assert_eq!(target, "x");
            match value {
                ast::IRExpr::List(elements) => {
                    assert_eq!(elements.len(), 0);
                }
                _ => panic!("Expected List expression"),
            }
        }
        _ => panic!("Expected Assign statement"),
    }

    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_list_with_expressions() {
    let source = r#"
a = 5
b = 10
x = [a, a + b, b * 2]
print(x)
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();

    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}
