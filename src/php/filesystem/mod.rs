mod move_uploaded_file;
use crate::compiler::Parser;
use super::BuiltinResult;
pub(super) fn compile(name:&str,p:&mut Parser<'_>)->Option<BuiltinResult>{
    match name{"move_uploaded_file"=>Some(move_uploaded_file::compile(p)),_=>None}
}
