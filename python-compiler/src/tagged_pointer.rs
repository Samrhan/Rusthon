/// Tagged Pointer Implementation for PyObject Optimization
///
/// This module implements a NaN-boxing scheme to reduce PyObject size from 16 bytes to 8 bytes.
///
/// ## Memory Layout (64-bit value)
///
/// ### For actual floating-point numbers:
/// ```
/// [  sign  ][  exponent  ][        mantissa        ]
/// [ 1 bit  ][ 11 bits    ][     52 bits            ]
/// ```
///
/// ### For tagged values (using quiet NaN encoding):
/// ```
/// [1][11111111111][1][ tag (3 bits) ][ payload (48 bits) ]
///  ^      ^         ^       ^                 ^
///  |      |         |       |                 |
///  |      |         |       |                 +-- Integer value or pointer
///  |      |         |       +-- Type tag (INT=0, BOOL=1, STRING=2, LIST=3)
///  |      |         +-- Quiet NaN bit
///  |      +-- All ones (NaN exponent)
///  +-- Sign bit (set for NaN box)
/// ```
///
/// ## Type Encoding
///
/// - Floats: Stored as-is (canonical float representation)
/// - Integers: NaN-boxed with tag 0 and 48-bit signed payload
/// - Booleans: NaN-boxed with tag 1 and 1-bit payload
/// - Strings: NaN-boxed with tag 2 and 48-bit pointer payload
/// - Lists: NaN-boxed with tag 3 and 48-bit pointer payload
///
/// ## Advantages
/// - 50% memory reduction (16 bytes → 8 bytes)
/// - Cache-friendly (fits in single register)
/// - Fast type checking (single bit test for float vs non-float)
/// - Compatible with x86-64 user-space pointers (48-bit)
///
/// ## Limitations
/// - Integers limited to ±140,737,488,355,328 (48-bit)
/// - Pointers limited to 48-bit (acceptable on x86-64)
/// - Slightly more complex implementation than struct approach
// Bit patterns for NaN boxing
const QNAN: u64 = 0x7FF8_0000_0000_0000;
const TAG_MASK: u64 = 0x0007_0000_0000_0000;
const PAYLOAD_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

// Type tags (3 bits)
const TAG_INT: u64 = 0;
const TAG_BOOL: u64 = 1;
const TAG_STRING: u64 = 2;
const TAG_LIST: u64 = 3;

/// Represents a Python object using NaN-boxing for efficient memory usage
#[derive(Copy, Clone, Debug)]
pub struct TaggedPointer(u64);

impl TaggedPointer {
    /// Creates a PyObject containing an integer
    #[inline]
    pub fn from_int(value: i64) -> Self {
        // Truncate to 48 bits (sign-extended)
        let payload = (value as u64) & PAYLOAD_MASK;
        let tagged = QNAN | (TAG_INT << 48) | payload;
        TaggedPointer(tagged)
    }

    /// Creates a PyObject containing a float
    #[inline]
    pub fn from_float(value: f64) -> Self {
        // Store float directly - canonical representation
        TaggedPointer(value.to_bits())
    }

    /// Creates a PyObject containing a boolean
    #[inline]
    pub fn from_bool(value: bool) -> Self {
        let payload = if value { 1 } else { 0 };
        let tagged = QNAN | (TAG_BOOL << 48) | payload;
        TaggedPointer(tagged)
    }

    /// Creates a PyObject containing a string pointer
    #[inline]
    pub fn from_string_ptr(ptr: u64) -> Self {
        // Ensure pointer fits in 48 bits
        let payload = ptr & PAYLOAD_MASK;
        let tagged = QNAN | (TAG_STRING << 48) | payload;
        TaggedPointer(tagged)
    }

    /// Creates a PyObject containing a list pointer
    #[inline]
    pub fn from_list_ptr(ptr: u64) -> Self {
        let payload = ptr & PAYLOAD_MASK;
        let tagged = QNAN | (TAG_LIST << 48) | payload;
        TaggedPointer(tagged)
    }

    /// Checks if this PyObject contains a float
    #[inline]
    pub fn is_float(&self) -> bool {
        // A value is a float if it's NOT a NaN-boxed value
        // NaN-boxed values have exponent=0x7FF and quiet NaN bit set
        (self.0 & 0x7FF8_0000_0000_0000) != QNAN
    }

    /// Checks if this PyObject contains an integer
    #[inline]
    pub fn is_int(&self) -> bool {
        !self.is_float() && self.get_tag() == TAG_INT
    }

    /// Checks if this PyObject contains a boolean
    #[inline]
    pub fn is_bool(&self) -> bool {
        !self.is_float() && self.get_tag() == TAG_BOOL
    }

    /// Checks if this PyObject contains a string
    #[inline]
    pub fn is_string(&self) -> bool {
        !self.is_float() && self.get_tag() == TAG_STRING
    }

    /// Checks if this PyObject contains a list
    #[inline]
    pub fn is_list(&self) -> bool {
        !self.is_float() && self.get_tag() == TAG_LIST
    }

