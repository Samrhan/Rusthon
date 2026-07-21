//! NumPy-style `ndarray` code generation.
//!
//! This module implements the compiled subset of NumPy. An `ndarray` is a heap
//! object with an **unboxed, typed, contiguous** data buffer — the opposite of
//! Rusthon lists, whose elements are individually NaN-boxed. Storing raw values
//! is what lets the element-wise loops below be auto-vectorised by LLVM's
//! `default<O2>` pipeline (loop + SLP vectorisation), which is the whole point
//! of having arrays.
//!
//! ## Memory layout
//!
//! ```text
//! base ─▶ [ dtype ][ ndim ][ size ][ x0 ][ x1 ] ... [ x(size-1) ]
//!          i64      i64      i64     └────── size elements ──────┘
//!         └──────── header (3 words) ─────┘
//! ```
//!
//! - `dtype`: element type tag (currently always [`DTYPE_F64`]).
//! - `ndim` : number of dimensions (currently always 1).
//! - `size` : total number of elements.
//! - data   : `size` contiguous elements. For `float64` each is an `f64`; the
//!   slot is 8 bytes so header words and data elements share one allocation and
//!   one addressing scheme (index `i` lives at word `HEADER_WORDS + i`).
//!
//! The `dtype`/`ndim` header fields are carried now (even though only
//! `float64`/1-D are generated) so multi-dtype and multi-dimensional arrays can
//! be added without changing the layout of existing code.

use crate::ast::BinOp;
use crate::codegen::{CodeGenError, Compiler};
use inkwell::intrinsics::Intrinsic;
use inkwell::values::{FloatValue, FunctionValue, IntValue, PointerValue};

/// Element dtype tag for `float64` arrays.
pub const DTYPE_F64: i64 = 0;

/// Number of `i64`-sized header words preceding the data buffer.
const HEADER_WORDS: u64 = 3;
/// Word offset of the `size` field within the header.
const SIZE_WORD: u64 = 2;

/// Allocates an uninitialised array with `len` elements of the given dtype and
/// writes its header. Returns the base pointer (data is left uninitialised).
fn alloc_array<'ctx>(
    compiler: &mut Compiler<'ctx>,
    len: IntValue<'ctx>,
    dtype: i64,
) -> Result<PointerValue<'ctx>, CodeGenError> {
    let i64_type = compiler.context.i64_type();

    // total_words = HEADER_WORDS + len ; total_bytes = total_words * 8
    let header = i64_type.const_int(HEADER_WORDS, false);
    let total_words = compiler
        .builder
        .build_int_add(header, len, "arr_words")
        .unwrap();
    let word_size = i64_type.const_int(8, false);
    let total_bytes = compiler
        .builder
        .build_int_mul(total_words, word_size, "arr_bytes")
        .unwrap();

    let malloc_fn = compiler.runtime.add_malloc(&compiler.module);
    let base = match compiler
        .builder
        .build_call(malloc_fn, &[total_bytes.into()], "malloc_arr")
        .unwrap()
        .try_as_basic_value()
    {
        inkwell::values::ValueKind::Basic(v) => v.into_pointer_value(),
        _ => {
            return Err(CodeGenError::ModuleVerification(
                "malloc did not return a value".to_string(),
            ))
        }
    };

    // Write header: [dtype][ndim=1][size=len]
    store_word(compiler, base, 0, i64_type.const_int(dtype as u64, true));
    store_word(compiler, base, 1, i64_type.const_int(1, false));
    store_word(compiler, base, SIZE_WORD, len);

    Ok(base)
}

/// Stores an `i64` at header `word` of the array.
fn store_word<'ctx>(
    compiler: &Compiler<'ctx>,
    base: PointerValue<'ctx>,
    word: u64,
    value: IntValue<'ctx>,
) {
    let i64_type = compiler.context.i64_type();
    let ptr = unsafe {
        compiler
            .builder
            .build_in_bounds_gep(
                i64_type,
                base,
                &[i64_type.const_int(word, false)],
                "hdr_ptr",
            )
            .unwrap()
    };
    compiler.builder.build_store(ptr, value).unwrap();
}

/// Returns the number of elements stored in the array (`size` header field).
pub fn array_len<'ctx>(compiler: &Compiler<'ctx>, base: PointerValue<'ctx>) -> IntValue<'ctx> {
    let i64_type = compiler.context.i64_type();
    let ptr = unsafe {
        compiler
            .builder
            .build_in_bounds_gep(
                i64_type,
                base,
                &[i64_type.const_int(SIZE_WORD, false)],
                "size_ptr",
            )
            .unwrap()
    };
    compiler
        .builder
        .build_load(i64_type, ptr, "arr_size")
        .unwrap()
        .into_int_value()
}

