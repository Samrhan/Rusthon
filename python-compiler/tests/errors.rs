use python_compiler::*;

#[test]
fn test_undefined_variable_error() {
    let source = "print(undefined_var)";
    let ast = parser::parse_program(source);
    assert!(ast.is_ok(), "Parsing should succeed");

    let ir = lowering::lower_program(&ast.unwrap());
    assert!(ir.is_ok(), "Lowering should succeed");

    let context = inkwell::context::Context::create();
    let compiler = codegen::Compiler::new(&context);
    let result = compiler.compile_program(&ir.unwrap());

    assert!(result.is_err(), "Should fail with undefined variable");
    match result {
        Err(codegen::CodeGenError::UndefinedVariable(var)) => {
            assert!(var.contains("undefined_var"), "Error should mention the undefined variable");
        }
        _ => panic!("Expected UndefinedVariable error"),
    }
}

#[test]
fn test_undefined_function_error() {
    let source = "print(undefined_func())";
    let ast = parser::parse_program(source);
    assert!(ast.is_ok(), "Parsing should succeed");

    let ir = lowering::lower_program(&ast.unwrap());
    assert!(ir.is_ok(), "Lowering should succeed");

    let context = inkwell::context::Context::create();
    let compiler = codegen::Compiler::new(&context);
    let result = compiler.compile_program(&ir.unwrap());

    assert!(result.is_err(), "Should fail with undefined function");
}

#[test]
fn test_unsupported_expression() {
    // Test with a feature that's not supported (e.g., list literals)
    let source = "x = [1, 2, 3]";
    let ast = parser::parse_program(source);
    assert!(ast.is_ok(), "Parsing should succeed");

    let ir = lowering::lower_program(&ast.unwrap());
    assert!(ir.is_err(), "Should fail with unsupported expression");
}

#[test]
fn test_parse_error() {
    // Test with invalid Python syntax
    let source = "def (invalid syntax";
    let ast = parser::parse_program(source);
    assert!(ast.is_err(), "Should fail to parse");
}

#[test]
fn test_print_multiple_args_error() {
    // Test with print() having multiple arguments (not supported)
    let source = "print(1, 2, 3)";
    let ast = parser::parse_program(source);
    assert!(ast.is_ok(), "Parsing should succeed");

    let ir = lowering::lower_program(&ast.unwrap());
    assert!(ir.is_err(), "Should fail with wrong number of print arguments");
}
