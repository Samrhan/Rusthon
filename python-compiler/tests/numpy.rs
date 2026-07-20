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

#[test]
fn test_item_assignment_lowers_to_index_assign() {
    let source = r#"
import numpy as np
a = np.array([1.0, 2.0, 3.0])
a[0] = 9.0
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();

    match &ir[1] {
        ast::IRStmt::IndexAssign {
            target,
            index,
            value,
        } => {
            assert_eq!(*target, ast::IRExpr::Variable("a".to_string()));
            assert_eq!(*index, ast::IRExpr::Constant(0));
            assert_eq!(*value, ast::IRExpr::Float(9.0));
        }
        other => panic!("Expected IndexAssign, got {other:?}"),
    }
}

#[test]
fn test_slice_lowers_to_slice() {
    let source = r#"
import numpy as np
a = np.array([1.0, 2.0, 3.0, 4.0])
s = a[1:3]
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();

    match &ir[1] {
        ast::IRStmt::Assign { target, value } => {
            assert_eq!(target, "s");
            match value {
                ast::IRExpr::Slice {
                    value,
                    lower,
                    upper,
                } => {
                    assert_eq!(**value, ast::IRExpr::Variable("a".to_string()));
                    assert_eq!(lower.as_deref(), Some(&ast::IRExpr::Constant(1)));
                    assert_eq!(upper.as_deref(), Some(&ast::IRExpr::Constant(3)));
                }
                other => panic!("Expected Slice, got {other:?}"),
            }
        }
        other => panic!("Expected Assign, got {other:?}"),
    }
}

#[test]
fn test_open_slice_has_no_bounds() {
    let source = r#"
import numpy as np
a = np.arange(5)
s = a[:]
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();

    match &ir[1] {
        ast::IRStmt::Assign { value, .. } => match value {
            ast::IRExpr::Slice { lower, upper, .. } => {
                assert!(lower.is_none() && upper.is_none());
            }
            other => panic!("Expected Slice, got {other:?}"),
        },
        other => panic!("Expected Assign, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Interprocedural arrayness analysis
// ---------------------------------------------------------------------------

#[test]
fn test_arrayness_analysis_flows_through_functions() {
    let source = r#"
import numpy as np
def scale(v, k):
    return v * k
def total(v):
    return v.sum()
def add(a, b):
    return a + b
arr = np.array([1.0, 2.0, 3.0])
b = scale(arr, 2.0)
print(total(b))
print(add(2, 3))
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let info = compiler::arrayness::analyze(&ir);

    // `scale(v, k)` returns `v * k` with an array `v` → returns an array.
    assert!(info.call_returns_array("scale"), "scale returns an array");
    assert!(info.param_is_array("scale", 0), "scale.v may be an array");
    assert!(!info.param_is_array("scale", 1), "scale.k is scalar");

    // `total(v)` returns `v.sum()` → a scalar, but takes an array parameter.
    assert!(!info.call_returns_array("total"), "total returns a scalar");
    assert!(info.param_is_array("total", 0), "total.v may be an array");

    // `add` is only ever called with scalars → untouched.
    assert!(!info.call_returns_array("add"), "add returns a scalar");
    assert!(!info.param_is_array("add", 0), "add.a is scalar");
    assert!(!info.param_is_array("add", 1), "add.b is scalar");
}

#[test]
fn test_arrayness_analysis_transitive_and_recursive() {
    let source = r#"
import numpy as np
def make(n):
    return np.arange(n)
def double(v):
    return v * 2
def make_doubled(n):
    return double(make(n))
def grow(v, k):
    if k <= 0:
        return v
    return grow(v + 1, k - 1)
d = make_doubled(4)
r = grow(np.zeros(3), 2)
print(d.sum())
print(r.sum())
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let info = compiler::arrayness::analyze(&ir);

    // Arrayness propagates transitively: make -> double -> make_doubled.
    assert!(info.call_returns_array("make"));
    assert!(info.call_returns_array("double"));
    assert!(info.call_returns_array("make_doubled"));
    assert!(info.param_is_array("double", 0));
    // And through recursion: grow takes and returns an array.
    assert!(info.param_is_array("grow", 0));
    assert!(info.call_returns_array("grow"));
}

// ---------------------------------------------------------------------------
// Codegen snapshots
// ---------------------------------------------------------------------------

#[test]
fn test_array_flows_through_functions() {
    let source = r#"
import numpy as np
def scale(v, k):
    return v * k
def total(v):
    return v.sum()
a = np.array([1.0, 2.0, 3.0])
b = scale(a, 2.0)
print(b[0])
print(total(b))
"#;
    insta::assert_snapshot!(compile(source));
}

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

#[test]
fn test_array_printing() {
    let source = r#"
import numpy as np
a = np.array([1.0, 2.0, 3.0])
print(a)
"#;
    insta::assert_snapshot!(compile(source));
}

#[test]
fn test_array_item_assignment() {
    let source = r#"
import numpy as np
a = np.array([1.0, 2.0, 3.0])
a[1] = 99.0
print(a[1])
"#;
    insta::assert_snapshot!(compile(source));
}

#[test]
fn test_array_slicing() {
    let source = r#"
import numpy as np
a = np.arange(6)
s = a[1:4]
print(s.sum())
print(a[:2].sum())
print(a[3:].sum())
"#;
    insta::assert_snapshot!(compile(source));
}

#[test]
fn test_array_min_max() {
    let source = r#"
import numpy as np
a = np.array([3.0, 1.0, 4.0, 1.0, 5.0])
print(a.max())
print(a.min())
print(np.max(a))
print(np.min(a))
"#;
    insta::assert_snapshot!(compile(source));
}

// ---------------------------------------------------------------------------
// Behavioural invariants
// ---------------------------------------------------------------------------

#[test]
fn test_slicing_non_array_is_rejected() {
    // Slicing is array-only; a plain list slice must be a compile-time error
    // rather than silently miscompiling.
    let source = r#"
xs = [1, 2, 3]
s = xs[0:2]
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    assert!(
        compiler.compile_program(&ir).is_err(),
        "slicing a list should be rejected"
    );
}

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