/// Returns a pointer to element 0 of the data buffer (skips the header).
fn data_ptr<'ctx>(compiler: &Compiler<'ctx>, base: PointerValue<'ctx>) -> PointerValue<'ctx> {
    let i64_type = compiler.context.i64_type();
    unsafe {
        compiler
            .builder
            .build_in_bounds_gep(
                i64_type,
                base,
                &[i64_type.const_int(HEADER_WORDS, false)],
                "data_ptr",
            )
            .unwrap()
    }
}

/// Address of `data[index]`, addressing the buffer as `f64` elements.
fn elem_ptr<'ctx>(
    compiler: &Compiler<'ctx>,
    data: PointerValue<'ctx>,
    index: IntValue<'ctx>,
) -> PointerValue<'ctx> {
    let f64_type = compiler.context.f64_type();
    unsafe {
        compiler
            .builder
            .build_in_bounds_gep(f64_type, data, &[index], "elem_ptr")
            .unwrap()
    }
}

/// Loads `data[index]` as an `f64`.
fn load_f64<'ctx>(
    compiler: &Compiler<'ctx>,
    data: PointerValue<'ctx>,
    index: IntValue<'ctx>,
) -> FloatValue<'ctx> {
    let ptr = elem_ptr(compiler, data, index);
    compiler
        .builder
        .build_load(compiler.context.f64_type(), ptr, "elem")
        .unwrap()
        .into_float_value()
}

/// Stores `value` into `data[index]`.
fn store_f64<'ctx>(
    compiler: &Compiler<'ctx>,
    data: PointerValue<'ctx>,
    index: IntValue<'ctx>,
    value: FloatValue<'ctx>,
) {
    let ptr = elem_ptr(compiler, data, index);
    compiler.builder.build_store(ptr, value).unwrap();
}

/// Emits a `for i in 0..count` counted loop, invoking `body` once to generate
/// the loop body with the current induction value. The body must not itself
/// terminate the current block (straight-line code and calls are fine).
///
/// The generated loop is the canonical shape LLVM recognises and vectorises.
fn emit_counted_loop<'ctx, F>(
    compiler: &mut Compiler<'ctx>,
    count: IntValue<'ctx>,
    mut body: F,
) -> Result<(), CodeGenError>
where
    F: FnMut(&mut Compiler<'ctx>, IntValue<'ctx>) -> Result<(), CodeGenError>,
{
    let i64_type = compiler.context.i64_type();
    let current_fn = compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();

    let i_ptr = compiler.builder.build_alloca(i64_type, "i").unwrap();
    compiler
        .builder
        .build_store(i_ptr, i64_type.const_int(0, false))
        .unwrap();

    let cond_bb = compiler
        .context
        .append_basic_block(current_fn, "arr_loop_cond");
    let body_bb = compiler
        .context
        .append_basic_block(current_fn, "arr_loop_body");
    let end_bb = compiler
        .context
        .append_basic_block(current_fn, "arr_loop_end");

    compiler
        .builder
        .build_unconditional_branch(cond_bb)
        .unwrap();

    // cond: i < count
    compiler.builder.position_at_end(cond_bb);
    let i_val = compiler
        .builder
        .build_load(i64_type, i_ptr, "i_val")
        .unwrap()
        .into_int_value();
    let cont = compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::ULT, i_val, count, "loop_cont")
        .unwrap();
    compiler
        .builder
        .build_conditional_branch(cont, body_bb, end_bb)
        .unwrap();

    // body
    compiler.builder.position_at_end(body_bb);
    let i_val = compiler
        .builder
        .build_load(i64_type, i_ptr, "i")
        .unwrap()
        .into_int_value();
    body(compiler, i_val)?;
    let next = compiler
        .builder
        .build_int_add(i_val, i64_type.const_int(1, false), "i_next")
        .unwrap();
    compiler.builder.build_store(i_ptr, next).unwrap();
    compiler
        .builder
        .build_unconditional_branch(cond_bb)
        .unwrap();

    compiler.builder.position_at_end(end_bb);
    Ok(())
}

