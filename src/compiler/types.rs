use std::collections::HashMap;
use crate::lexer::Token;
#[derive(Clone,Copy,PartialEq,Eq,Debug)]pub(crate) enum Ty{Int,Bool,Str,Void,Json,Bytes}
impl Ty{pub(super) fn rust(self)->&'static str{match self{Self::Int=>"i64",Self::Bool=>"bool",Self::Str=>"String",Self::Void=>"()",Self::Json=>"Json",Self::Bytes=>"Vec<u8>"}}}
#[derive(Clone)]pub(super) struct FnSig{pub(super) name:String,pub(super) args:Vec<(String,Ty)>,pub(super) ret:Ty,pub(super) start:usize,pub(super) end:usize}
pub(crate) struct Parser<'a>{pub(super) ts:&'a [Token],pub(super) at:usize,pub(super) vars:HashMap<String,Ty>,pub(super) fns:Vec<FnSig>}

