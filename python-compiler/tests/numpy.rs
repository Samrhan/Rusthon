//! Tests for the compiled NumPy subset (`ndarray`) and the generic module
//! system that routes `np.array(...)` / `arr.sum()` to it.

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
// Lowering: imports resolve to module/method/attribute IR nodes
// ---------------------------------------------------------------------------

#[test]
fn test_import_alias_lowers_to_module_call() {
    let source = r#"
import numpy as np
x = np.array([1.0, 2.0, 3.0])
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();

    // The `import` statement itself produces no IR.
    assert_eq!(ir.len(), 1, "import should not emit a statement");
    match &ir[0] {
        ast::IRStmt::Assign { target, value } => {
            assert_eq!(target, "x");
            match value {
                ast::IRExpr::ModuleCall { module, func, args } => {
                    assert_eq!(module, "numpy", "alias np should resolve to numpy");
                    assert_eq!(func, "array");
                    assert_eq!(args.len(), 1);
                }
                other => panic!("Expected ModuleCall, got {other:?}"),
            }
        }
        other => panic!("Expected Assign, got {other:?}"),
    }
}

#[test]
fn test_method_call_lowers_to_method_call() {
    let source = r#"
import numpy as np
a = np.array([1.0, 2.0])
s = a.sum()
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();

    match &ir[1] {
        ast::IRStmt::Assign { target, value } => {
            assert_eq!(target, "s");
            match value {
                ast::IRExpr::MethodCall {
                    receiver,
                    method,
                    args,
                } => {
                    assert_eq!(**receiver, ast::IRExpr::Variable("a".to_string()));
                    assert_eq!(method, "sum");
                    assert!(args.is_empty());
                }
                other => panic!("Expected MethodCall, got {other:?}"),
            }
        }
        other => panic!("Expected Assign, got {other:?}"),
    }
}

#[test]
fn test_module_constant_lowers_to_zero_arg_call() {
    let source = r#"
import numpy as np
p = np.pi
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();

    match &ir[0] {
        ast::IRStmt::Assign { value, .. } => match value {
            ast::IRExpr::ModuleCall { module, func, args } => {
                assert_eq!(module, "numpy");
                assert_eq!(func, "pi");
                assert!(args.is_empty());
            }
            other => panic!("Expected ModuleCall, got {other:?}"),
        },
        other => panic!("Expected Assign, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Codegen snapshots
// ---------------------------------------------------------------------------

#[test]
fn test_array_creation_and_indexing() {
    let source = r#"
import numpy as np
a = np.array([1.0, 2.0, 3.0, 4.0])
print(a[0])
print(a[3])
"#;
    insta::assert_snapshot!(compile(source));
}

#[test]
fn test_array_elementwise_add() {
    let source = r#"
import numpy as np
a = np.array([1.0, 2.0, 3.0, 4.0])
b = np.array([10.0, 20.0, 30.0, 40.0])
c = a + b
print(c[0])
"#;
    insta::assert_snapshot!(compile(source));
}

#[test]
fn test_array_scalar_broadcast() {
    let source = r#"
import numpy as np
a = np.arange(5)
b = a * 2
c = 10 + a
print(b[4])
print(c[0])
"#;
    insta::assert_snapshot!(compile(source));
}

#[test]
fn test_array_reductions() {
    let source = r#"
import numpy as np
a = np.array([1.0, 2.0, 3.0, 4.0])
print(a.sum())
print(a.mean())
print(len(a))
print(a.size)
"#;
    insta::assert_snapshot!(compile(source));
}

#[test]
fn test_array_constructors() {
    let source = r#"
import numpy as np
z = np.zeros(3)
o = np.ones(4)
r = np.arange(6)
print(z.sum())
print(o.sum())
print(r.sum())
"#;
    insta::assert_snapshot!(compile(source));
}

// ---------------------------------------------------------------------------
// Behavioural invariants
// ---------------------------------------------------------------------------

#[test]
fn test_elementwise_loop_is_vectorized() {
    // The point of unboxed arrays is that the element-wise loop auto-vectorizes
    // under the O2 pipeline. Adding two arrays should yield SIMD f64 ops.
    let source = r#"
import numpy as np
a = np.array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0])
b = np.array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0])
c = a + b
print(c[0])
"#;
    let ir = compile(source);
    assert!(
        ir.contains("x double>"),
        "expected vectorized (<N x double>) element-wise loop, got:\n{ir}"
    );
}

#[test]
fn test_scalar_program_has_no_array_machinery() {
    // Pay-as-you-go: a program that never touches NumPy must not emit any of the
    // array-dispatch machinery. This guards the property that keeps every
    // existing scalar snapshot byte-for-byte unchanged.
    let source = r#"
def add(a, b):
    return a + b

print(add(2, 3))
xs = [1, 2, 3]
print(xs[1])
print(len(xs))
"#;
    let ir = compile(source);
    assert!(!ir.contains("is_array"), "scalar code emitted is_array");
    assert!(
        !ir.contains("arr_loop"),
        "scalar code emitted an array loop"
    );
    assert!(
        !ir.contains("malloc_arr"),
        "scalar code emitted an array allocation"
    );
}
