# Memory Model

Rusthon uses a simple stack-based memory model with heap allocation for strings and lists.

## Memory Regions

```
┌─────────────────┐
│      Stack      │  Variables, parameters, temporaries
│  (auto-managed) │
├─────────────────┤
│      Heap       │  String literals and list data
│   (malloc'ed)   │  ⚠️ Never freed (memory leak)
└─────────────────┘
```

## Stack Allocation

### Variables

All variables are stack-allocated:

```python
x = 42
y = 3.14
```

Generated LLVM:
```llvm
define i32 @main() {
entry:
  %x = alloca %PyObject        ; Allocate on stack
  %y = alloca %PyObject        ; Allocate on stack

  %x_val = insertvalue %PyObject { i8 0, double undef }, double 42.0, 1
  store %PyObject %x_val, ptr %x

  %y_val = insertvalue %PyObject { i8 1, double undef }, double 3.14, 1
  store %PyObject %y_val, ptr %y

  ret i32 0
}
```

**Memory layout:**
```
Stack frame for main():
  [%y] <- %PyObject (16 bytes)
  [%x] <- %PyObject (16 bytes)
  [return address]
```

### Function Parameters

Parameters are passed by value, then stored on the stack:

```python
def add(a, b):
    return a + b
```

Generated LLVM:
```llvm
define %PyObject @add(%PyObject %a, %PyObject %b) {
entry:
  %a_ptr = alloca %PyObject     ; Allocate on stack
  %b_ptr = alloca %PyObject     ; Allocate on stack

  store %PyObject %a, ptr %a_ptr   ; Store parameter
  store %PyObject %b, ptr %b_ptr   ; Store parameter

  ; ... function body ...

  ret %PyObject %result
}
```

**Memory layout:**
```
Stack frame for add():
  [%b_ptr] <- %PyObject (16 bytes)
  [%a_ptr] <- %PyObject (16 bytes)
  [return address]
Arguments passed in registers or caller's stack frame
```

### Temporaries

Expression temporaries live on the stack:

```python
result = (a + b) * (c + d)
```

Generated LLVM:
```llvm
%temp1 = add %a, %b       ; Temporary in register/stack
%temp2 = add %c, %d       ; Temporary in register/stack
%result_val = mul %temp1, %temp2
store %PyObject %result_val, ptr %result
```

## Heap Allocation

### String Literals

Strings are heap-allocated with `malloc()`:

```python
print("Hello, World!")
```

Generated LLVM:
```llvm
@str_literal = private unnamed_addr constant [14 x i8] c"Hello, World!\00"

define i32 @main() {
entry:
  ; Allocate heap memory
  %str_ptr = call ptr @malloc(i64 14)

  ; Copy string data
  call void @memcpy(ptr %str_ptr, ptr @str_literal, i64 14, i1 false)

  ; Create PyObject with string pointer
  %ptr_int = ptrtoint ptr %str_ptr to i64
  %ptr_double = sitofp i64 %ptr_int to double
  %str_obj = insertvalue %PyObject { i8 3, double undef }, double %ptr_double, 1

  ; Use string...

  ; ⚠️ Memory is NEVER freed - this is a memory leak!

  ret i32 0
}
```

**Memory layout:**
```
Heap:
  [H] "Hello, World!\0" (14 bytes, malloc'ed)
       ↑
       |
Stack:
  [str_obj] <- PyObject { tag: 3, payload: ptr_to_H }
```

### Lists

Lists are heap-allocated with a **length header** followed by elements:

```python
my_list = [10, 20, 30]
print(len(my_list))
```

