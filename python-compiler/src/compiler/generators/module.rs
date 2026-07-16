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
    let recv = compiler.compile_expression(receiver)?;
    match method {
        "sum" => {
            expect_argc(method, args, 0)?;
            ndarray::reduce_sum(compiler, recv)
        }
        "mean" => {
            expect_argc(method, args, 0)?;
            ndarray::mean(compiler, recv)
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
    let obj = compiler.compile_expression(value)?;
    match attr {
        "size" => Ok(ndarray::size(compiler, obj)),
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
    match func {
        // Constructors.
        "array" => {
            expect_argc(func, args, 1)?;
            let list = compiler.compile_expression(&args[0])?;
            ndarray::from_list(compiler, list)
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
            let arr = compiler.compile_expression(&args[0])?;
            ndarray::reduce_sum(compiler, arr)
        }
        "mean" => {
            expect_argc(func, args, 1)?;
            let arr = compiler.compile_expression(&args[0])?;
            ndarray::mean(compiler, arr)
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
