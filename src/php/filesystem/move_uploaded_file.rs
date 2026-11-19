use crate::compiler::{Parser,Ty};
use super::super::BuiltinResult;
pub(super) fn compile(p:&mut Parser<'_>)->BuiltinResult {
    let (a,at)=p.expr(0)?;
    if at!=Ty::Json && at!=Ty::Str{return Err("move_uploaded_file expects upload token".into())}
    p.need(",")?;
    let (b,bt)=p.expr(0)?;
    p.need(")")?;
    if bt!=Ty::Str && bt!=Ty::Json{return Err("move_uploaded_file destination must be string-compatible".into())}
    let first=if at==Ty::Str{format!("Json::Text({a})")}else{a};
    Ok((format!("{{ let phix_upload_dest = phix_string(&({b})); ctx.move_uploaded_file(&({first}), &phix_upload_dest) }}"),Ty::Bool))
}