/// `np.array(list)` — builds a `float64` array from a Rusthon list by unboxing
/// each element into the contiguous data buffer.
pub fn from_list<'ctx>(
    compiler: &mut Compiler<'ctx>,
    list_obj: IntValue<'ctx>,
) -> Result<IntValue<'ctx>, CodeGenError> {
    let (list_ptr, list_len) = compiler.extract_list_ptr_and_len(list_obj);
    let base = alloc_array(compiler, list_len, DTYPE_F64)?;
    let data = data_ptr(compiler, base);
    let pyobject_type = compiler.create_pyobject_type();

    emit_counted_loop(compiler, list_len, |compiler, i| {
        // List elements start at word 1 (word 0 is the list length header).
        let elem_index = compiler
            .builder
            .build_int_add(
                i,
                compiler.context.i64_type().const_int(1, false),
                "list_idx",
            )
            .unwrap();
        let src = unsafe {
            compiler
                .builder
                .build_in_bounds_gep(pyobject_type, list_ptr, &[elem_index], "list_elem_ptr")
                .unwrap()
        };
        let boxed = compiler
            .builder
            .build_load(pyobject_type, src, "boxed_elem")
            .unwrap()
            .into_int_value();
        let value = compiler.extract_payload(boxed);
        store_f64(compiler, data, i, value);
        Ok(())
    })?;

    Ok(compiler.create_pyobject_array(base))
}

/// Builds a 1-D array of `len` elements, filling each with `fill(i)` where `i`
/// is the element index as an `f64`. Backs `zeros`/`ones`/`arange`.
fn build_filled<'ctx, F>(
    compiler: &mut Compiler<'ctx>,
    len_obj: IntValue<'ctx>,
    mut fill: F,
) -> Result<IntValue<'ctx>, CodeGenError>
where
    F: FnMut(&mut Compiler<'ctx>, IntValue<'ctx>) -> FloatValue<'ctx>,
{
    let len = scalar_to_i64(compiler, len_obj);
    let base = alloc_array(compiler, len, DTYPE_F64)?;
    let data = data_ptr(compiler, base);
    emit_counted_loop(compiler, len, |compiler, i| {
        let value = fill(compiler, i);
        store_f64(compiler, data, i, value);
        Ok(())
    })?;
    Ok(compiler.create_pyobject_array(base))
}

/// `np.zeros(n)`.
pub fn zeros<'ctx>(
    compiler: &mut Compiler<'ctx>,
    len_obj: IntValue<'ctx>,
) -> Result<IntValue<'ctx>, CodeGenError> {
    build_filled(compiler, len_obj, |compiler, _i| {
        compiler.context.f64_type().const_float(0.0)
    })
}

/// `np.ones(n)`.
pub fn ones<'ctx>(
    compiler: &mut Compiler<'ctx>,
    len_obj: IntValue<'ctx>,
) -> Result<IntValue<'ctx>, CodeGenError> {
    build_filled(compiler, len_obj, |compiler, _i| {
        compiler.context.f64_type().const_float(1.0)
    })
}

/// `np.arange(n)` — array of `0.0, 1.0, ..., n-1`.
pub fn arange<'ctx>(
    compiler: &mut Compiler<'ctx>,
    len_obj: IntValue<'ctx>,
) -> Result<IntValue<'ctx>, CodeGenError> {
    build_filled(compiler, len_obj, |compiler, i| {
        compiler
            .builder
            .build_signed_int_to_float(i, compiler.context.f64_type(), "arange_val")
            .unwrap()
    })
}

/// Element-wise binary op with NumPy-style scalar broadcasting.
///
/// At least one operand is known (at runtime) to be an array; `lhs_is_array` /
/// `rhs_is_array` are the `i1` results of that test. Broadcasting is done with
/// the classic **stride trick**: an array operand reads `data[i]` (stride 1) and
/// a scalar operand reads a 1-element slot (stride 0), so the inner loop is
/// branch-free and vectorisable.
pub fn binop<'ctx>(
    compiler: &mut Compiler<'ctx>,
    op: &BinOp,
    lhs_obj: IntValue<'ctx>,
    rhs_obj: IntValue<'ctx>,
    lhs_is_array: IntValue<'ctx>,
    rhs_is_array: IntValue<'ctx>,
) -> Result<IntValue<'ctx>, CodeGenError> {
    // Per-operand data pointer + stride. Pointer arithmetic (extract/gep) never
    // dereferences, so it is safe to compute for a scalar operand too — the
    // stride-0 select just makes the bogus array pointer unused.
    let (lhs_ptr, lhs_stride) = operand_source(compiler, lhs_obj, lhs_is_array);
    let (rhs_ptr, rhs_stride) = operand_source(compiler, rhs_obj, rhs_is_array);

    let length = array_length_of(compiler, lhs_obj, rhs_obj, lhs_is_array);

    let base = alloc_array(compiler, length, DTYPE_F64)?;
    let result_data = data_ptr(compiler, base);

    let op = op.clone();
    emit_counted_loop(compiler, length, |compiler, i| {
        let off_l = compiler
            .builder
            .build_int_mul(i, lhs_stride, "off_l")
            .unwrap();
        let off_r = compiler
            .builder
            .build_int_mul(i, rhs_stride, "off_r")
            .unwrap();
        let a = load_f64(compiler, lhs_ptr, off_l);
        let b = load_f64(compiler, rhs_ptr, off_r);
        let r = match op {
            BinOp::Add => compiler.builder.build_float_add(a, b, "arr_add").unwrap(),
            BinOp::Sub => compiler.builder.build_float_sub(a, b, "arr_sub").unwrap(),
            BinOp::Mul => compiler.builder.build_float_mul(a, b, "arr_mul").unwrap(),
            BinOp::Div => compiler.builder.build_float_div(a, b, "arr_div").unwrap(),
            BinOp::Mod => compiler.builder.build_float_rem(a, b, "arr_mod").unwrap(),
            // Only arithmetic ops reach here (see `compile_binary_op`).
            _ => unreachable!("non-arithmetic array op"),
        };
        store_f64(compiler, result_data, i, r);
        Ok(())
    })?;

    Ok(compiler.create_pyobject_array(base))
}