    /// Extracts the type tag (only valid for non-float values)
    #[inline]
    fn get_tag(&self) -> u64 {
        (self.0 & TAG_MASK) >> 48
    }

    /// Extracts the payload (only valid for non-float values)
    #[inline]
    fn get_payload(&self) -> u64 {
        self.0 & PAYLOAD_MASK
    }

    /// Extracts the integer value (assumes is_int() == true)
    #[inline]
    pub fn as_int(&self) -> i64 {
        let payload = self.get_payload();
        // Sign-extend from 48 bits to 64 bits
        let sign_bit = (payload >> 47) & 1;
        if sign_bit == 1 {
            // Negative: fill upper bits with 1s
            (payload | !PAYLOAD_MASK) as i64
        } else {
            // Positive: upper bits already 0
            payload as i64
        }
    }

    /// Extracts the float value (assumes is_float() == true)
    #[inline]
    pub fn as_float(&self) -> f64 {
        f64::from_bits(self.0)
    }

    /// Extracts the boolean value (assumes is_bool() == true)
    #[inline]
    pub fn as_bool(&self) -> bool {
        self.get_payload() != 0
    }

    /// Extracts the string pointer (assumes is_string() == true)
    #[inline]
    pub fn as_string_ptr(&self) -> u64 {
        self.get_payload()
    }

    /// Extracts the list pointer (assumes is_list() == true)
    #[inline]
    pub fn as_list_ptr(&self) -> u64 {
        self.get_payload()
    }

    /// Returns the raw 64-bit representation
    #[inline]
    pub fn as_u64(&self) -> u64 {
        self.0
    }

    /// Creates a TaggedPointer from a raw 64-bit value
    #[inline]
    pub fn from_u64(value: u64) -> Self {
        TaggedPointer(value)
    }

    /// Converts to f64 for LLVM codegen (returns the payload as double)
    /// For integers, booleans, and pointers, this returns them as f64 for compatibility
    #[inline]
    pub fn to_f64_payload(&self) -> f64 {
        if self.is_float() {
            self.as_float()
        } else if self.is_int() {
            self.as_int() as f64
        } else if self.is_bool() {
            if self.as_bool() {
                1.0
            } else {
                0.0
            }
        } else {
            // For pointers (string/list), convert to f64
            // This is for backward compatibility with existing codegen
            self.get_payload() as f64
        }
    }

    /// Returns the type tag as u8 for LLVM codegen compatibility
    #[inline]
    pub fn type_tag(&self) -> u8 {
        if self.is_float() {
            1 // TYPE_TAG_FLOAT
        } else {
            match self.get_tag() {
                TAG_INT => 0,    // TYPE_TAG_INT
                TAG_BOOL => 2,   // TYPE_TAG_BOOL
                TAG_STRING => 3, // TYPE_TAG_STRING
                TAG_LIST => 4,   // TYPE_TAG_LIST
                _ => 0,          // Fallback
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;

    #[test]
    fn test_integer_boxing() {
        let obj = TaggedPointer::from_int(42);
        assert!(obj.is_int());
        assert!(!obj.is_float());
        assert_eq!(obj.as_int(), 42);
    }

    #[test]
    fn test_negative_integer() {
        let obj = TaggedPointer::from_int(-100);
        assert!(obj.is_int());
        assert_eq!(obj.as_int(), -100);
    }

    #[test]
    fn test_float_boxing() {
        let obj = TaggedPointer::from_float(123.456);
        assert!(obj.is_float());
        assert!(!obj.is_int());
        assert_eq!(obj.as_float(), 123.456);
    }

    #[test]
    fn test_boolean_boxing() {
        let obj_true = TaggedPointer::from_bool(true);
        let obj_false = TaggedPointer::from_bool(false);

        assert!(obj_true.is_bool());
        assert!(obj_false.is_bool());
        assert!(obj_true.as_bool());
        assert!(!obj_false.as_bool());
    }

    #[test]
    fn test_string_pointer() {
        let ptr: u64 = 0x123456789ABC;
        let obj = TaggedPointer::from_string_ptr(ptr);

        assert!(obj.is_string());
        assert!(!obj.is_float());
        assert_eq!(obj.as_string_ptr(), ptr);
    }

    #[test]
    fn test_size() {
        assert_eq!(mem::size_of::<TaggedPointer>(), 8);
    }

    #[test]
    fn test_type_discrimination() {
        let int_obj = TaggedPointer::from_int(100);
        let float_obj = TaggedPointer::from_float(2.5);
        let bool_obj = TaggedPointer::from_bool(true);
        let str_obj = TaggedPointer::from_string_ptr(0x1000);

        assert!(
            int_obj.is_int() && !int_obj.is_float() && !int_obj.is_bool() && !int_obj.is_string()
        );
        assert!(float_obj.is_float() && !float_obj.is_int() && !float_obj.is_bool());
        assert!(bool_obj.is_bool() && !bool_obj.is_int() && !bool_obj.is_float());
        assert!(str_obj.is_string() && !str_obj.is_int() && !str_obj.is_float());
    }
}
