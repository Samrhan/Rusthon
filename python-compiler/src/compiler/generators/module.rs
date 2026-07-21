//! Module, method and attribute dispatch.
//!
//! This is the generic bridge between the module-aware IR produced by lowering
//! ([`IRExpr::ModuleCall`], [`IRExpr::MethodCall`], [`IRExpr::Attribute`]) and
//! the concrete code generators. Lowering knows nothing about what any module
//! *does*; it only records that the user imported it. This dispatcher owns the
//! set of modules the compiler can actually generate code for.
//!
//! Adding a new module means adding a match arm here (and a generator module
//! like [`ndarray`]) — nothing in the parser or lowering changes.

use crate::ast::IRExpr;
use crate::codegen::{CodeGenError, Compiler};
use crate::compiler::arrayness::{self, ArrayDtype};
use crate::compiler::generators::ndarray;
use inkwell::values::IntValue;

/// Compiles a call to a function exposed by an imported module, e.g.
/// `np.array(...)` (lowered to `module = "numpy", func = "array"`).
pub fn compile_module_call<'ctx>(
    compiler: &mut Compiler<'ctx>,
    module: &str,
    func: &str,
    args: &[IRExpr],
) -> Result<IntValue<'ctx>, CodeGenError> {
    match module {
        "numpy" => compile_numpy_call(compiler, func, args),
        other => Err(CodeGenError::UnsupportedFeature(format!(
            "unknown module '{other}'"
        ))),
    }
}

/// Compiles a method call on a runtime value, e.g. `arr.sum()`.
///
/// Methods are dispatched purely by name here; the receiver's runtime type tag
/// selects the right behaviour inside the individual generators. For the
/// NumPy subset every supported method is an `ndarray` reduction.
pub fn compile_method_call<'ctx>(
    compiler: &mut Compiler<'ctx>,
    receiver: &IRExpr,
    method: &str,
    args: &[IRExpr],
) -> Result<IntValue<'ctx>, CodeGenError> {
    // All supported methods are reductions over an array of known dtype.
    let dtype = compiler.require_known_array_dtype(receiver)?;
    let recv = compiler.compile_expression(receiver)?;
    match method {
        "sum" => {
            expect_argc(method, args, 0)?;
            ndarray::reduce_sum(compiler, recv, dtype)
        }
        "mean" => {
            expect_argc(method, args, 0)?;
            ndarray::mean(compiler, recv, dtype)
        }
        "max" => {
            expect_argc(method, args, 0)?;
            ndarray::reduce_max(compiler, recv, dtype)
        }
        "min" => {
            expect_argc(method, args, 0)?;
            ndarray::reduce_min(compiler, recv, dtype)
        }
        "prod" => {
            expect_argc(method, args, 0)?;
            ndarray::reduce_prod(compiler, recv, dtype)
        }
        other => Err(CodeGenError::UnsupportedFeature(format!(
            "unknown method '.{other}()'"
        ))),
    }
}

/// Compiles attribute access on a runtime value, e.g. `arr.size`.
pub fn compile_attribute<'ctx>(
    compiler: &mut Compiler<'ctx>,
    value: &IRExpr,
    attr: &str,
) -> Result<IntValue<'ctx>, CodeGenError> {
    match attr {
        "size" => {
            let obj = compiler.compile_expression(value)?;
            Ok(ndarray::size(compiler, obj))
        }
        // `a.T` — transpose of a 2-D array.
        "T" => {
            let dtype = compiler.require_known_array_dtype(value)?;
            let obj = compiler.compile_expression(value)?;
            ndarray::transpose(compiler, obj, dtype)
        }
        other => Err(CodeGenError::UnsupportedFeature(format!(
            "unknown attribute '.{other}'"
        ))),
    }
}