/// Computes `(data_ptr, stride)` for one operand of an element-wise op.
/// Array → `(data buffer, 1)`; scalar → `(1-element slot holding the value, 0)`.
fn operand_source<'ctx>(
    compiler: &mut Compiler<'ctx>,
    obj: IntValue<'ctx>,
    is_array: IntValue<'ctx>,
) -> (PointerValue<'ctx>, IntValue<'ctx>) {
    let i64_type = compiler.context.i64_type();
    let f64_type = compiler.context.f64_type();

    // Array branch data pointer (safe to compute even when `obj` is scalar).
    let arr_base = compiler.extract_array_ptr(obj);
    let arr_data = data_ptr(compiler, arr_base);

    // Scalar branch: stash the unboxed scalar in a 1-element stack slot.
    let slot = compiler
        .builder
        .build_alloca(f64_type, "scalar_slot")
        .unwrap();
    let scalar = compiler.extract_payload(obj);
    compiler.builder.build_store(slot, scalar).unwrap();

    let ptr = compiler
        .builder
        .build_select(is_array, arr_data, slot, "operand_ptr")
        .unwrap()
        .into_pointer_value();
    let stride = compiler
        .builder
        .build_select(
            is_array,
            i64_type.const_int(1, false),
            i64_type.const_int(0, false),
            "operand_stride",
        )
        .unwrap()
        .into_int_value();
    (ptr, stride)
}

/// Length of whichever operand is an array (assumes equal length when both are).
/// A branch is required because `array_len` loads from the header, which is only
/// valid on the operand that is actually an array.
fn array_length_of<'ctx>(
    compiler: &mut Compiler<'ctx>,
    lhs_obj: IntValue<'ctx>,
    rhs_obj: IntValue<'ctx>,
    lhs_is_array: IntValue<'ctx>,
) -> IntValue<'ctx> {
    let i64_type = compiler.context.i64_type();
    let current_fn = compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();

    let lhs_bb = compiler
        .context
        .append_basic_block(current_fn, "len_from_lhs");
    let rhs_bb = compiler
        .context
        .append_basic_block(current_fn, "len_from_rhs");
    let cont_bb = compiler.context.append_basic_block(current_fn, "len_cont");

    compiler
        .builder
        .build_conditional_branch(lhs_is_array, lhs_bb, rhs_bb)
        .unwrap();

    compiler.builder.position_at_end(lhs_bb);
    let lhs_base = compiler.extract_array_ptr(lhs_obj);
    let lhs_len = array_len(compiler, lhs_base);
    compiler
        .builder
        .build_unconditional_branch(cont_bb)
        .unwrap();
    let lhs_pred = compiler.builder.get_insert_block().unwrap();

    compiler.builder.position_at_end(rhs_bb);
    let rhs_base = compiler.extract_array_ptr(rhs_obj);
    let rhs_len = array_len(compiler, rhs_base);
    compiler
        .builder
        .build_unconditional_branch(cont_bb)
        .unwrap();
    let rhs_pred = compiler.builder.get_insert_block().unwrap();

    compiler.builder.position_at_end(cont_bb);
    let phi = compiler.builder.build_phi(i64_type, "arr_length").unwrap();
    phi.add_incoming(&[(&lhs_len, lhs_pred), (&rhs_len, rhs_pred)]);
    phi.as_basic_value().into_int_value()
}

