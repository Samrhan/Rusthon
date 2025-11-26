# Type System

Rusthon implements dynamic typing using a tagged union approach.

## PyObject Structure

```rust
%PyObject = type { i8, double }
```

**Fields:**
- `tag` (i8): Type discriminator
- `payload` (double): 64-bit value storage

**Size:** 16 bytes (8-byte alignment)

## Type Tags

```rust
TYPE_TAG_INT    = 0  // Integer values
TYPE_TAG_FLOAT  = 1  // Floating-point values
TYPE_TAG_BOOL   = 2  // Boolean values (0.0 or 1.0)
TYPE_TAG_STRING = 3  // String pointers (cast to double)
```

## Type Representations

### Integers
```llvm
%int_obj = insertvalue %PyObject { i8 0, double undef }, double 42.0, 1
```
- Tag: 0
- Payload: Integer value as double
- Range: ±2^53 (double precision limit)

### Floats
```llvm
%float_obj = insertvalue %PyObject { i8 1, double undef }, double 3.14, 1
```
- Tag: 1
- Payload: Native double value
- Range: Full double precision

### Booleans
```llvm
%bool_obj = insertvalue %PyObject { i8 2, double undef }, double 1.0, 1
```
- Tag: 2
- Payload: 0.0 (false) or 1.0 (true)

### Strings
```llvm
%str_ptr = call ptr @malloc(i64 %len)
call void @memcpy(ptr %str_ptr, ptr @literal, i64 %len, i1 false)
%ptr_as_double = ptrtoint ptr %str_ptr to i64
%ptr_as_double_f = sitofp i64 %ptr_as_double to double
%str_obj = insertvalue %PyObject { i8 3, double undef }, double %ptr_as_double_f, 1
```
- Tag: 3
- Payload: Pointer cast to double
- Storage: Heap-allocated (malloc)
- Memory: Never freed (leak)

## Type Promotion

### Integer + Float → Float
```python
x = 5 + 3.14  # Result: 8.14 (float)
```

**Logic:**
```rust
if left_tag == INT && right_tag == FLOAT:
    result_tag = FLOAT
    result_payload = left_payload + right_payload
elif left_tag == FLOAT && right_tag == INT:
    result_tag = FLOAT
    result_payload = left_payload + right_payload
elif left_tag == INT && right_tag == INT:
    result_tag = INT
    result_payload = left_payload + right_payload
else:  // both FLOAT
    result_tag = FLOAT
    result_payload = left_payload + right_payload
```

### Type Promotion Table

| Left | Operator | Right | Result |
|------|----------|-------|--------|
| INT | +, -, *, /, % | INT | INT |
| INT | +, -, *, /, % | FLOAT | FLOAT |
| FLOAT | +, -, *, /, % | INT | FLOAT |
| FLOAT | +, -, *, /, % | FLOAT | FLOAT |

## Runtime Type Checking

### Print Dispatch
```llvm
%tag = extractvalue %PyObject %obj, 0
%is_string = icmp eq i8 %tag, 3
br i1 %is_string, label %print_string, label %check_int

check_int:
%is_int = icmp eq i8 %tag, 0
br i1 %is_int, label %print_int, label %print_float

print_string:
  ; Extract pointer and print
  br label %merge

print_int:
  ; Extract int and print
  br label %merge

print_float:
  ; Extract float and print
  br label %merge

merge:
  ; Continue
```

### Bitwise Operations
Bitwise operations require integer operands:

```llvm
%tag = extractvalue %PyObject %obj, 0
%payload = extractvalue %PyObject %obj, 1
%as_int = fptosi double %payload to i64
; Perform bitwise operation
%result_double = sitofp i64 %bitwise_result to double
%result_obj = insertvalue %PyObject { i8 0, double undef }, double %result_double, 1
```

## Type Conversion Utilities

### pyobject_to_bool
```llvm
; Extract payload and compare to 0.0
%payload = extractvalue %PyObject %obj, 1
%is_nonzero = fcmp one double %payload, 0.0
; Returns i1
```

### pyobject_to_int
```llvm
%payload = extractvalue %PyObject %obj, 1
%int_val = fptosi double %payload to i64
; Returns i64
```

### pyobject_to_float
```llvm
%payload = extractvalue %PyObject %obj, 1
; Returns double (no conversion needed)
```

## Design Trade-offs

### Advantages
✅ Simple implementation
✅ Fits in 16 bytes
✅ Cache-friendly
✅ Supports dynamic typing
✅ Type promotion is straightforward
✅ No heap allocation for primitives

### Disadvantages
❌ Integers limited to ±2^53 (double precision)
❌ Extra 8 bytes per value (vs native types)
❌ Runtime type checks required
❌ String pointers cast to double (non-portable)
❌ Wastes space for booleans

## Comparison with Other Systems

### CPython
- Uses `PyObject*` pointers (8 bytes)
- Full heap allocation
- Reference counting
- Supports arbitrary precision integers
- Much more complex

### Rusthon
- Uses value type (16 bytes)
- Stack allocation
- No garbage collection
- Limited integer range
- Much simpler

### Static Compilation (C)
- Native types (4-8 bytes)
- No type tags
- Zero overhead
- No dynamic typing
- Not applicable to Python

## Future Improvements

### Tagged Pointers
Instead of 16-byte structs, use 8-byte tagged pointers:
```
Pointer: [63-bit pointer | 1-bit tag]
Integer: [62-bit value | 2-bit tag]
```

Advantages:
- 50% size reduction
- Still cache-friendly
- Standard technique

Disadvantages:
- More complex implementation
- Platform-specific
- Limited integer range

### Specialized Types
Add optimized paths for common cases:
```rust
if statically_known_int(expr):
    generate_native_i64_code()
else:
    generate_pyobject_code()
```

Advantages:
- Better performance for known types
- No runtime overhead

Disadvantages:
- Requires type inference
- More complex code generation

## Next Steps

- [Memory Model](/architecture/memory-model) - Stack allocation and layout
- [Code Generation](/implementation/code-generation) - How types are generated
- [Limitations](/limitations) - Type system limitations
