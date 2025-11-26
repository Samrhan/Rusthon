# Architecture

Rusthon is a multi-stage compiler that transforms Python source code into native executables through LLVM.

## High-Level Overview

```
┌─────────────┐
│   Python    │
│   Source    │
└──────┬──────┘
       │
       ▼
┌─────────────┐     rustpython-parser
│  Parser     │◄───────────────────────
└──────┬──────┘
       │ Python AST
       ▼
┌─────────────┐
│  Lowering   │     Custom IR
└──────┬──────┘
       │ IR
       ▼
┌─────────────┐     inkwell (LLVM bindings)
│  Code Gen   │◄───────────────────────
└──────┬──────┘
       │ LLVM IR
       ▼
┌─────────────┐
│  LLVM IR    │     .ll file
│    File     │
└──────┬──────┘
       │
       ▼
┌─────────────┐     clang
│   Clang     │◄───────────
│  Compiler   │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│   Native    │
│ Executable  │
└─────────────┘
```

## Components

### 1. Parser (`parser.rs`)
- Wraps `rustpython-parser` v0.4
- Converts Python source → Python AST
- Minimal implementation (6 lines)
- Handles syntax validation

### 2. Lowering (`lowering.rs`)
- Converts Python AST → Custom IR
- Simplifies Python constructs
- Desugars augmented assignments (`x += 5` → `x = x + 5`)
- Type-agnostic representation
- ~253 lines of Rust

### 3. Code Generation (`codegen.rs`)
- Converts IR → LLVM IR
- Implements type system (PyObject)
- Manages stack allocation
- Generates external C function calls
- Most complex module (~927 lines)

### 4. Main Driver (`main.rs`)
- CLI entry point
- Orchestrates compilation pipeline
- Invokes `clang` for final compilation
- Error reporting with `ariadne`
- ~99 lines

### 5. Error Handling (`error.rs`)
- Beautiful error messages with `ariadne`
- Line/column tracking
- Three error types: Parse, Lowering, CodeGen
- ~65 lines

## Design Principles

### 1. Simplicity
- Each stage has a single, clear responsibility
- Minimal abstractions
- Direct mapping from Python to LLVM where possible

### 2. Correctness
- Comprehensive test suite (735+ lines of tests)
- Snapshot testing for LLVM IR verification
- Error handling at every stage

### 3. Performance
- Direct compilation to native code
- No interpreter overhead
- LLVM's world-class optimizations
- Static typing where possible

### 4. Extensibility
- Modular architecture
- Clear IR representation
- Easy to add new operators/features
- Well-defined interfaces between stages

## Key Technical Decisions

### Tagged Union Type System
Instead of static typing, Rusthon uses a tagged union (`PyObject`) that carries type information at runtime.

**Why?**
- Supports Python's dynamic typing
- Allows type promotion (int → float)
- Simple implementation
- Efficient (fits in 16 bytes)

**Trade-offs:**
- Slightly larger than native types
- Runtime type checks
- But: LLVM can optimize away many checks

### Stack-Only Allocation
All variables and function parameters live on the stack (except strings).

**Why?**
- Simplifies memory management
- No garbage collector needed
- Fast allocation/deallocation
- Predictable memory usage

**Trade-offs:**
- Strings leak memory (malloc'd, never freed)
- Can't return heap-allocated objects
- But: Suitable for short-running programs

### Direct LLVM IR Generation
Skip intermediate LLVM passes and generate IR directly.

**Why?**
- Full control over generated code
- Easier to debug
- No complex optimization pipeline
- Direct mapping to Python semantics

**Trade-offs:**
- More verbose code generation
- Manual SSA form management
- But: Simpler to understand and modify

## Data Flow

### Variable Access
```
Python: x = 10
   ↓
IR: Assign { target: "x", value: Constant(10) }
   ↓
LLVM:
  %x = alloca %PyObject        ; Stack allocation
  %val = create_int(10)        ; Create PyObject
  store %val, ptr %x           ; Store to stack
```

### Function Call
```
Python: result = add(5, 3)
   ↓
IR: Assign {
      target: "result",
      value: Call { func: "add", args: [Constant(5), Constant(3)] }
    }
   ↓
LLVM:
  %arg1 = create_int(5)
  %arg2 = create_int(3)
  %ret = call @add(%arg1, %arg2)
  store %ret, ptr %result
```

### Control Flow
```
Python: if x > 5: print(x)
   ↓
IR: If {
      condition: Comparison { op: Gt, left: Variable("x"), right: Constant(5) },
      then_body: [Print([Variable("x")])],
      else_body: []
    }
   ↓
LLVM:
  %cond = fcmp ogt %x_val, 5.0
  br i1 %cond, label %then, label %else
then:
  call @print_int(%x_val)
  br label %merge
else:
  br label %merge
merge:
  ; continue...
```

## Module Structure

```
python-compiler/
├── src/
│   ├── main.rs           # CLI & orchestration
│   ├── lib.rs            # Library exports
│   ├── parser.rs         # Parser wrapper
│   ├── ast.rs            # IR definitions
│   ├── lowering.rs       # AST → IR
│   ├── codegen.rs        # IR → LLVM
│   └── error.rs          # Error handling
├── tests/                # Test suite
│   ├── lib.rs
│   ├── arithmetic.rs
│   ├── variables.rs
│   ├── functions.rs
│   ├── floats.rs
│   ├── control_flow.rs
│   ├── bitwise.rs
│   ├── unary.rs
│   ├── augmented_assignment.rs
│   ├── precedence.rs
│   ├── edge_cases.rs
│   ├── strings.rs
│   ├── complex_control_flow.rs
│   ├── input.rs
│   ├── errors.rs
│   └── integration.rs
└── examples/             # Example programs
```

## Next Steps

- [Compilation Pipeline](/architecture/compilation-pipeline) - Detailed pipeline walkthrough
- [Type System](/architecture/type-system) - PyObject and type promotion
- [Memory Model](/architecture/memory-model) - Stack allocation and memory layout