/// `arr[i]` — loads a single element and returns it boxed as a float PyObject.
pub fn index_load<'ctx>(
    compiler: &mut Compiler<'ctx>,
    arr_obj: IntValue<'ctx>,
    index_obj: IntValue<'ctx>,
) -> IntValue<'ctx> {
    let base = compiler.extract_array_ptr(arr_obj);
    let data = data_ptr(compiler, base);
    let index = scalar_to_i64(compiler, index_obj);
    let value = load_f64(compiler, data, index);
    compiler.create_pyobject_float(value)
}

/// `arr.sum()` — reduces the array to a scalar float PyObject.
pub fn reduce_sum<'ctx>(
    compiler: &mut Compiler<'ctx>,
    arr_obj: IntValue<'ctx>,
) -> Result<IntValue<'ctx>, CodeGenError> {
    let acc = accumulate(compiler, arr_obj)?;
    Ok(compiler.create_pyobject_float(acc))
}

/// `arr.mean()` — sum divided by element count, as a scalar float PyObject.
pub fn mean<'ctx>(
    compiler: &mut Compiler<'ctx>,
    arr_obj: IntValue<'ctx>,
) -> Result<IntValue<'ctx>, CodeGenError> {
    let base = compiler.extract_array_ptr(arr_obj);
    let len = array_len(compiler, base);
    let sum = accumulate(compiler, arr_obj)?;
    let len_f = compiler
        .builder
        .build_signed_int_to_float(len, compiler.context.f64_type(), "len_f")
        .unwrap();
    let mean = compiler
        .builder
        .build_float_div(sum, len_f, "mean")
        .unwrap();
    Ok(compiler.create_pyobject_float(mean))
}

/// Sums the elements of an array into an `f64` accumulator.
fn accumulate<'ctx>(
    compiler: &mut Compiler<'ctx>,
    arr_obj: IntValue<'ctx>,
) -> Result<FloatValue<'ctx>, CodeGenError> {
    let f64_type = compiler.context.f64_type();
    let base = compiler.extract_array_ptr(arr_obj);
    let len = array_len(compiler, base);
    let data = data_ptr(compiler, base);

    let acc_ptr = compiler.builder.build_alloca(f64_type, "acc").unwrap();
    compiler
        .builder
        .build_store(acc_ptr, f64_type.const_float(0.0))
        .unwrap();

    emit_counted_loop(compiler, len, |compiler, i| {
        let cur = compiler
            .builder
            .build_load(compiler.context.f64_type(), acc_ptr, "acc_cur")
            .unwrap()
            .into_float_value();
        let elem = load_f64(compiler, data, i);
        let next = compiler
            .builder
            .build_float_add(cur, elem, "acc_next")
            .unwrap();
        compiler.builder.build_store(acc_ptr, next).unwrap();
        Ok(())
    })?;

    Ok(compiler
        .builder
        .build_load(f64_type, acc_ptr, "acc_final")
        .unwrap()
        .into_float_value())
}

/// `arr.size` — element count as an integer PyObject.
pub fn size<'ctx>(compiler: &mut Compiler<'ctx>, arr_obj: IntValue<'ctx>) -> IntValue<'ctx> {
    let base = compiler.extract_array_ptr(arr_obj);
    let len = array_len(compiler, base);
    compiler.create_pyobject_int(len)
}

/// `arr[index] = value` — stores a scalar into an array element in place.
pub fn store_index<'ctx>(
    compiler: &mut Compiler<'ctx>,
    arr_obj: IntValue<'ctx>,
    index_obj: IntValue<'ctx>,
    value_obj: IntValue<'ctx>,
) {
    let base = compiler.extract_array_ptr(arr_obj);
    let data = data_ptr(compiler, base);
    let index = scalar_to_i64(compiler, index_obj);
    let value = compiler.extract_payload(value_obj);
    store_f64(compiler, data, index, value);
}

/// `arr[lower:upper]` — returns a new array copying the `[lower, upper)` range.
///
/// Bounds are optional (already unboxed to `i64` when present): omitted `lower`
/// defaults to `0`, omitted `upper` to the length. Both are clamped to
/// `[0, len]` (and `upper` to `>= lower`) so out-of-range slices yield a shorter
/// array rather than reading out of bounds, matching NumPy.
pub fn slice<'ctx>(
    compiler: &mut Compiler<'ctx>,
    arr_obj: IntValue<'ctx>,
    lower: Option<IntValue<'ctx>>,
    upper: Option<IntValue<'ctx>>,
) -> Result<IntValue<'ctx>, CodeGenError> {
    let i64_type = compiler.context.i64_type();
    let zero = i64_type.const_int(0, false);
    let src_base = compiler.extract_array_ptr(arr_obj);
    let len = array_len(compiler, src_base);
    let src_data = data_ptr(compiler, src_base);

    let lo = clamp(compiler, lower.unwrap_or(zero), zero, len);
    let hi = clamp(compiler, upper.unwrap_or(len), lo, len);
    let new_len = compiler.builder.build_int_sub(hi, lo, "slice_len").unwrap();

    let base = alloc_array(compiler, new_len, DTYPE_F64)?;
    let dst_data = data_ptr(compiler, base);
    emit_counted_loop(compiler, new_len, |compiler, k| {
        let src_index = compiler.builder.build_int_add(lo, k, "src_idx").unwrap();
        let value = load_f64(compiler, src_data, src_index);
        store_f64(compiler, dst_data, k, value);
        Ok(())
    })?;
    Ok(compiler.create_pyobject_array(base))
}

