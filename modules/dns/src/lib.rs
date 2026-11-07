#![forbid(unsafe_code)]

pub mod domain;
pub mod provider;
pub mod backend;
pub mod zone_file;

#[cfg(test)]
mod tests;
