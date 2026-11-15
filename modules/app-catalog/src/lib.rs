#![forbid(unsafe_code)]

pub mod installer;
pub mod scanner;
pub mod schema;
pub mod dependency_solver;
pub use dependency_solver::*;
