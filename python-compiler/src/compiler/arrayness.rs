//! Whole-program "arrayness" analysis.
//!
//! Rusthon represents every value as one NaN-boxed `i64`, so an ndarray passed
//! to — or returned from — a function is already carried correctly at runtime.
//! The only thing missing across a function boundary is the *compile-time*
//! knowledge that a value is an array **and of which dtype**, which gates the
//! array code paths (see [`Compiler::expr_may_be_array`]) and picks int-vs-float
//! storage. Within a single function codegen tracks this flow-sensitively; this
//! module propagates the same facts *between* functions so arrays flow through
//! parameters and return values.
//!
//! Because an `int64` array stores raw `i64` bytes and a `float64` array raw
//! `f64` bytes, the dtype **must** be known statically (reading one as the other
//! reinterprets bytes). Where the dtype cannot be pinned down (e.g. a function
//! returns an int array on one path and a float array on another) it becomes
//! [`ArrayDtype::Unknown`], and codegen reports an error rather than guess.
//!
//! [`Compiler::expr_may_be_array`]: crate::codegen::Compiler::expr_may_be_array

use crate::ast::{BinOp, IRExpr, IRStmt};
use std::collections::HashMap;

/// NumPy constructor functions that build a fresh array.
const NUMPY_CONSTRUCTORS: &[&str] = &["array", "zeros", "ones", "arange"];

/// Element-wise unary math functions (ufuncs). Each maps an array to a new
/// (always `float64`) array. The codegen mapping to LLVM intrinsics lives in
/// `generators::ndarray::ufunc_intrinsic`; the two lists are kept in sync by
/// `test_ufunc_tables_agree`.
pub const NUMPY_UFUNCS: &[&str] = &["sqrt", "abs", "exp", "log", "sin", "cos", "floor", "ceil"];

/// The element type of an array (or the type produced by a numeric operation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayDtype {
    /// 64-bit signed integer elements.
    Int,
    /// 64-bit IEEE-754 float elements.
    Float,
    /// Statically indeterminate (e.g. conflicting dtypes merged across paths).
    Unknown,
}

impl ArrayDtype {
    /// NumPy-style promotion for a binary op: `int op int` stays `int`, anything
    /// touching `float` becomes `float`, and `Unknown` is contagious.
    pub fn promote(self, other: ArrayDtype) -> ArrayDtype {
        use ArrayDtype::*;
        match (self, other) {
            (Unknown, _) | (_, Unknown) => Unknown,
            (Int, Int) => Int,
            _ => Float,
        }
    }

    /// Merge of the *same* value's dtype seen along different paths/call sites:
    /// equal dtypes stay, disagreement collapses to `Unknown`.
    pub fn merge(self, other: ArrayDtype) -> ArrayDtype {
        if self == other {
            self
        } else {
            ArrayDtype::Unknown
        }
    }
}

/// Whether `func` is a NumPy array constructor (`array`/`zeros`/`ones`/`arange`).
pub fn is_numpy_constructor(func: &str) -> bool {
    NUMPY_CONSTRUCTORS.contains(&func)
}

/// Whether `func` is an element-wise unary ufunc (`sqrt`/`exp`/...).
pub fn is_numpy_ufunc(func: &str) -> bool {
    NUMPY_UFUNCS.contains(&func)
}

/// The dtype of a `numpy.<func>(args)` call's result, or `None` if it is not an
/// array (reductions, `dot`, constants).
///
/// - `array([literals])`: `Int` if every element is an integer literal, else
///   `Float`; a non-literal argument defaults to `Float`.
/// - `zeros`/`ones`: `Float`; `arange`: `Int` (matching NumPy).
/// - ufuncs mirror the *array-ness* of their argument and always yield `Float`.
///
/// Single source of truth shared by the analysis and codegen.
pub fn numpy_call_dtype(
    func: &str,
    args: &[IRExpr],
    arg_dtype: impl Fn(&IRExpr) -> Option<ArrayDtype>,
) -> Option<ArrayDtype> {
    if is_numpy_constructor(func) {
        return Some(match func {
            "arange" => ArrayDtype::Int,
            "array" => array_literal_dtype(args.first()),
            _ => ArrayDtype::Float, // zeros, ones
        });
    }
    if is_numpy_ufunc(func) {
        return args.first().and_then(&arg_dtype).map(|_| ArrayDtype::Float);
    }
    // `np.matmul(A, B)` returns a 2-D array; its dtype promotes the operands.
    if func == "matmul" {
        let a = args.first().and_then(&arg_dtype);
        let b = args.get(1).and_then(&arg_dtype);
        return Some(match (a, b) {
            (Some(x), Some(y)) => x.promote(y),
            _ => ArrayDtype::Float,
        });
    }
    None
}

