//! Value and Type System for NaN-Boxing
//!
//! This module implements the NaN-boxing scheme used to represent Python objects
//! as single 64-bit values, achieving 50% memory reduction compared to struct-based representations.
//!
//! ## Memory Layout (64-bit value)
//!
//! ```text
//! Floats: [  sign  ][  exponent  ][        mantissa        ]
//!         [ 1 bit  ][ 11 bits    ][     52 bits            ]
//!
//! Tagged: [1][11111111111][1][ tag (3 bits) ][ payload (48 bits) ]
//!          ^      ^         ^       ^                 ^
//!          |      |         |       |                 +-- Value/pointer
//!          |      |         |       +-- Type tag
//!          |      |         +-- Quiet NaN bit
//!          |      +-- All ones (NaN exponent)
//!          +-- Sign bit
//! ```
//!
//! ## Type Tags
//! - TAG_INT = 0: Signed 48-bit integer
//! - TAG_BOOL = 1: Boolean (1-bit payload)
//! - TAG_STRING = 2: String pointer (48-bit)
//! - TAG_LIST = 3: List pointer (48-bit)
//! - Floats: No tag (stored as canonical float64)

use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::values::{FloatValue, IntValue, PointerValue};

// NaN-boxing constants for tagged pointers
// PyObject is now represented as a single i64 using NaN-boxing
const QNAN: u64 = 0x7FF8_0000_0000_0000;
const TAG_MASK: u64 = 0x0007_0000_0000_0000;
const PAYLOAD_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

// Type tags for NaN-boxing (stored in bits 48-50)
const TAG_INT: u64 = 0;
const TAG_BOOL: u64 = 1;
const TAG_STRING: u64 = 2;
const TAG_LIST: u64 = 3;

// Legacy type tags (for compatibility with print dispatch logic)
pub const TYPE_TAG_INT: u8 = 0;
pub const TYPE_TAG_FLOAT: u8 = 1;
pub const TYPE_TAG_BOOL: u8 = 2;
pub const TYPE_TAG_STRING: u8 = 3;
pub const TYPE_TAG_LIST: u8 = 4;

/// Value manager for NaN-boxing operations
///
/// This struct provides methods for creating and extracting values from NaN-boxed PyObjects.
/// It encapsulates all type system operations, making it easy to switch between different
/// value representations (e.g., structs vs NaN-boxing) by only modifying this module.
pub struct ValueManager<'ctx> {
    context: &'ctx Context,
}

impl<'ctx> ValueManager<'ctx> {
    /// Creates a new ValueManager
    pub fn new(context: &'ctx Context) -> Self {
        Self { context }
    }