Generated LLVM:
```llvm
define i32 @main() {
entry:
  ; Allocate heap memory: (n+1) * 8 bytes
  ; Where n=3 elements, +1 for length header
  %list_ptr = call ptr @malloc(i64 32)

  ; Store length at offset 0
  store i64 3, ptr %list_ptr

  ; Store element 0 at offset 1
  %elem_ptr_0 = getelementptr i64, ptr %list_ptr, i64 1
  store i64 9221120237041090570, ptr %elem_ptr_0  ; NaN-boxed 10

  ; Store element 1 at offset 2
  %elem_ptr_1 = getelementptr i64, ptr %list_ptr, i64 2
  store i64 9221120237041090580, ptr %elem_ptr_1  ; NaN-boxed 20

  ; Store element 2 at offset 3
  %elem_ptr_2 = getelementptr i64, ptr %list_ptr, i64 3
  store i64 9221120237041090590, ptr %elem_ptr_2  ; NaN-boxed 30

  ; Create PyObject with list pointer
  %ptr_int = ptrtoint ptr %list_ptr to i64
  %list_obj = ... ; NaN-boxed list with TAG_LIST=3

  ; Reading length for len()
  %len_ptr = getelementptr i64, ptr %list_ptr, i64 0
  %length = load i64, ptr %len_ptr  ; Returns 3

  ; ⚠️ Memory is NEVER freed - this is a memory leak!

  ret i32 0
}
```

**Memory layout:**
```
Heap:
  Offset:  0        1        2        3
  Value:   3        10       20       30
           ^length  ^elem[0] ^elem[1] ^elem[2]
  Size: 32 bytes (4 × i64)
       ↑
       |
Stack:
  [list_obj] <- PyObject { tag: 4, payload: ptr_to_list }
```

**Key points:**
- **Allocation size**: `(n + 1) * sizeof(i64)` = `(n + 1) * 8` bytes
- **Offset 0**: List length (i64)
- **Offset 1..n**: List elements (NaN-boxed i64 values)
- **Indexing**: `list[i]` accesses offset `i + 1`
- **len() operation**: O(1) - reads i64 at offset 0
- **Memory overhead**: +8 bytes per list for length header

## Memory Lifecycle

### Variable Lifetime

```python
def compute():
    x = 10        # x allocated
    y = 20        # y allocated
    z = x + y     # z allocated, x and y read
    return z      # z returned, all deallocated
```

**Lifetime diagram:**
```
enter compute()
  ├─ alloca x
  ├─ alloca y
  ├─ alloca z
  ├─ compute...
  ├─ return z (copy value)
  └─ dealloca z, y, x (automatic)
```

### Function Call Memory

```python
def outer():
    x = 10
    y = inner(x)
    return y

def inner(a):
    b = a * 2
    return b
```

**Memory lifecycle:**
```
outer() frame:
  x = 10
  call inner(x):
    inner() frame:
      a = 10 (copy of x)
      b = 20
      return b (copy)
    [inner() frame destroyed]
  y = 20 (returned value)
  return y
[outer() frame destroyed]
```

## Stack Frame Layout

### Example Program

```python
def factorial(n):
    if n <= 1:
        return 1
    return n * factorial(n - 1)

result = factorial(5)
```

### Stack During Execution

```
Time: factorial(5)
┌──────────────────┐
│ n_ptr = 5        │ factorial(5)
├──────────────────┤
│ result_ptr       │ main()
└──────────────────┘

Time: factorial(4) called
┌──────────────────┐
│ n_ptr = 4        │ factorial(4)
├──────────────────┤
│ n_ptr = 5        │ factorial(5)
├──────────────────┤
│ result_ptr       │ main()
└──────────────────┘

Time: factorial(3) called
┌──────────────────┐
│ n_ptr = 3        │ factorial(3)
├──────────────────┤
│ n_ptr = 4        │ factorial(4)
├──────────────────┤
│ n_ptr = 5        │ factorial(5)
├──────────────────┤
│ result_ptr       │ main()
└──────────────────┘

... (continues to factorial(1))

Time: returning from factorial(1)
┌──────────────────┐
│ return 1         │
└──────────────────┘
(frames unwind, values multiplied)
```

## Memory Safety

### Guaranteed Safe

✅ **No dangling pointers** - Variables can't outlive their scope
✅ **No use-after-free** - Stack automatically deallocated
✅ **No double-free** - No manual deallocation
✅ **No buffer overflows** - No manual array indexing

### Not Safe

❌ **Memory leaks** - Strings never freed
❌ **Stack overflow** - Deep recursion can exhaust stack
❌ **Integer overflow** - No bounds checking on arithmetic

## Performance Characteristics

### Stack Allocation

**Advantages:**
- ⚡ Extremely fast (pointer increment)
- ⚡ Predictable performance
- ⚡ Cache-friendly (locality)
- ⚡ No fragmentation
- ⚡ Automatic deallocation