/// Clamps `x` into `[lo, hi]` (assumes `lo <= hi`).
fn clamp<'ctx>(
    compiler: &Compiler<'ctx>,
    x: IntValue<'ctx>,
    lo: IntValue<'ctx>,
    hi: IntValue<'ctx>,
) -> IntValue<'ctx> {
    let below = compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::SLT, x, lo, "below")
        .unwrap();
    let x = compiler
        .builder
        .build_select(below, lo, x, "clamp_lo")
        .unwrap()
        .into_int_value();
    let above = compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::SGT, x, hi, "above")
        .unwrap();
    compiler
        .builder
        .build_select(above, hi, x, "clamp_hi")
        .unwrap()
        .into_int_value()
}

/// `arr.max()` — largest element as a scalar float PyObject.
pub fn reduce_max<'ctx>(
    compiler: &mut Compiler<'ctx>,
    arr_obj: IntValue<'ctx>,
) -> Result<IntValue<'ctx>, CodeGenError> {
    reduce_extreme(compiler, arr_obj, true)
}

/// `arr.min()` — smallest element as a scalar float PyObject.
pub fn reduce_min<'ctx>(
    compiler: &mut Compiler<'ctx>,
    arr_obj: IntValue<'ctx>,
) -> Result<IntValue<'ctx>, CodeGenError> {
    reduce_extreme(compiler, arr_obj, false)
}

/// Shared min/max reduction. The accumulator starts at ∓∞ so any real element
/// wins on the first iteration; an empty array therefore reduces to ±∞.
fn reduce_extreme<'ctx>(
    compiler: &mut Compiler<'ctx>,
    arr_obj: IntValue<'ctx>,
    want_max: bool,
) -> Result<IntValue<'ctx>, CodeGenError> {
    let f64_type = compiler.context.f64_type();
    let base = compiler.extract_array_ptr(arr_obj);
    let len = array_len(compiler, base);
    let data = data_ptr(compiler, base);

    let init = if want_max {
        f64::NEG_INFINITY
    } else {
        f64::INFINITY
    };
    let acc_ptr = compiler.builder.build_alloca(f64_type, "acc").unwrap();
    compiler
        .builder
        .build_store(acc_ptr, f64_type.const_float(init))
        .unwrap();

    let pred = if want_max {
        inkwell::FloatPredicate::OGT
    } else {
        inkwell::FloatPredicate::OLT
    };
    emit_counted_loop(compiler, len, |compiler, i| {
        let cur = compiler
            .builder
            .build_load(compiler.context.f64_type(), acc_ptr, "acc_cur")
            .unwrap()
            .into_float_value();
        let elem = load_f64(compiler, data, i);
        let is_better = compiler
            .builder
            .build_float_compare(pred, elem, cur, "is_better")
            .unwrap();
        let next = compiler
            .builder
            .build_select(is_better, elem, cur, "acc_next")
            .unwrap()
            .into_float_value();
        compiler.builder.build_store(acc_ptr, next).unwrap();
        Ok(())
    })?;

    let result = compiler
        .builder
        .build_load(f64_type, acc_ptr, "acc_final")
        .unwrap()
        .into_float_value();
    Ok(compiler.create_pyobject_float(result))
}

