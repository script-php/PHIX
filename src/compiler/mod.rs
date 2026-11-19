mod ast;
mod flow;
mod ir;
mod types;
mod parser;
mod codegen;
pub(crate) use types::{Ty,Parser};
pub use codegen::compile;