/// Infers the dtype of `np.array(arg)` from its literal argument: `Int` if every
/// (possibly nested) leaf is an integer literal, else `Float`.
fn array_literal_dtype(arg: Option<&IRExpr>) -> ArrayDtype {
    match arg {
        Some(IRExpr::List(elts)) if elts.iter().all(all_int_leaves) => ArrayDtype::Int,
        _ => ArrayDtype::Float,
    }
}

/// Whether every leaf of a (possibly nested) list literal is an integer literal.
fn all_int_leaves(expr: &IRExpr) -> bool {
    match expr {
        IRExpr::List(elts) => elts.iter().all(all_int_leaves),
        other => matches!(other, IRExpr::Constant(_) | IRExpr::Bool(_)),
    }
}

/// The dtype a *scalar* operand contributes to promotion. Integer literals count
/// as `Int`; anything whose scalar type we don't track is treated as `Float`
/// (safe: it only ever promotes a result towards float, never mis-reads storage).
fn scalar_operand_dtype(expr: &IRExpr) -> ArrayDtype {
    match expr {
        IRExpr::Constant(_) | IRExpr::Bool(_) | IRExpr::Len(_) => ArrayDtype::Int,
        _ => ArrayDtype::Float,
    }
}

/// Interprocedural arrayness facts for a program.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ArraynessInfo {
    /// For each array-returning function, the dtype of the returned array.
    pub array_returning: HashMap<String, ArrayDtype>,
    /// For each function name, one entry per parameter: `Some(dtype)` if the
    /// parameter may receive an array of that dtype, `None` if it is scalar.
    pub array_params: HashMap<String, Vec<Option<ArrayDtype>>>,
}

impl ArraynessInfo {
    /// The dtype of the array `func` returns, if it returns one.
    pub fn return_dtype(&self, func: &str) -> Option<ArrayDtype> {
        self.array_returning.get(func).copied()
    }

    /// The dtype of the array parameter `index` of `func`, if it is an array.
    pub fn param_dtype(&self, func: &str, index: usize) -> Option<ArrayDtype> {
        self.array_params
            .get(func)
            .and_then(|flags| flags.get(index))
            .copied()
            .flatten()
    }
}

/// A top-level function definition, borrowed for the duration of the analysis.
struct FnDef<'a> {
    name: &'a str,
    params: &'a [String],
    defaults: &'a [Option<IRExpr>],
    body: &'a [IRStmt],
}

/// A map from local variable name to the dtype of the array it may hold.
type Ctx = HashMap<String, ArrayDtype>;

/// Computes the arrayness (and dtype) facts for a whole program.
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
            .insert(f.name.to_string(), vec![None; f.params.len()]);
    }

    // Monotonic fixpoint: each pass reads the previous iteration's facts
    // (`prev`) and accumulates new ones into `info` until nothing changes.
    loop {
        let prev = info.clone();

        for f in &functions {
            // A default value can itself make a parameter array-typed.
            for (i, default) in f.defaults.iter().enumerate() {
                if let Some(default) = default {
                    if let Some(dt) = expr_array_dtype(default, &Ctx::new(), &prev) {
                        mark_param(&mut info, f.name, i, dt);
                    }
                }
            }

            // Walk the body with parameters seeded from what callers pass in.
            let mut ctx = Ctx::new();
            for (i, param) in f.params.iter().enumerate() {
                if let Some(dt) = prev.param_dtype(f.name, i) {
                    ctx.insert(param.clone(), dt);
                }
            }
            analyze_block(f.body, &mut ctx, Some(f.name), &prev, &mut info);
        }

        // Top-level statements: their calls also constrain callee parameters.
        // Nested `FunctionDef`s are skipped here (analyzed above).
        let mut ctx = Ctx::new();
        analyze_block(program, &mut ctx, None, &prev, &mut info);

        if info == prev {
            break;
        }
    }

    info
}

/// Records that parameter `index` of `func` may receive an array of `dtype`,
/// merging with any dtype seen at other call sites.
fn mark_param(info: &mut ArraynessInfo, func: &str, index: usize, dtype: ArrayDtype) {
    if let Some(flags) = info.array_params.get_mut(func) {
        if let Some(slot) = flags.get_mut(index) {
            *slot = Some(match *slot {
                Some(existing) => existing.merge(dtype),
                None => dtype,
            });
        }
    }
}

