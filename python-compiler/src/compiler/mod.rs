//! Compiler Module
//!
//! This module contains the modular compiler infrastructure for the Rusthon Python-to-LLVM compiler.
//!
//! ## Architecture
//! - `runtime`: External C function declarations (printf, malloc, etc.)
//! - `values`: NaN-boxing type system for PyObject representation
//! - `generators`: Code generation modules (expression, statement, control flow)
//!
//! ## Refactoring Progress
//! See `/REFACTORING_PROGRESS.md` for detailed progress and next steps.

pub mod generators;
pub mod runtime;
pub mod values;
