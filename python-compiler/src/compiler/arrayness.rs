//! Whole-program "arrayness" analysis.
//!
//! Rusthon represents every value as one NaN-boxed `i64`, so an ndarray passed
//! to — or returned from — a function is already carried correctly at runtime.
//! The only thing missing across a function boundary is the *compile-time*
//! knowledge that a value might be an array, which is what gates the array code
//! paths (see [`Compiler::expr_may_be_array`]). Within a single function codegen
//! tracks this flow-sensitively; this module propagates the same fact *between*
//! functions so arrays can flow through parameters and return values.
//!
//! It computes two facts with a monotonic fixpoint:
//! - [`ArraynessInfo::array_returning`]: functions that may return an array.
//! - [`ArraynessInfo::array_params`]: for each function, which parameters may
//!   receive an array.
//!
//! The analysis is a sound over-approximation: it must flag *every* value that
//! could be an array (a miss would miscompile array code), while a spurious flag
//! only costs an unnecessary — but still correct — runtime array dispatch.
//!
//! [`Compiler::expr_may_be_array`]: crate::codegen::Compiler::expr_may_be_array

use crate::ast::{BinOp, IRExpr, IRStmt};
use std::collections::{HashMap, HashSet};

/// NumPy constructor functions that build a fresh array.
const NUMPY_CONSTRUCTORS: &[&str] = &["array", "zeros", "ones", "arange"];

/// Element-wise unary math functions (ufuncs). Each maps an array to a new
/// array, so — like constructors — a call to one yields an array. The codegen
/// mapping to LLVM intrinsics lives in `generators::ndarray::ufunc_intrinsic`;
/// the two lists are kept in sync by `test_ufunc_tables_agree`.
pub const NUMPY_UFUNCS: &[&str] = &["sqrt", "abs", "exp", "log", "sin", "cos", "floor", "ceil"];

/// Whether `func` is a NumPy array constructor (`array`/`zeros`/`ones`/`arange`).
pub fn is_numpy_constructor(func: &str) -> bool {
    NUMPY_CONSTRUCTORS.contains(&func)
}

/// Whether `func` is an element-wise unary ufunc (`sqrt`/`exp`/...).
pub fn is_numpy_ufunc(func: &str) -> bool {
    NUMPY_UFUNCS.contains(&func)
}

/// Whether a `numpy.<func>(args)` call evaluates to an array, given a predicate
/// that reports whether each argument may be an array.
///
/// This is the single source of truth shared by the interprocedural analysis
/// ([`expr_is_array`]) and codegen ([`Compiler::expr_may_be_array`]):
/// - constructors always yield an array;
/// - a ufunc mirrors its argument (`np.sqrt(arr)` is an array, `np.sqrt(x)` a
///   scalar), exactly like NumPy;
/// - reductions (`sum`/`mean`/`max`/`min`/`prod`/`dot`) and constants
///   (`pi`/`e`) yield scalars.
///
/// [`Compiler::expr_may_be_array`]: crate::codegen::Compiler::expr_may_be_array
pub fn numpy_call_returns_array(
    func: &str,
    args: &[IRExpr],
    arg_is_array: impl Fn(&IRExpr) -> bool,
) -> bool {
    if is_numpy_constructor(func) {
        return true;
    }
    if is_numpy_ufunc(func) {
        return args.first().is_some_and(arg_is_array);
    }
    false
}

/// Interprocedural arrayness facts for a program.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ArraynessInfo {
    /// Names of functions that may return an array.
    pub array_returning: HashSet<String>,
    /// For each function name, one flag per parameter: may it receive an array?
    pub array_params: HashMap<String, Vec<bool>>,
}

impl ArraynessInfo {
    /// Whether a call to `func` might yield an array.
    pub fn call_returns_array(&self, func: &str) -> bool {
        self.array_returning.contains(func)
    }