    /// Returns the PyObject type: i64 (NaN-boxed value)
    /// PyObjects are now single 64-bit values using NaN-boxing for 50% memory reduction
    pub fn pyobject_type(&self) -> inkwell::types::IntType<'ctx> {
        self.context.i64_type()
    }

    /// Creates a PyObject value from an integer using NaN-boxing
    pub fn create_int(&self, builder: &Builder<'ctx>, value: IntValue<'ctx>) -> IntValue<'ctx> {
        // NaN-box: QNAN | (TAG_INT << 48) | (value & PAYLOAD_MASK)
        // Truncate to 48 bits (sign-extended)
        let payload_mask = self.context.i64_type().const_int(PAYLOAD_MASK, false);
        let payload = builder
            .build_and(value, payload_mask, "int_payload")
            .unwrap();

        // Create tag bits: TAG_INT << 48
        let tag_shifted = self.context.i64_type().const_int(TAG_INT << 48, false);

        // Combine: QNAN | tag | payload
        let qnan_const = self.context.i64_type().const_int(QNAN, false);
        let with_tag = builder
            .build_or(qnan_const, tag_shifted, "with_tag")
            .unwrap();
        builder.build_or(with_tag, payload, "pyobject_int").unwrap()
    }

    /// Creates a PyObject value from a float using NaN-boxing
    /// Floats are stored as-is in their canonical IEEE 754 representation
    pub fn create_float(&self, builder: &Builder<'ctx>, value: FloatValue<'ctx>) -> IntValue<'ctx> {
        // For floats, we store them directly (not NaN-boxed)
        // Just bitcast f64 to i64
        builder
            .build_bit_cast(value, self.context.i64_type(), "float_as_i64")
            .unwrap()
            .into_int_value()
    }

    /// Creates a PyObject value from a boolean using NaN-boxing
    pub fn create_bool(&self, builder: &Builder<'ctx>, value: IntValue<'ctx>) -> IntValue<'ctx> {
        // NaN-box: QNAN | (TAG_BOOL << 48) | (0 or 1)
        // Zero-extend i1 to i64
        let payload = builder
            .build_int_z_extend(value, self.context.i64_type(), "bool_payload")
            .unwrap();

        // Create tag bits: TAG_BOOL << 48
        let tag_shifted = self.context.i64_type().const_int(TAG_BOOL << 48, false);

        // Combine: QNAN | tag | payload
        let qnan_const = self.context.i64_type().const_int(QNAN, false);
        let with_tag = builder
            .build_or(qnan_const, tag_shifted, "with_tag")
            .unwrap();
        builder
            .build_or(with_tag, payload, "pyobject_bool")
            .unwrap()
    }

    /// Creates a PyObject value from a string pointer using NaN-boxing
    pub fn create_string(
        &self,
        builder: &Builder<'ctx>,
        ptr: PointerValue<'ctx>,
    ) -> IntValue<'ctx> {
        // NaN-box: QNAN | (TAG_STRING << 48) | (ptr & PAYLOAD_MASK)
        // Convert pointer to i64
        let ptr_as_int = builder
            .build_ptr_to_int(ptr, self.context.i64_type(), "ptr_to_int")
            .unwrap();

        // Mask to 48 bits
        let payload_mask = self.context.i64_type().const_int(PAYLOAD_MASK, false);
        let payload = builder
            .build_and(ptr_as_int, payload_mask, "ptr_payload")
            .unwrap();

        // Create tag bits: TAG_STRING << 48
        let tag_shifted = self.context.i64_type().const_int(TAG_STRING << 48, false);

        // Combine: QNAN | tag | payload
        let qnan_const = self.context.i64_type().const_int(QNAN, false);
        let with_tag = builder
            .build_or(qnan_const, tag_shifted, "with_tag")
            .unwrap();
        builder
            .build_or(with_tag, payload, "pyobject_string")
            .unwrap()
    }

    /// Creates a PyObject value from a list pointer and length using NaN-boxing
    /// The pointer should point to a memory layout: [length: i64][element_0: i64]...[element_n: i64]
    /// The length is stored at offset 0 in the allocation
    pub fn create_list(
        &self,
        builder: &Builder<'ctx>,
        ptr: PointerValue<'ctx>,
        _len: usize,
    ) -> IntValue<'ctx> {
        // Store the pointer in the NaN-boxed value
        // NaN-box: QNAN | (TAG_LIST << 48) | (ptr & PAYLOAD_MASK)
        let ptr_as_int = builder
            .build_ptr_to_int(ptr, self.context.i64_type(), "ptr_to_int")
            .unwrap();

        // Mask to 48 bits
        let payload_mask = self.context.i64_type().const_int(PAYLOAD_MASK, false);
        let payload = builder
            .build_and(ptr_as_int, payload_mask, "list_ptr_payload")
            .unwrap();

        // Create tag bits: TAG_LIST << 48
        let tag_shifted = self.context.i64_type().const_int(TAG_LIST << 48, false);

        // Combine: QNAN | tag | payload
        let qnan_const = self.context.i64_type().const_int(QNAN, false);
        let with_tag = builder
            .build_or(qnan_const, tag_shifted, "with_tag")
            .unwrap();
        builder
            .build_or(with_tag, payload, "pyobject_list")
            .unwrap()
    }

    /// Extracts a string pointer from a PyObject
    /// Assumes the PyObject has a STRING tag
    pub fn extract_string_ptr(
        &self,
        builder: &Builder<'ctx>,
        pyobject: IntValue<'ctx>,
    ) -> PointerValue<'ctx> {
        // Extract payload (lower 48 bits)
        let payload_mask = self.context.i64_type().const_int(PAYLOAD_MASK, false);
        let payload = builder
            .build_and(pyobject, payload_mask, "extract_ptr_payload")
            .unwrap();

        // Convert to pointer
        builder
            .build_int_to_ptr(
                payload,
                self.context.ptr_type(inkwell::AddressSpace::default()),
                "payload_to_ptr",
            )
            .unwrap()
    }

    /// Extracts a list pointer and length from a PyObject
    /// Assumes the PyObject has a LIST tag
    /// The pointer points to: [length: i64][element_0: i64]...[element_n: i64]
    pub fn extract_list_ptr_and_len(
        &self,
        builder: &Builder<'ctx>,
        pyobject: IntValue<'ctx>,
    ) -> (PointerValue<'ctx>, IntValue<'ctx>) {
        // Extract payload (lower 48 bits)
        let payload_mask = self.context.i64_type().const_int(PAYLOAD_MASK, false);
        let payload = builder
            .build_and(pyobject, payload_mask, "extract_list_payload")
            .unwrap();

        // Convert to pointer
        let ptr = builder
            .build_int_to_ptr(
                payload,
                self.context.ptr_type(inkwell::AddressSpace::default()),
                "payload_to_list_ptr",
            )
            .unwrap();

        // Read the length from offset 0
        let pyobject_type = self.pyobject_type();
        let len_ptr = unsafe {
            builder
                .build_in_bounds_gep(
                    pyobject_type,
                    ptr,
                    &[self.context.i64_type().const_int(0, false)],
                    "len_ptr",
                )
                .unwrap()
        };
        let len = builder
            .build_load(pyobject_type, len_ptr, "list_len")
            .unwrap()
            .into_int_value();

        (ptr, len)
    }

    /// Reconstructs a PyObject from a tag and payload
    /// tag: IntValue (i64) representing the type tag (0=INT, 1=FLOAT, 2=BOOL, 3=STRING, 4=LIST)
    /// payload: FloatValue representing the payload as f64
    /// Returns: IntValue (i64) representing the NaN-boxed PyObject
    pub fn create_from_tag_and_payload(
        &self,
        builder: &Builder<'ctx>,
        tag: IntValue<'ctx>,
        payload: FloatValue<'ctx>,
    ) -> IntValue<'ctx> {
        let float_tag = self
            .context
            .i64_type()
            .const_int(TYPE_TAG_FLOAT as u64, false);
        let is_float = builder
            .build_int_compare(inkwell::IntPredicate::EQ, tag, float_tag, "is_float_tag")
            .unwrap();

        // For floats: just bitcast f64 to i64
        let float_result = builder
            .build_bit_cast(payload, self.context.i64_type(), "float_to_i64")
            .unwrap()
            .into_int_value();

        // For non-floats: Convert back from external tag to internal tag, then NaN-box
        // TYPE_TAG_INT (0) -> TAG_INT (0)
        // TYPE_TAG_BOOL (2) -> TAG_BOOL (1)
        // TYPE_TAG_STRING (3) -> TAG_STRING (2)
        // TYPE_TAG_LIST (4) -> TAG_LIST (3)
        let bool_tag = self
            .context
            .i64_type()
            .const_int(TYPE_TAG_BOOL as u64, false);
        let string_tag = self
            .context
            .i64_type()
            .const_int(TYPE_TAG_STRING as u64, false);
        let list_tag = self
            .context
            .i64_type()
            .const_int(TYPE_TAG_LIST as u64, false);

        let is_bool = builder
            .build_int_compare(inkwell::IntPredicate::EQ, tag, bool_tag, "is_bool")
            .unwrap();
        let is_string = builder
            .build_int_compare(inkwell::IntPredicate::EQ, tag, string_tag, "is_string")
            .unwrap();
        let is_list = builder
            .build_int_compare(inkwell::IntPredicate::EQ, tag, list_tag, "is_list")
            .unwrap();

        let internal_tag_1 = self.context.i64_type().const_int(TAG_BOOL, false);
        let internal_tag_2 = self.context.i64_type().const_int(TAG_STRING, false);
        let internal_tag_3 = self.context.i64_type().const_int(TAG_LIST, false);
        let internal_tag_0 = self.context.i64_type().const_int(TAG_INT, false);

        let internal_tag_temp1 = builder
            .build_select(is_bool, internal_tag_1, internal_tag_0, "tag_temp1")
            .unwrap()
            .into_int_value();
        let internal_tag_temp2 = builder
            .build_select(is_string, internal_tag_2, internal_tag_temp1, "tag_temp2")
            .unwrap()
            .into_int_value();
        let internal_tag = builder
            .build_select(is_list, internal_tag_3, internal_tag_temp2, "internal_tag")
            .unwrap()
            .into_int_value();

        // Convert payload from f64 to i64 bits
        let payload_i64 = builder
            .build_float_to_signed_int(payload, self.context.i64_type(), "payload_to_i64")
            .unwrap();

        // Mask to 48 bits
        let payload_mask = self.context.i64_type().const_int(PAYLOAD_MASK, false);
        let payload_masked = builder
            .build_and(payload_i64, payload_mask, "payload_masked")
            .unwrap();

        // Build NaN-boxed value: QNAN | (tag << 48) | payload
        let tag_shifted = builder
            .build_left_shift(
                internal_tag,
                self.context.i64_type().const_int(48, false),
                "tag_shifted",
            )
            .unwrap();
        let qnan_const = self.context.i64_type().const_int(QNAN, false);
        let with_qnan = builder
            .build_or(qnan_const, tag_shifted, "with_qnan")
            .unwrap();
        let nanboxed_result = builder
            .build_or(with_qnan, payload_masked, "nanboxed")
            .unwrap();

        // Select between float and NaN-boxed based on tag
        builder
            .build_select(is_float, float_result, nanboxed_result, "pyobject")
            .unwrap()
            .into_int_value()
    }

    /// Checks if a PyObject is a float (not NaN-boxed)
    pub fn is_float(&self, builder: &Builder<'ctx>, pyobject: IntValue<'ctx>) -> IntValue<'ctx> {
        // A value is a float if (value & QNAN) != QNAN
        let qnan_const = self.context.i64_type().const_int(QNAN, false);
        let masked = builder
            .build_and(pyobject, qnan_const, "check_qnan")
            .unwrap();
        let is_not_qnan = builder
            .build_int_compare(inkwell::IntPredicate::NE, masked, qnan_const, "is_float")
            .unwrap();
        is_not_qnan
    }

    /// Extracts the tag from a NaN-boxed PyObject
    /// Returns tag as i64 for compatibility (0=INT, 1=FLOAT, 2=BOOL, 3=STRING, 4=LIST)
    pub fn extract_tag(&self, builder: &Builder<'ctx>, pyobject: IntValue<'ctx>) -> IntValue<'ctx> {
        // Check if it's a float first
        let is_float_val = self.is_float(builder, pyobject);

        // If not NaN-boxed (i.e., it's a float), return TYPE_TAG_FLOAT (1)
        // Otherwise extract tag from bits 48-50
        let tag_mask = self.context.i64_type().const_int(TAG_MASK, false);
        let tag_bits = builder.build_and(pyobject, tag_mask, "tag_bits").unwrap();
        let tag_shifted = builder
            .build_right_shift(
                tag_bits,
                self.context.i64_type().const_int(48, false),
                false,
                "tag",
            )
            .unwrap();

        // Convert internal tag to external tag
        // TAG_INT (0) -> TYPE_TAG_INT (0)
        // TAG_BOOL (1) -> TYPE_TAG_BOOL (2)
        // TAG_STRING (2) -> TYPE_TAG_STRING (3)
        // TAG_LIST (3) -> TYPE_TAG_LIST (4)
        let tag_map_bool = self
            .context
            .i64_type()
            .const_int(TYPE_TAG_BOOL as u64, false);
        let tag_map_string = self
            .context
            .i64_type()
            .const_int(TYPE_TAG_STRING as u64, false);
        let tag_map_list = self
            .context
            .i64_type()
            .const_int(TYPE_TAG_LIST as u64, false);

        // Select based on tag value
        let is_bool = builder
            .build_int_compare(
                inkwell::IntPredicate::EQ,
                tag_shifted,
                self.context.i64_type().const_int(TAG_BOOL, false),
                "is_bool",
            )
            .unwrap();
        let is_string = builder
            .build_int_compare(
                inkwell::IntPredicate::EQ,
                tag_shifted,
                self.context.i64_type().const_int(TAG_STRING, false),
                "is_string",
            )
            .unwrap();
        let is_list = builder
            .build_int_compare(
                inkwell::IntPredicate::EQ,
                tag_shifted,
                self.context.i64_type().const_int(TAG_LIST, false),
                "is_list",
            )
            .unwrap();

        // Build the mapped tag
        let mapped_tag = builder
            .build_select(is_bool, tag_map_bool, tag_shifted, "map_bool")
            .unwrap()
            .into_int_value();
        let mapped_tag = builder
            .build_select(is_string, tag_map_string, mapped_tag, "map_string")
            .unwrap()
            .into_int_value();
        let mapped_tag = builder
            .build_select(is_list, tag_map_list, mapped_tag, "map_list")
            .unwrap()
            .into_int_value();

        // If it's a float, return TYPE_TAG_FLOAT, otherwise return mapped tag
        let float_tag = self
            .context
            .i64_type()
            .const_int(TYPE_TAG_FLOAT as u64, false);
        builder
            .build_select(is_float_val, float_tag, mapped_tag, "final_tag")
            .unwrap()
            .into_int_value()
    }

    /// Extracts the payload as f64 from a PyObject
    /// For floats: bitcast i64 to f64
    /// For integers/bools: extract and convert to f64
    /// For pointers: extract as integer and convert to f64
    pub fn extract_payload(
        &self,
        builder: &Builder<'ctx>,
        pyobject: IntValue<'ctx>,
    ) -> FloatValue<'ctx> {
        let is_float_val = self.is_float(builder, pyobject);

        // If it's a float, bitcast i64 to f64
        let as_float = builder
            .build_bit_cast(pyobject, self.context.f64_type(), "i64_to_f64")
            .unwrap()
            .into_float_value();

        // Otherwise, extract lower 48 bits and convert to f64
        let payload_mask = self.context.i64_type().const_int(PAYLOAD_MASK, false);
        let payload_int = builder
            .build_and(pyobject, payload_mask, "extract_payload")
            .unwrap();

        // Sign-extend from 48 bits to 64 bits for integers
        let sign_bit = builder
            .build_right_shift(
                payload_int,
                self.context.i64_type().const_int(47, false),
                false,
                "sign_bit",
            )
            .unwrap();
        let is_negative = builder
            .build_int_compare(
                inkwell::IntPredicate::EQ,
                sign_bit,
                self.context.i64_type().const_int(1, false),
                "is_negative",
            )
            .unwrap();

        // If negative, fill upper bits with 1s
        let sign_extension = self.context.i64_type().const_int(!PAYLOAD_MASK, false);
        let extended = builder
            .build_or(payload_int, sign_extension, "sign_extend")
            .unwrap();
        let signed_payload = builder
            .build_select(is_negative, extended, payload_int, "signed_payload")
            .unwrap()
            .into_int_value();

        // Convert to f64
        let payload_as_float = builder
            .build_signed_int_to_float(signed_payload, self.context.f64_type(), "payload_to_f64")
            .unwrap();

        // Select based on whether it's a float
        builder
            .build_select(is_float_val, as_float, payload_as_float, "final_payload")
            .unwrap()
            .into_float_value()
    }

    /// Converts a PyObject to a boolean (i1) for conditionals
    /// Returns true if the value is non-zero
    pub fn to_bool(&self, builder: &Builder<'ctx>, pyobject: IntValue<'ctx>) -> IntValue<'ctx> {
        let payload = self.extract_payload(builder, pyobject);
        let zero = self.context.f64_type().const_float(0.0);
        builder
            .build_float_compare(inkwell::FloatPredicate::ONE, payload, zero, "to_bool")
            .unwrap()
    }
}
