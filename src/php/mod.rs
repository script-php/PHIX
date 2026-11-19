//! PHP-compatible built-ins are compiled here, independently of the parser.
mod registry;
mod value;
mod strings;
mod assets;
mod filesystem;
use crate::compiler::{Parser,Ty};
pub(crate) type BuiltinResult = Result<(String,Ty),String>;
pub(crate) fn compile_builtin(name:&str, parser:&mut Parser<'_>)->Option<BuiltinResult>{
    let entry = registry::lookup(name)?;
    let compiled = match entry.category {
        registry::Category::Strings => strings::compile(name, parser),
        registry::Category::Assets => assets::compile(name, parser),
        registry::Category::Filesystem => filesystem::compile(name, parser),
    };
    Some(compiled.unwrap_or_else(|| Err(format!("internal compiler error: missing implementation for {name}"))))
}