/// Records that `func` may return an array of `dtype`, merging across returns.
fn mark_return(info: &mut ArraynessInfo, func: &str, dtype: ArrayDtype) {
    let entry = info
        .array_returning
        .entry(func.to_string())
        .or_insert(dtype);
    *entry = entry.merge(dtype);
}

/// Walks a block, growing `ctx` (locals that may be arrays) and recording facts.
fn analyze_block(
    stmts: &[IRStmt],
    ctx: &mut Ctx,
    current_fn: Option<&str>,
    prev: &ArraynessInfo,
    info: &mut ArraynessInfo,
) {
    for stmt in stmts {
        match stmt {
            IRStmt::Assign { target, value } => {
                analyze_expr(value, ctx, prev, info);
                // Grow-only within a function: once a name may be an array we
                // keep treating it as such, merging dtypes seen along the way.
                if let Some(dt) = expr_array_dtype(value, ctx, prev) {
                    let merged = ctx.get(target).map_or(dt, |old| old.merge(dt));
                    ctx.insert(target.clone(), merged);
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
            IRStmt::Unpack { value, .. } => analyze_expr(value, ctx, prev, info),
            IRStmt::Print(exprs) => {
                for e in exprs {
                    analyze_expr(e, ctx, prev, info);
                }
            }
            IRStmt::ExprStmt(e) => analyze_expr(e, ctx, prev, info),
            IRStmt::Return(value) => {
                analyze_expr(value, ctx, prev, info);
                if let Some(f) = current_fn {
                    if let Some(dt) = expr_array_dtype(value, ctx, prev) {
                        mark_return(info, f, dt);
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

/// Recurses through an expression, recording the dtypes of array call arguments.
fn analyze_expr(expr: &IRExpr, ctx: &Ctx, prev: &ArraynessInfo, info: &mut ArraynessInfo) {
    match expr {
        IRExpr::Call { func, args } => {
            for (i, arg) in args.iter().enumerate() {
                analyze_expr(arg, ctx, prev, info);
                if let Some(dt) = expr_array_dtype(arg, ctx, prev) {
                    mark_param(info, func, i, dt);
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
        IRExpr::IndexND { array, indices } => {
            analyze_expr(array, ctx, prev, info);
            for idx in indices {
                analyze_expr(idx, ctx, prev, info);
            }
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
        IRExpr::List(elts) | IRExpr::Tuple(elts) => {
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

/// The dtype of the array `expr` evaluates to, or `None` if it is not an array.
/// Mirrors [`Compiler::expr_array_dtype`], extended with cross-function facts.
///
/// [`Compiler::expr_array_dtype`]: crate::codegen::Compiler::expr_array_dtype
fn expr_array_dtype(expr: &IRExpr, ctx: &Ctx, info: &ArraynessInfo) -> Option<ArrayDtype> {
    match expr {
        IRExpr::ModuleCall { module, func, args } if module == "numpy" => {
            numpy_call_dtype(func, args, |a| expr_array_dtype(a, ctx, info))
        }
        IRExpr::Slice { value, .. } => expr_array_dtype(value, ctx, info),
        // `a.T` (transpose) is an array of the same dtype as `a`.
        IRExpr::Attribute { value, attr } if attr == "T" => expr_array_dtype(value, ctx, info),
        IRExpr::BinaryOp { op, left, right }
            if matches!(
                op,
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod
            ) =>
        {
            binop_result_dtype(op, left, right, |e| expr_array_dtype(e, ctx, info))
        }
        IRExpr::Variable(name) => ctx.get(name).copied(),
        IRExpr::Call { func, .. } => info.return_dtype(func),
        _ => None,
    }
}

/// The dtype of an element-wise `left op right` where at least one operand may
/// be an array, or `None` if neither operand is an array.
pub fn binop_result_dtype(
    op: &BinOp,
    left: &IRExpr,
    right: &IRExpr,
    arg_dtype: impl Fn(&IRExpr) -> Option<ArrayDtype>,
) -> Option<ArrayDtype> {
    let ld = arg_dtype(left);
    let rd = arg_dtype(right);
    if ld.is_none() && rd.is_none() {
        return None; // scalar op scalar — not an array
    }
    // True division always yields float, even for int operands (NumPy `/`).
    if matches!(op, BinOp::Div) {
        return Some(
            if ld == Some(ArrayDtype::Unknown) || rd == Some(ArrayDtype::Unknown) {
                ArrayDtype::Unknown
            } else {
                ArrayDtype::Float
            },
        );
    }
    let left_dt = ld.unwrap_or_else(|| scalar_operand_dtype(left));
    let right_dt = rd.unwrap_or_else(|| scalar_operand_dtype(right));
    Some(left_dt.promote(right_dt))
}
