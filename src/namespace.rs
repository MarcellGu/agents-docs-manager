pub mod config;
mod service;
pub mod types;

pub use service::{create, delete, list, list_docs, rename};