**Disadvantages:**
- 📏 Limited size (typically 1-8 MB)
- 📏 Can't return local references
- 📏 Large objects problematic

### Heap Allocation (Strings)

**Advantages:**
- 📦 Unlimited size (within RAM)
- 📦 Can outlive function

**Disadvantages:**
- 🐌 Slower than stack
- 🐌 Fragmentation possible
- 💧 Memory leaks (never freed)
- 🐛 More complex

## Comparison with Other Systems

### CPython
```
Heap:
  All PyObject* allocated on heap
  Reference counting for deallocation
  Garbage collector for cycles

Stack:
  Only C-level variables
  Pointers to heap objects
```

### Rusthon
```
Stack:
  All PyObject values (8 bytes each, NaN-boxed)
  Direct value storage

Heap:
  String data (variable length)
  List data (length header + elements)
  Never freed (leak)
```

### Native Code (C)
```
Stack:
  Local variables
  Function parameters

Heap:
  Explicitly malloc/free
  Programmer managed
```

## Design Decisions

### Why Stack-Only?

**Pros:**
- ✅ Simplifies implementation
- ✅ No GC needed
- ✅ Predictable performance
- ✅ No allocation overhead

**Cons:**
- ❌ Can't return heap objects
- ❌ Limited to short-lived programs
- ❌ Strings and lists leak memory

### Why Heap Strings and Lists?

**Pros:**
- ✅ Arbitrary length strings
- ✅ String literals work
- ✅ Dynamic-sized lists
- ✅ O(1) len() operation for lists

**Cons:**
- ❌ Memory leaks
- ❌ Inconsistent with primitive types (int, float, bool)

### Alternative: Arena Allocation

Future improvement:
```rust
// Allocate from arena
arena = Arena::new()
str_ptr = arena.alloc(string_data)

// Free entire arena at program end
arena.destroy()  // Frees all strings at once
```

## Memory Usage Examples

### Small Program
```python
x = 42
print(x)
```

**Memory:**
- Stack: 8 bytes (1 PyObject, NaN-boxed)
- Heap: 0 bytes
- Total: 8 bytes

### Medium Program
```python
def fib(n):
    if n <= 1:
        return n
    return fib(n-1) + fib(n-2)

print(fib(10))
```

**Memory:**
- Stack: ~176 bytes (11 frames × 16 bytes, approximate)
- Heap: 0 bytes
- Total: ~176 bytes (peak)

### String Program
```python
print("Hello")
print("World")
print("Rust")
```

**Memory:**
- Stack: ~24 bytes (3 PyObject temporaries, NaN-boxed)
- Heap: 18 bytes (3 strings: 6+6+5+nulls, leaked)
- Total: ~42 bytes

### List Program
```python
my_list = [10, 20, 30, 40, 50]
print(len(my_list))
```

**Memory:**
- Stack: 8 bytes (1 PyObject for list, NaN-boxed pointer)
- Heap: 48 bytes (1 length header + 5 elements × 8 bytes, leaked)
- Total: 56 bytes

## Debugging Memory Issues

### Stack Overflow

**Symptoms:**
```
Segmentation fault (core dumped)
```

**Cause:**
- Too deep recursion
- Too many local variables
- Very large temporaries

**Solution:**
```python
# Bad - deep recursion
def factorial(n):
    if n <= 1:
        return 1
    return n * factorial(n - 1)

factorial(100000)  # Stack overflow!

# Good - iterative
def factorial(n):
    result = 1
    i = 1
    while i <= n:
        result *= i
        i += 1
    return result

factorial(100000)  # Works fine
```

### Memory Leaks (Strings and Lists)

**Detection:**
```bash
valgrind ./program
# Will report leaked string and list allocations
```

**Mitigation:**
- Use strings and lists sparingly
- For long-running programs, minimize dynamic allocations
- Consider using fixed-size data where possible
- Or implement arena allocation (future work)

## Next Steps

- [Type System](/architecture/type-system) - Understanding PyObject
- [Code Generation](/implementation/code-generation) - How memory is managed
- [Limitations](/limitations) - Memory-related limitations
