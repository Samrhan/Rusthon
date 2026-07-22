//! Tests for tuples: literals, indexing, `len`, unpacking, multiple return.

use inkwell::context::Context;
use python_compiler::*;

fn compile(source: &str) -> String {
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    compiler.compile_program(&ir).unwrap()
}

// ---------------------------------------------------------------------------
// Lowering
// ---------------------------------------------------------------------------

#[test]
fn test_tuple_literal_lowers_to_tuple() {
    let source = "t = (1, 2, 3)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    match &ir[0] {
        ast::IRStmt::Assign { target, value } => {
            assert_eq!(target, "t");
            match value {
                ast::IRExpr::Tuple(elts) => {
                    assert_eq!(elts.len(), 3);
                    assert_eq!(elts[0], ast::IRExpr::Constant(1));
                    assert_eq!(elts[2], ast::IRExpr::Constant(3));
                }
                other => panic!("Expected Tuple, got {other:?}"),
            }
        }
        other => panic!("Expected Assign, got {other:?}"),
    }
}

#[test]
fn test_unpacking_lowers_to_unpack() {
    let source = r#"
t = (1, 2)
a, b = t
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    match &ir[1] {
        ast::IRStmt::Unpack { targets, value } => {
            assert_eq!(targets, &["a".to_string(), "b".to_string()]);
            assert_eq!(*value, ast::IRExpr::Variable("t".to_string()));
        }
        other => panic!("Expected Unpack, got {other:?}"),
    }
}

#[test]
fn test_multiple_return_lowers_to_tuple() {
    let source = r#"
def pair(a, b):
    return a, b
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    match &ir[0] {
        ast::IRStmt::FunctionDef { body, .. } => match &body[0] {
            ast::IRStmt::Return(ast::IRExpr::Tuple(elts)) => {
                assert_eq!(elts.len(), 2);
                assert_eq!(elts[0], ast::IRExpr::Variable("a".to_string()));
                assert_eq!(elts[1], ast::IRExpr::Variable("b".to_string()));
            }
            other => panic!("Expected Return(Tuple), got {other:?}"),
        },
        other => panic!("Expected FunctionDef, got {other:?}"),
    }
}

#[test]
fn test_unpack_non_name_target_is_rejected() {
    // `a[0], b = t` (a non-name unpack target) is not supported.
    let source = r#"
xs = [0, 0]
t = (1, 2)
xs[0], b = t
"#;
    let ast = parser::parse_program(source).unwrap();
    assert!(
        lowering::lower_program(&ast).is_err(),
        "unpacking into a non-name target should be rejected"
    );
}

// ---------------------------------------------------------------------------
// Codegen
// ---------------------------------------------------------------------------

#[test]
fn test_tuple_indexing_and_unpacking() {
    let source = r#"
t = (10, 20, 30)
print(t[0])
print(t[2])
print(len(t))
a, b, c = t
print(a)
print(c)
"#;
    insta::assert_snapshot!(compile(source));
}

#[test]
fn test_multiple_return_values() {
    let source = r#"
def minmax(p, q):
    if p < q:
        return p, q
    return q, p

lo, hi = minmax(8, 3)
print(lo)
print(hi)
"#;
    insta::assert_snapshot!(compile(source));
}

#[test]
fn test_tuple_program_compiles() {
    // A mixed tuple program compiles and produces a `main`.
    let source = r#"
def swap(a, b):
    return b, a

x, y = swap(1, 2)
print(x)
print(y)
t = (x, y, x + y)
print(t[2])
"#;
    let ir = compile(source);
    assert!(ir.contains("@main"), "should produce a main function");
}
