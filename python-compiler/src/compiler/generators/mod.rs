//! Code Generators Module
//!
//! This module contains specialized code generation functions for different aspects
//! of Python-to-LLVM compilation.
//!
//! ## Architecture
//! The generators module breaks down the monolithic code generation into focused,
//! maintainable modules:
//! - `expression`: Expression compilation (binary ops, calls, literals, etc.)
//! - `statement`: Statement compilation (to be added)
//! - `control`: Control flow compilation (to be added)

pub mod expression;
