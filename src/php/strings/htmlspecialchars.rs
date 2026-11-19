use crate::compiler::{Parser,Ty};
use super::super::BuiltinResult;
pub(super) fn compile(p:&mut Parser<'_>)->BuiltinResult {
    let(s,t)=p.expr(0)?;
    p.need(")")?;
    if t==Ty::Void || t==Ty::Bytes{return Err("htmlspecialchars requires a scalar value".into());}
    Ok((format!("phix_escape_html(&({s}))"),Ty::Str))
}