    /// Whether parameter `index` of `func` might receive an array.
    pub fn param_is_array(&self, func: &str, index: usize) -> bool {
        self.array_params
            .get(func)
            .and_then(|flags| flags.get(index))
            .copied()
            .unwrap_or(false)
    }
}

/// A top-level function definition, borrowed for the duration of the analysis.
struct FnDef<'a> {
    name: &'a str,
    params: &'a [String],
    defaults: &'a [Option<IRExpr>],
    body: &'a [IRStmt],
}

/// Computes the arrayness facts for a whole program.
pub fn analyze(program: &[IRStmt]) -> ArraynessInfo {
    let functions: Vec<FnDef> = program
        .iter()
        .filter_map(|stmt| match stmt {
            IRStmt::FunctionDef {
                name,
                params,
                defaults,
                body,
            } => Some(FnDef {
                name,
                params,
                defaults,
                body,
            }),
            _ => None,
        })
        .collect();

    let mut info = ArraynessInfo::default();
    for f in &functions {
        info.array_params
            .insert(f.name.to_string(), vec![false; f.params.len()]);
    }

    // Monotonic fixpoint: each pass reads the previous iteration's facts
    // (`prev`) and accumulates new ones into `info` until nothing changes.
    loop {
        let prev = info.clone();

        for f in &functions {
            // A default value can itself make a parameter array-typed.
            for (i, default) in f.defaults.iter().enumerate() {
                if let Some(default) = default {
                    if expr_is_array(default, &HashSet::new(), &prev) {
                        mark_param(&mut info, f.name, i);
                    }
                }
            }

            // Walk the body with parameters seeded from what callers pass in.
            let mut ctx: HashSet<String> = HashSet::new();
            for (i, param) in f.params.iter().enumerate() {
                if prev.param_is_array(f.name, i) {
                    ctx.insert(param.clone());
                }
            }
            analyze_block(f.body, &mut ctx, Some(f.name), &prev, &mut info);
        }

        // Top-level statements: their calls also constrain callee parameters.
        // Nested `FunctionDef`s are skipped here (analyzed above).
        let mut ctx = HashSet::new();
        analyze_block(program, &mut ctx, None, &prev, &mut info);

        if info == prev {
            break;
        }
    }

    info
}

/// Marks parameter `index` of `func` as possibly array-typed.
fn mark_param(info: &mut ArraynessInfo, func: &str, index: usize) {
    if let Some(flags) = info.array_params.get_mut(func) {
        if let Some(slot) = flags.get_mut(index) {
            *slot = true;
        }
    }
}

/// Walks a block, growing `ctx` (locals that may be arrays) and recording facts.
fn analyze_block(
    stmts: &[IRStmt],
    ctx: &mut HashSet<String>,
    current_fn: Option<&str>,
    prev: &ArraynessInfo,
    info: &mut ArraynessInfo,
) {
    for stmt in stmts {
        match stmt {
            IRStmt::Assign { target, value } => {
                analyze_expr(value, ctx, prev, info);
                // Grow-only within a function: once a name may be an array we
                // keep treating it as such. This is a sound over-approximation
                // that makes control flow trivial to handle.
                if expr_is_array(value, ctx, prev) {
                    ctx.insert(target.clone());
                }
            }
            IRStmt::IndexAssign {
                target,
                index,
                value,
            } => {
                analyze_expr(target, ctx, prev, info);
                analyze_expr(index, ctx, prev, info);
                analyze_expr(value, ctx, prev, info);
            }
            IRStmt::Print(exprs) => {
                for e in exprs {
                    analyze_expr(e, ctx, prev, info);
                }
            }
            IRStmt::ExprStmt(e) => analyze_expr(e, ctx, prev, info),
            IRStmt::Return(value) => {
                analyze_expr(value, ctx, prev, info);
                if let Some(f) = current_fn {
                    if expr_is_array(value, ctx, prev) {
                        info.array_returning.insert(f.to_string());
                    }
                }
            }
            IRStmt::If {
                condition,
                then_body,
                else_body,
            } => {
                analyze_expr(condition, ctx, prev, info);
                analyze_block(then_body, ctx, current_fn, prev, info);
                analyze_block(else_body, ctx, current_fn, prev, info);
            }
            IRStmt::While { condition, body } => {
                analyze_expr(condition, ctx, prev, info);
                analyze_block(body, ctx, current_fn, prev, info);
            }
            IRStmt::For {
                start, end, body, ..
            } => {
                analyze_expr(start, ctx, prev, info);
                analyze_expr(end, ctx, prev, info);
                // The loop variable is a range integer, never an array.
                analyze_block(body, ctx, current_fn, prev, info);
            }
            IRStmt::FunctionDef { .. } | IRStmt::Break | IRStmt::Continue => {}
        }
    }
}

