# Compilation Pipeline

This document provides a detailed walkthrough of Rusthon's compilation stages.

## Stage 1: Parsing

**Input:** Python source code (`.py` file)
**Output:** Python AST (from rustpython-parser)
**Module:** `parser.rs`

### Process

1. Read Python source file
2. Pass to `rustpython_parser::parse_program()`
3. Return Python AST or parse error

### Example

```python
print(1 + 2)
```

Becomes (simplified AST):

```
Module {
  body: [
    Expr {
      value: Call {
        func: Name("print"),
        args: [
          BinOp {
            left: Constant(1),
            op: Add,
            right: Constant(2)
          }
        ]
      }
    }
  ]
}
```

### Error Handling

Parse errors include:
- Syntax errors
- Invalid tokens
- Malformed expressions

Example error:
```
Error: Parse error
   ╭─[test.py:1:5]
   │
 1 │ def (invalid
   │     ┬
   │     ╰── expected identifier
───╯
```

## Stage 2: Lowering

**Input:** Python AST
**Output:** Custom IR (Intermediate Representation)
**Module:** `lowering.rs`

### Process

1. Walk Python AST
2. Convert each statement/expression to IR equivalent
3. Desugar complex constructs
4. Validate supported features

### IR Structures

**Statements:**
```rust
pub enum IRStmt {
    Print(Vec<IRExpr>),
    Assign { target: String, value: IRExpr },
    FunctionDef { name: String, params: Vec<String>, body: Vec<IRStmt> },
    Return(IRExpr),
    If { condition: IRExpr, then_body: Vec<IRStmt>, else_body: Vec<IRStmt> },
    While { condition: IRExpr, body: Vec<IRStmt> },
}
```

**Expressions:**
```rust
pub enum IRExpr {
    Constant(i64),
    Float(f64),
    StringLiteral(String),
    Variable(String),
    BinaryOp { op: BinaryOperator, left: Box<IRExpr>, right: Box<IRExpr> },
    UnaryOp { op: UnaryOperator, operand: Box<IRExpr> },
    Comparison { op: ComparisonOperator, left: Box<IRExpr>, right: Box<IRExpr> },
    Call { func: String, args: Vec<IRExpr> },
    Input,
}
```

### Transformations

**Augmented Assignment Desugaring:**
```python
x += 5
```
↓
```rust
Assign {
  target: "x",
  value: BinaryOp {
    op: Add,
    left: Variable("x"),
    right: Constant(5)
  }
}
```

**Print Function Extraction:**
```python
print(42)
```
↓
```rust
Print([Constant(42)])
```

### Error Handling

Lowering errors include:
- Unsupported statements (e.g., `for`, `class`)
- Unsupported expressions (e.g., list literals, dict literals)
- Unsupported operators
- Invalid comparison chains (e.g., `a < b < c`)

Example error:
```
Error: Unsupported statement
   ╭─[test.py:1:1]
   │
 1 │ for i in range(10):
   │ ┬
   │ ╰── 'for' loops are not yet supported
───╯
```

## Stage 3: Code Generation

**Input:** IR
**Output:** LLVM IR (textual representation)
**Module:** `codegen.rs`

### Process

1. Create LLVM context and module
2. Define PyObject struct type
3. Declare external C functions
4. Generate function declarations
5. Generate function bodies
6. Generate top-level code in `main()`
7. Verify and return LLVM IR string

### LLVM Setup

**PyObject Structure:**
```llvm
%PyObject = type { i8, double }
; Field 0: tag (i8) - type discriminator
; Field 1: payload (double) - value storage
```

**External Functions:**
```llvm
declare i32 @printf(ptr, ...)
declare i32 @scanf(ptr, ...)
declare ptr @malloc(i64)
declare void @memcpy(ptr, ptr, i64, i1)
```

### Type Tags

```rust
const TYPE_TAG_INT: i8 = 0
const TYPE_TAG_FLOAT: i8 = 1
const TYPE_TAG_BOOL: i8 = 2
const TYPE_TAG_STRING: i8 = 3
```

### Code Generation Examples

**Variable Assignment:**
```python
x = 42
```
↓
```llvm
%x = alloca %PyObject
%val = insertvalue %PyObject { i8 0, double undef }, double 42.0, 1
store %PyObject %val, ptr %x
```

**Function Definition:**
```python
def add(a, b):
    return a + b
```
↓
```llvm
define %PyObject @add(%PyObject %a, %PyObject %b) {
entry:
  %a_ptr = alloca %PyObject
  %b_ptr = alloca %PyObject
  store %PyObject %a, ptr %a_ptr
  store %PyObject %b, ptr %b_ptr

  %a_val = load %PyObject, ptr %a_ptr
  %b_val = load %PyObject, ptr %b_ptr

  ; Extract tags and payloads
  %a_tag = extractvalue %PyObject %a_val, 0
  %a_payload = extractvalue %PyObject %a_val, 1
  %b_tag = extractvalue %PyObject %b_val, 0
  %b_payload = extractvalue %PyObject %b_val, 1

  ; Type promotion logic...
  ; Addition logic...

  ret %PyObject %result
}
```

**If Statement:**
```python
if x > 5:
    print(x)
else:
    print(0)
```
↓
```llvm
; Load and compare
%x_val = load %PyObject, ptr %x
%x_payload = extractvalue %PyObject %x_val, 1
%cond_float = fcmp ogt double %x_payload, 5.0
%cond = uitofp i1 %cond_float to double
%cond_bool = fcmp one double %cond, 0.0
br i1 %cond_bool, label %then, label %else

then:
  ; print(x)
  br label %merge

else:
  ; print(0)
  br label %merge

merge:
  ; continue...
```

