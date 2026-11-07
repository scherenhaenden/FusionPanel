#![forbid(unsafe_code)]

pub mod domain;
pub mod provider;
pub mod backend;
pub mod zone_file;
pub mod validation;

#[cfg(test)]
mod tests;
