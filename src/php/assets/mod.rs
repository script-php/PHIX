use crate::compiler::{Parser,Ty};
use super::BuiltinResult;
pub(super) fn compile(name:&str,p:&mut Parser<'_>)->Option<BuiltinResult>{
    if !["asset_text","asset_exists","asset_size","asset_bytes","asset_write"].contains(&name){return None;}
    Some((||{
        let(s,t)=p.expr(0)?;
        p.need(")")?;
        if t!=Ty::Str{return Err(format!("{name} expects string"));}
        let ty=match name{"asset_text"=>Ty::Str,"asset_exists"=>Ty::Bool,"asset_bytes"=>Ty::Bytes,"asset_write"=>Ty::Void,_=>Ty::Int};
        Ok((format!("ctx.{name}(&({s}))"),ty))
    })())
}
