mod htmlspecialchars;
use crate::compiler::Parser;
use super::BuiltinResult;
pub(super) fn compile(name:&str,p:&mut Parser<'_>)->Option<BuiltinResult>{
    match name {"htmlspecialchars"=>Some(htmlspecialchars::compile(p)),_=>None}
}