/// Recurses through an expression, recording which call arguments may be arrays.
fn analyze_expr(
    expr: &IRExpr,
    ctx: &HashSet<String>,
    prev: &ArraynessInfo,
    info: &mut ArraynessInfo,
) {
    match expr {
        IRExpr::Call { func, args } => {
            for (i, arg) in args.iter().enumerate() {
                analyze_expr(arg, ctx, prev, info);
                if expr_is_array(arg, ctx, prev) {
                    mark_param(info, func, i);
                }
            }
        }
        IRExpr::ModuleCall { args, .. } => {
            for arg in args {
                analyze_expr(arg, ctx, prev, info);
            }
        }
        IRExpr::MethodCall { receiver, args, .. } => {
            analyze_expr(receiver, ctx, prev, info);
            for arg in args {
                analyze_expr(arg, ctx, prev, info);
            }
        }
        IRExpr::BinaryOp { left, right, .. } | IRExpr::Comparison { left, right, .. } => {
            analyze_expr(left, ctx, prev, info);
            analyze_expr(right, ctx, prev, info);
        }
        IRExpr::UnaryOp { operand, .. } => analyze_expr(operand, ctx, prev, info),
        IRExpr::Index { list, index } => {
            analyze_expr(list, ctx, prev, info);
            analyze_expr(index, ctx, prev, info);
        }
        IRExpr::Slice {
            value,
            lower,
            upper,
        } => {
            analyze_expr(value, ctx, prev, info);
            if let Some(l) = lower {
                analyze_expr(l, ctx, prev, info);
            }
            if let Some(u) = upper {
                analyze_expr(u, ctx, prev, info);
            }
        }
        IRExpr::Attribute { value, .. } => analyze_expr(value, ctx, prev, info),
        IRExpr::Len(e) => analyze_expr(e, ctx, prev, info),
        IRExpr::List(elts) => {
            for e in elts {
                analyze_expr(e, ctx, prev, info);
            }
        }
        IRExpr::Constant(_)
        | IRExpr::Float(_)
        | IRExpr::Bool(_)
        | IRExpr::Variable(_)
        | IRExpr::StringLiteral(_)
        | IRExpr::Input => {}
    }
}

/// Whether `expr` may evaluate to an array, given the local array set `ctx` and
/// the interprocedural facts collected so far. Mirrors
/// [`Compiler::expr_may_be_array`], extended with the cross-function facts.
///
/// [`Compiler::expr_may_be_array`]: crate::codegen::Compiler::expr_may_be_array
fn expr_is_array(expr: &IRExpr, ctx: &HashSet<String>, info: &ArraynessInfo) -> bool {
    match expr {
        IRExpr::ModuleCall { module, func, args } => {
            module == "numpy"
                && numpy_call_returns_array(func, args, |a| expr_is_array(a, ctx, info))
        }
        IRExpr::Slice { .. } => true,
        IRExpr::BinaryOp { op, left, right } => {
            matches!(
                op,
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod
            ) && (expr_is_array(left, ctx, info) || expr_is_array(right, ctx, info))
        }
        IRExpr::Variable(name) => ctx.contains(name),
        IRExpr::Call { func, .. } => info.call_returns_array(func),
        _ => false,
    }
}