/// Prints an array as `[e0 e1 ... en]` using the compiler's float format,
/// followed by a newline when `with_newline` is set.
pub fn print_array<'ctx>(
    compiler: &mut Compiler<'ctx>,
    arr_obj: IntValue<'ctx>,
    with_newline: bool,
) {
    let printf = compiler.runtime.add_printf(&compiler.module);
    let base = compiler.extract_array_ptr(arr_obj);
    let len = array_len(compiler, base);
    let data = data_ptr(compiler, base);

    let lbracket = compiler
        .builder
        .build_global_string_ptr("[", "arr_lbracket")
        .unwrap()
        .as_pointer_value();
    let rbracket = compiler
        .builder
        .build_global_string_ptr("]", "arr_rbracket")
        .unwrap()
        .as_pointer_value();
    let float_fmt = compiler
        .format_strings
        .get_float_format_string_no_newline(&compiler.builder);
    let space_fmt = compiler
        .format_strings
        .get_space_format_string(&compiler.builder);

    compiler
        .builder
        .build_call(printf, &[lbracket.into()], "print_lb")
        .unwrap();

    let zero = compiler.context.i64_type().const_int(0, false);
    emit_counted_loop(compiler, len, |compiler, i| {
        // Separate elements with a single space (before every element but the first).
        let needs_space = compiler
            .builder
            .build_int_compare(inkwell::IntPredicate::UGT, i, zero, "needs_space")
            .unwrap();
        let current_fn = compiler
            .builder
            .get_insert_block()
            .unwrap()
            .get_parent()
            .unwrap();
        let space_bb = compiler
            .context
            .append_basic_block(current_fn, "print_space");
        let elem_bb = compiler
            .context
            .append_basic_block(current_fn, "print_elem");
        compiler
            .builder
            .build_conditional_branch(needs_space, space_bb, elem_bb)
            .unwrap();

        compiler.builder.position_at_end(space_bb);
        compiler
            .builder
            .build_call(printf, &[space_fmt.into()], "print_sep")
            .unwrap();
        compiler
            .builder
            .build_unconditional_branch(elem_bb)
            .unwrap();

        compiler.builder.position_at_end(elem_bb);
        let elem = load_f64(compiler, data, i);
        compiler
            .builder
            .build_call(printf, &[float_fmt.into(), elem.into()], "print_elem")
            .unwrap();
        Ok(())
    })
    .expect("array print loop");

    compiler
        .builder
        .build_call(printf, &[rbracket.into()], "print_rb")
        .unwrap();
    if with_newline {
        let nl = compiler
            .format_strings
            .get_newline_format_string(&compiler.builder);
        compiler
            .builder
            .build_call(printf, &[nl.into()], "print_nl")
            .unwrap();
    }
}

/// Maps a NumPy unary ufunc name to its overloaded LLVM intrinsic (on `f64`),
/// or `None` if the name is not an element-wise unary ufunc.
///
/// The set of names here must match [`crate::compiler::arrayness::NUMPY_UFUNCS`]
/// (guarded by `test_ufunc_tables_agree`).
pub fn ufunc_intrinsic(func: &str) -> Option<&'static str> {
    Some(match func {
        "sqrt" => "llvm.sqrt",
        "abs" => "llvm.fabs",
        "exp" => "llvm.exp",
        "log" => "llvm.log",
        "sin" => "llvm.sin",
        "cos" => "llvm.cos",
        "floor" => "llvm.floor",
        "ceil" => "llvm.ceil",
        _ => return None,
    })
}

/// Looks up an overloaded-on-`f64` LLVM intrinsic and returns its declaration.
fn intrinsic_f64<'ctx>(
    compiler: &Compiler<'ctx>,
    intrinsic: &str,
) -> Result<FunctionValue<'ctx>, CodeGenError> {
    let f64_type = compiler.context.f64_type();
    let intr = Intrinsic::find(intrinsic).ok_or_else(|| {
        CodeGenError::UnsupportedFeature(format!("unknown intrinsic '{intrinsic}'"))
    })?;
    intr.get_declaration(&compiler.module, &[f64_type.into()])
        .ok_or_else(|| {
            CodeGenError::UnsupportedFeature(format!("could not declare intrinsic '{intrinsic}'"))
        })
}

/// Applies a unary intrinsic to a scalar PyObject, returning a scalar float.
pub fn unary_scalar<'ctx>(
    compiler: &mut Compiler<'ctx>,
    obj: IntValue<'ctx>,
    intrinsic: &str,
) -> Result<IntValue<'ctx>, CodeGenError> {
    let decl = intrinsic_f64(compiler, intrinsic)?;
    let x = compiler.extract_payload(obj);
    let y = match compiler
        .builder
        .build_call(decl, &[x.into()], "ufunc")
        .unwrap()
        .try_as_basic_value()
    {
        inkwell::values::ValueKind::Basic(v) => v.into_float_value(),
        _ => {
            return Err(CodeGenError::ModuleVerification(
                "intrinsic returned no value".to_string(),
            ))
        }
    };
    Ok(compiler.create_pyobject_float(y))
}