**While Loop:**
```python
while i < 10:
    i += 1
```
↓
```llvm
br label %loop_cond

loop_cond:
  %i_val = load %PyObject, ptr %i
  %i_payload = extractvalue %PyObject %i_val, 1
  %cond = fcmp olt double %i_payload, 10.0
  br i1 %cond, label %loop_body, label %loop_exit

loop_body:
  ; i += 1
  br label %loop_cond

loop_exit:
  ; continue...
```

### Error Handling

Code generation errors include:
- Undefined variables
- Undefined functions
- Type system violations
- LLVM verification failures

Example error:
```
Error: Undefined variable
   ╭─[test.py:1:7]
   │
 1 │ print(undefined_var)
   │       ┬────────────
   │       ╰── variable 'undefined_var' is not defined
───╯
```

## Stage 4: LLVM IR File Output

**Input:** LLVM IR string
**Output:** `.ll` file
**Module:** `main.rs`

### Process

1. Generate output filename (`input.py` → `input.ll`)
2. Write LLVM IR to file
3. Handle file system errors

### Example Output

`hello.py` → `hello.ll`:

```llvm
; ModuleID = 'python_module'
source_filename = "python_module"

%PyObject = type { i8, double }

declare i32 @printf(ptr, ...)

define i32 @main() {
entry:
  ; Create string "Hello, World!"
  %str_ptr = call ptr @malloc(i64 14)
  call void @memcpy(ptr %str_ptr, ptr @str_literal, i64 14, i1 false)

  ; Create PyObject with string
  %obj = insertvalue %PyObject { i8 3, double undef }, double ptrtoint(ptr %str_ptr to double), 1

  ; Print
  call i32 (ptr, ...) @printf(ptr @fmt_str, ptr %str_ptr)

  ret i32 0
}
```

## Stage 5: Native Compilation

**Input:** `.ll` file
**Output:** Native executable
**Tool:** `clang`

### Process

1. Invoke `clang` as subprocess
2. Pass `.ll` file as input
3. Specify output executable name
4. Wait for compilation
5. Handle clang errors

### Command

```bash
clang-18 -o hello hello.ll
```

### Output

Native executable for the target platform:
- Linux: ELF binary
- macOS: Mach-O binary
- Windows: PE binary (via WSL)

## Complete Example

### Input: `fibonacci.py`

```python
def fib(n):
    if n <= 1:
        return n
    return fib(n - 1) + fib(n - 2)

print(fib(10))
```

### Step 1: Parse → Python AST

```
Module {
  body: [
    FunctionDef {
      name: "fib",
      args: ["n"],
      body: [
        If {
          test: Compare { left: Name("n"), ops: [LessEq], comparators: [Num(1)] },
          body: [Return(Name("n"))],
          orelse: [
            Return(
              BinOp {
                left: Call { func: Name("fib"), args: [BinOp { ... }] },
                op: Add,
                right: Call { func: Name("fib"), args: [BinOp { ... }] }
              }
            )
          ]
        }
      ]
    },
    Expr {
      value: Call { func: Name("print"), args: [Call { func: Name("fib"), args: [Num(10)] }] }
    }
  ]
}
```

### Step 2: Lower → IR

```rust
IRProgram {
  functions: [
    FunctionDef {
      name: "fib",
      params: ["n"],
      body: [
        If {
          condition: Comparison { op: LessEq, left: Variable("n"), right: Constant(1) },
          then_body: [Return(Variable("n"))],
          else_body: [
            Return(
              BinaryOp {
                op: Add,
                left: Call { func: "fib", args: [BinaryOp { op: Sub, left: Variable("n"), right: Constant(1) }] },
                right: Call { func: "fib", args: [BinaryOp { op: Sub, left: Variable("n"), right: Constant(2) }] }
              }
            )
          ]
        }
      ]
    }
  ],
  statements: [
    Print([Call { func: "fib", args: [Constant(10)] }])
  ]
}
```

### Step 3: Generate → LLVM IR (simplified)

```llvm
define %PyObject @fib(%PyObject %n) {
entry:
  %n_ptr = alloca %PyObject
  store %PyObject %n, ptr %n_ptr
  ; ... condition check ...
  br i1 %cond, label %then, label %else

then:
  %n_val = load %PyObject, ptr %n_ptr
  ret %PyObject %n_val

else:
  ; ... recursive calls ...
  ret %PyObject %result
}

define i32 @main() {
entry:
  %arg = insertvalue %PyObject { i8 0, double undef }, double 10.0, 1
  %result = call %PyObject @fib(%PyObject %arg)
  ; ... print result ...
  ret i32 0
}
```

### Step 4: Write → `fibonacci.ll`

File written to disk.

### Step 5: Compile → `fibonacci` executable

```bash
clang-18 -o fibonacci fibonacci.ll
```

### Run

```bash
./fibonacci
55
```

## Performance Characteristics

| Stage | Time (relative) | Complexity |
|-------|----------------|------------|
| Parsing | Fast | O(n) |
| Lowering | Fast | O(n) |
| Code Generation | Moderate | O(n) with some O(n²) in type checking |
| LLVM Compilation | Slow | O(n log n) with optimizations |

## Next Steps

- [Type System](/architecture/type-system) - Understanding PyObject
- [Memory Model](/architecture/memory-model) - Stack allocation details
- [Implementation](/implementation) - Deep dive into each module