/// Dispatches the `numpy` module's functions and constants.
fn compile_numpy_call<'ctx>(
    compiler: &mut Compiler<'ctx>,
    func: &str,
    args: &[IRExpr],
) -> Result<IntValue<'ctx>, CodeGenError> {
    // Element-wise unary ufuncs (np.sqrt, np.exp, ...). Like NumPy they apply
    // element-wise to an array and directly to a scalar; the array-vs-scalar
    // choice follows the argument's compile-time arrayness.
    if let Some(intrinsic) = ndarray::ufunc_intrinsic(func) {
        expect_argc(func, args, 1)?;
        return if compiler.expr_may_be_array(&args[0]) {
            let src_dtype = compiler.require_known_array_dtype(&args[0])?;
            let arg = compiler.compile_expression(&args[0])?;
            ndarray::unary_map(compiler, arg, src_dtype, intrinsic)
        } else {
            let arg = compiler.compile_expression(&args[0])?;
            ndarray::unary_scalar(compiler, arg, intrinsic)
        };
    }

    match func {
        // Constructors.
        "array" => {
            expect_argc(func, args, 1)?;
            let dtype =
                arrayness::numpy_call_dtype("array", args, |a| compiler.expr_array_dtype(a))
                    .unwrap_or(ArrayDtype::Float);
            // Nested lists `[[..], [..]]` build a 2-D array.
            if let IRExpr::List(rows) = &args[0] {
                if !rows.is_empty() && rows.iter().all(|r| matches!(r, IRExpr::List(_))) {
                    return ndarray::from_nested(compiler, rows, dtype);
                }
            }
            let list = compiler.compile_expression(&args[0])?;
            ndarray::from_list(compiler, list, dtype)
        }
        "zeros" => {
            expect_argc(func, args, 1)?;
            let len = compiler.compile_expression(&args[0])?;
            ndarray::zeros(compiler, len)
        }
        "ones" => {
            expect_argc(func, args, 1)?;
            let len = compiler.compile_expression(&args[0])?;
            ndarray::ones(compiler, len)
        }
        "arange" => {
            expect_argc(func, args, 1)?;
            let len = compiler.compile_expression(&args[0])?;
            ndarray::arange(compiler, len)
        }
        // Free-function forms of the reductions: np.sum(a) / np.mean(a).
        "sum" => {
            expect_argc(func, args, 1)?;
            let dtype = compiler.require_known_array_dtype(&args[0])?;
            let arr = compiler.compile_expression(&args[0])?;
            ndarray::reduce_sum(compiler, arr, dtype)
        }
        "mean" => {
            expect_argc(func, args, 1)?;
            let dtype = compiler.require_known_array_dtype(&args[0])?;
            let arr = compiler.compile_expression(&args[0])?;
            ndarray::mean(compiler, arr, dtype)
        }
        "max" => {
            expect_argc(func, args, 1)?;
            let dtype = compiler.require_known_array_dtype(&args[0])?;
            let arr = compiler.compile_expression(&args[0])?;
            ndarray::reduce_max(compiler, arr, dtype)
        }
        "min" => {
            expect_argc(func, args, 1)?;
            let dtype = compiler.require_known_array_dtype(&args[0])?;
            let arr = compiler.compile_expression(&args[0])?;
            ndarray::reduce_min(compiler, arr, dtype)
        }
        "prod" => {
            expect_argc(func, args, 1)?;
            let dtype = compiler.require_known_array_dtype(&args[0])?;
            let arr = compiler.compile_expression(&args[0])?;
            ndarray::reduce_prod(compiler, arr, dtype)
        }
        // Linear algebra: 1-D dot product (scalar).
        "dot" => {
            expect_argc(func, args, 2)?;
            let a_dtype = compiler.require_known_array_dtype(&args[0])?;
            let b_dtype = compiler.require_known_array_dtype(&args[1])?;
            let a = compiler.compile_expression(&args[0])?;
            let b = compiler.compile_expression(&args[1])?;
            ndarray::dot(compiler, a, b, a_dtype, b_dtype)
        }
        // 2-D matrix multiply (returns an array).
        "matmul" => {
            expect_argc(func, args, 2)?;
            let a_dtype = compiler.require_known_array_dtype(&args[0])?;
            let b_dtype = compiler.require_known_array_dtype(&args[1])?;
            let result = if a_dtype == ArrayDtype::Int && b_dtype == ArrayDtype::Int {
                ArrayDtype::Int
            } else {
                ArrayDtype::Float
            };
            let a = compiler.compile_expression(&args[0])?;
            let b = compiler.compile_expression(&args[1])?;
            ndarray::matmul(compiler, a, b, result)
        }
        // Constants (lowered to zero-argument module calls).
        "pi" => Ok(compiler.create_pyobject_float(
            compiler
                .context
                .f64_type()
                .const_float(std::f64::consts::PI),
        )),
        "e" => Ok(compiler
            .create_pyobject_float(compiler.context.f64_type().const_float(std::f64::consts::E))),
        other => Err(CodeGenError::UnsupportedFeature(format!(
            "unsupported numpy member 'numpy.{other}'"
        ))),
    }
}

/// Validates the argument count for a built-in, returning a clear error.
fn expect_argc(name: &str, args: &[IRExpr], expected: usize) -> Result<(), CodeGenError> {
    if args.len() != expected {
        return Err(CodeGenError::UnsupportedFeature(format!(
            "'{name}' expects {expected} argument(s), got {}",
            args.len()
        )));
    }
    Ok(())
}