/// Applies an element-wise unary intrinsic (e.g. `llvm.sqrt`) over an array,
/// returning a new array. Like the arithmetic loops, this is the canonical
/// shape LLVM vectorises (to `vsqrtpd` and friends where a vector form exists).
pub fn unary_map<'ctx>(
    compiler: &mut Compiler<'ctx>,
    arr_obj: IntValue<'ctx>,
    intrinsic: &str,
) -> Result<IntValue<'ctx>, CodeGenError> {
    let decl = intrinsic_f64(compiler, intrinsic)?;

    let src_base = compiler.extract_array_ptr(arr_obj);
    let len = array_len(compiler, src_base);
    let src_data = data_ptr(compiler, src_base);
    let base = alloc_array(compiler, len, DTYPE_F64)?;
    let dst_data = data_ptr(compiler, base);

    emit_counted_loop(compiler, len, |compiler, i| {
        let x = load_f64(compiler, src_data, i);
        let y = match compiler
            .builder
            .build_call(decl, &[x.into()], "ufunc")
            .unwrap()
            .try_as_basic_value()
        {
            inkwell::values::ValueKind::Basic(v) => v.into_float_value(),
            _ => {
                return Err(CodeGenError::ModuleVerification(
                    "intrinsic returned no value".to_string(),
                ))
            }
        };
        store_f64(compiler, dst_data, i, y);
        Ok(())
    })?;
    Ok(compiler.create_pyobject_array(base))
}

/// `np.dot(a, b)` — 1-D dot product `sum(a[i] * b[i])` as a scalar float
/// PyObject. Assumes equal lengths (uses `a`'s length).
pub fn dot<'ctx>(
    compiler: &mut Compiler<'ctx>,
    a_obj: IntValue<'ctx>,
    b_obj: IntValue<'ctx>,
) -> Result<IntValue<'ctx>, CodeGenError> {
    let f64_type = compiler.context.f64_type();
    let a_base = compiler.extract_array_ptr(a_obj);
    let len = array_len(compiler, a_base);
    let a_data = data_ptr(compiler, a_base);
    let b_base = compiler.extract_array_ptr(b_obj);
    let b_data = data_ptr(compiler, b_base);

    let acc_ptr = compiler.builder.build_alloca(f64_type, "acc").unwrap();
    compiler
        .builder
        .build_store(acc_ptr, f64_type.const_float(0.0))
        .unwrap();

    emit_counted_loop(compiler, len, |compiler, i| {
        let x = load_f64(compiler, a_data, i);
        let y = load_f64(compiler, b_data, i);
        let prod = compiler.builder.build_float_mul(x, y, "prod").unwrap();
        let cur = compiler
            .builder
            .build_load(compiler.context.f64_type(), acc_ptr, "acc_cur")
            .unwrap()
            .into_float_value();
        let next = compiler
            .builder
            .build_float_add(cur, prod, "acc_next")
            .unwrap();
        compiler.builder.build_store(acc_ptr, next).unwrap();
        Ok(())
    })?;

    let result = compiler
        .builder
        .build_load(f64_type, acc_ptr, "acc_final")
        .unwrap()
        .into_float_value();
    Ok(compiler.create_pyobject_float(result))
}

/// `arr.prod()` — product of all elements as a scalar float PyObject.
pub fn reduce_prod<'ctx>(
    compiler: &mut Compiler<'ctx>,
    arr_obj: IntValue<'ctx>,
) -> Result<IntValue<'ctx>, CodeGenError> {
    let f64_type = compiler.context.f64_type();
    let base = compiler.extract_array_ptr(arr_obj);
    let len = array_len(compiler, base);
    let data = data_ptr(compiler, base);

    let acc_ptr = compiler.builder.build_alloca(f64_type, "acc").unwrap();
    compiler
        .builder
        .build_store(acc_ptr, f64_type.const_float(1.0))
        .unwrap();

    emit_counted_loop(compiler, len, |compiler, i| {
        let cur = compiler
            .builder
            .build_load(compiler.context.f64_type(), acc_ptr, "acc_cur")
            .unwrap()
            .into_float_value();
        let elem = load_f64(compiler, data, i);
        let next = compiler
            .builder
            .build_float_mul(cur, elem, "acc_next")
            .unwrap();
        compiler.builder.build_store(acc_ptr, next).unwrap();
        Ok(())
    })?;

    let result = compiler
        .builder
        .build_load(f64_type, acc_ptr, "acc_final")
        .unwrap()
        .into_float_value();
    Ok(compiler.create_pyobject_float(result))
}

/// Unboxes a scalar PyObject (int/float/bool) to an `i64` index value.
fn scalar_to_i64<'ctx>(compiler: &Compiler<'ctx>, obj: IntValue<'ctx>) -> IntValue<'ctx> {
    let payload = compiler.extract_payload(obj);
    compiler
        .builder
        .build_float_to_signed_int(payload, compiler.context.i64_type(), "to_i64")
        .unwrap()
}
