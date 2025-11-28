//! Compiler Module
//!
//! This module contains the modular compiler infrastructure for the Rusthon Python-to-LLVM compiler.
//!
//! ## Architecture
//! - `runtime`: External C function declarations (printf, malloc, etc.)
//! - `values`: NaN-boxing type system for PyObject representation
//! - More modules to be added during refactoring...

pub mod runtime;
pub mod values;
