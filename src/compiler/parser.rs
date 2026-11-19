use super::{Parser,Ty};
use super::types::FnSig;
use crate::lexer::{Kind,Token};
impl<'a> Parser<'a>{
 pub(super) fn cur(&self)->&Token{&self.ts[self.at]}
 pub(super) fn is(&self,s:&str)->bool{self.cur().text==s}
 pub(super) fn take(&mut self,s:&str)->bool{if self.is(s){self.at+=1;true}else{false}}
 pub(crate) fn need(&mut self,s:&str)->Result<(),String>{if self.take(s){Ok(())}else{Err(format!("line {}: expected '{s}', got '{}'",self.cur().line,self.cur().text))}}
 pub(super) fn name(&mut self,k:Kind)->Result<String,String>{if self.cur().kind!=k{return Err(format!("line {}: unexpected '{}'",self.cur().line,self.cur().text));}let n=self.cur().text.clone();self.at+=1;Ok(n)}
 pub(super) fn ty(&mut self)->Result<Ty,String>{match self.name(Kind::Id)?.as_str(){"int"=>Ok(Ty::Int),"bool"=>Ok(Ty::Bool),"string"=>Ok(Ty::Str),"void"=>Ok(Ty::Void),x=>Err(format!("unknown type: {x}"))}}
 pub(super) fn signatures(&mut self)->Result<(),String>{while self.cur().kind!=Kind::End{if !self.take("function"){self.at+=1;continue;}let name=self.name(Kind::Id)?;self.need("(")?;let mut args=Vec::new();if !self.is(")"){loop{let ty=self.ty()?;let arg=self.name(Kind::Var)?;args.push((arg,ty));if !self.take(","){break;}}}self.need(")")?;self.need(":")?;let ret=self.ty()?;let start=self.at;self.need("{")?;let mut d=1;while d>0{if self.cur().kind==Kind::End{return Err("unterminated function".into());}if self.take("{"){d+=1;}else if self.take("}"){d-=1;}else{self.at+=1;}}let end=self.at;if self.fns.iter().any(|f|f.name==name){return Err(format!("duplicate function {name}"));}self.fns.push(FnSig{name,args,ret,start,end});}Ok(())}
 pub(super) fn as_json_expr(expr:String,ty:Ty)->Result<String,String>{
  match ty {
   Ty::Int=>Ok(format!("Json::Int({expr})")),
   Ty::Bool=>Ok(format!("Json::Bool({expr})")),
   Ty::Str=>Ok(format!("Json::Text({expr})")),
   Ty::Json=>Ok(expr),
   _=>Err("array elements must be scalar or array values".into()),
  }
 }
 pub(super) fn array_literal(&mut self)->Result<(String,Ty),String>{
  let mut entries=Vec::new();
  while !self.is("]") {
   // Parse an optional key before an array value. An array element may
   // directly alias an existing PHP array element with the & operator.
   let (first,ty)=if self.is("&") { (String::new(),Ty::Void) } else { self.expr(0)? };
   let key=if ty!=Ty::Void && self.take("=>") {
    format!("Some({})",Self::as_json_expr(first.clone(),ty)?)
   } else {"None".to_string()};
   let cell=if self.take("&") {
    let target=self.name(Kind::Var)?;
    if self.vars.get(&target)!=Some(&Ty::Json){return Err("array literal reference target must be promoted before parsing".into());}
    let mut keys=Vec::new();
    while self.take("["){let key=self.array_key()?;self.need("]")?;keys.push(key);}
    format!("v_{target}.bind_element(&[{}]).reference_cell().expect(\"invalid array reference path\")",keys.join(", "))
   } else {
    let (value,value_ty)=if ty==Ty::Void || key!="None" {self.expr(0)?}else{(first,ty)};
    format!("PhpCell::new({})",Self::as_json_expr(value,value_ty)?)
   };
   entries.push((key,cell));
   if !self.take(","){break;}if self.is("]"){break;}
  }
  self.need("]")?;
  // Evaluate array keys and values in PHP source order. A thrown key must
  // suppress its value; a thrown value must suppress all subsequent entries.
  let items=(0..entries.len()).map(|i|format!("phix_array_entry_{i}")).collect::<Vec<_>>().join(", ");
  let mut build=format!("Json::PhpArray(PhpArray::from_cell_parts(vec![{items}]))");
  for (i,(key,cell)) in entries.into_iter().enumerate().rev() {
   build=format!("{{ let phix_array_key_{i}: Option<Json> = {key}; if phix_has_runtime_error() {{ Json::Null }} else {{ let phix_array_cell_{i} = {cell}; if phix_has_runtime_error() {{ Json::Null }} else {{ let phix_array_entry_{i} = (phix_array_key_{i}, phix_array_cell_{i}); {build} }} }} }}");
  }
  Ok((build,Ty::Json))
 }
 pub(super) fn array_key(&mut self)->Result<String,String>{
  let (expr,ty)=self.expr(0)?;
  match ty {Ty::Int=>Ok(format!("Json::Int({expr})")),Ty::Str=>Ok(format!("Json::Text({expr})")),Ty::Bool=>Ok(format!("Json::Bool({expr})")),Ty::Json=>Ok(expr),_=>Err("unsupported array key expression".into())}
 }
 pub(super) fn primary(&mut self)->Result<(String,Ty),String>{
 if self.take("throw") {
  if self.take("new") {
   let class=self.name(Kind::Id)?;
   if !matches!(class.as_str(),"Exception"|"Error"|"TypeError"|"ValueError"|"ParseError"|"RuntimeException"|"LogicException"|"InvalidArgumentException"|"DomainException"|"LengthException"|"OutOfRangeException"|"BadFunctionCallException"|"BadMethodCallException"|"OutOfBoundsException"|"OverflowException"|"UnderflowException"|"RangeException"|"UnexpectedValueException") {return Err(format!("unsupported thrown exception class {class}"));}
   let has_arguments = self.take("(");
   let (message,code,previous)=if !has_arguments || self.is(")") {("String::new()".to_string(),"0i64".to_string(),"Json::Null".to_string())} else {
    let (message,ty)=self.expr(0)?;
    if ty!=Ty::Str {return Err("exception constructor currently requires a string message".into());}
    let code=if self.take(",") {let (code,t)=self.expr(0)?;if t!=Ty::Int{return Err("exception constructor code must be an integer".into());}code}else{"0i64".into()};
    let previous=if self.take(",") {let (previous,t)=self.expr(0)?;if t!=Ty::Json{return Err("exception constructor previous must be Throwable or null".into());}previous}else{"Json::Null".into()};
    (message,code,previous)
   };
   if has_arguments { self.need(")")?; }
   return Ok((format!("{{ let phix_throw_message = {message}; if phix_has_runtime_error() {{ Json::Null }} else {{ let phix_throw_code = {code}; if phix_has_runtime_error() {{ Json::Null }} else {{ let phix_throw_previous = {previous}; if phix_has_runtime_error() {{ Json::Null }} else {{ phix_throw_builtin_with_previous(\"{class}\", phix_throw_message, phix_throw_code, phix_throw_previous); Json::Null }} }} }} }}"),Ty::Json));
  }
  let (value,ty)=self.primary()?;
  if ty!=Ty::Json{return Err("throw expects a Throwable object".into());}
  return Ok((format!("{{ let phix_throw_operand = {value}; if !phix_has_runtime_error() {{ phix_throw_value(phix_throw_operand); }} Json::Null }}"),Ty::Json));
 }
 if self.take("["){return self.array_literal();}
 if self.take("null"){return Ok(("Json::Null".into(),Ty::Json));}
 if self.take("("){let(s,t)=self.expr(0)?;self.need(")")?;return Ok((format!("({s})"),t));}
 if self.take("!"){let(s,t)=self.primary()?;let truthy=Self::php_truthy_expr(s,t)?;return Ok((format!("!({truthy})"),Ty::Bool));}
 if self.take("-"){let(s,t)=self.primary()?;if t!=Ty::Int{return Err("- requires int".into());}return Ok((format!("-({s})"),t));}
 if self.cur().kind==Kind::Num{return Ok((format!("{}i64",self.name(Kind::Num)?),Ty::Int));}
 if self.cur().kind==Kind::Str{return Ok((format!("{}.to_string()",self.name(Kind::Str)?),Ty::Str));}
 if self.cur().kind==Kind::Var{let n=self.name(Kind::Var)?;
 if n=="_PHIX" || n=="_GET" || n=="_SERVER" || n=="_FILES"{
    let mut keys=Vec::new();
    while self.take("[") {
      let text=self.name(Kind::Str)?;
      self.need("]")?;
      // Literal keys remain quoted as valid Rust string literals.
      keys.push(text);
    }
    if keys.is_empty(){return Err(format!("${n} requires at least one quoted key"));}
    let reader=match n.as_str(){"_PHIX"=>"cfg_value","_GET"=>"get_value","_SERVER"=>"server_value","_FILES"=>"files_value",_=>unreachable!()};
    return Ok((format!("ctx.{reader}(&[{}])",keys.join(", ")),Ty::Json));
 }

 // PHP null coalescing suppresses the undefined-variable read for a
 // simple left-hand operand. Do not make ordinary variable reads silent.
 if !self.vars.contains_key(&n) {
  if self.is("??") { return Ok(("Json::Null".into(),Ty::Json)); }
  // PHP's isset-style coalescing read also permits an undefined array
  // root. Evaluate each index once, in order, without reading the root.
  // An ordinary undefined-variable read remains a compile-time error.
  if self.is("[") {
   let mut keys=Vec::new();
   while self.take("[") {
    let key=self.array_key()?;
    self.need("]")?;
    keys.push(key);
   }
   if !self.is("??") { return Err(format!("undefined variable ${n}")); }
   let mut read="Json::Null".to_string();
   for (i,key) in keys.into_iter().enumerate().rev() {
    read=format!("{{ let phix_missing_key_{i} = {key}; if phix_has_runtime_error() {{ Json::Null }} else {{ {read} }} }}");
   }
   return Ok((read,Ty::Json));
  }
  return Err(format!("undefined variable ${n}"));
 }
 let ty=*self.vars.get(&n).ok_or_else(||format!("undefined variable ${n}"))?;
 let mut result=if ty==Ty::Json{format!("v_{n}.read().clone()")}else if ty==Ty::Str || ty==Ty::Bytes{format!("v_{n}.clone()")}else{format!("v_{n}")};
 let mut result_ty=ty;
 while self.take("[") {
   if result_ty!=Ty::Json{return Err(format!("cannot index variable ${n}: not an array-compatible value"));}
   let key=self.array_key()?;self.need("]")?;
   result=format!("{{ let phix_index_base = {result}; if phix_has_runtime_error() {{ Json::Null }} else {{ let phix_index_key = {key}; if phix_has_runtime_error() {{ Json::Null }} else {{ phix_index_base.phix_index(&phix_index_key) }} }} }}");result_ty=Ty::Json;
 }
 while self.take("->") {
   if result_ty!=Ty::Json {return Err("method calls require an object-compatible value".into());}
   let method=self.name(Kind::Id)?;
   self.need("(")?;self.need(")")?;
   match method.as_str() {
    "getMessage"=>{result=format!("phix_exception_method_string(&{result}, \"message\")");result_ty=Ty::Str;},
    "getCode"=>{result=format!("phix_exception_method_int(&{result}, \"code\")");result_ty=Ty::Int;},
    "getPrevious"=>{result=format!("phix_exception_method_previous(&{result})");result_ty=Ty::Json;},
    "getFile"|"getTraceAsString"=>return Err(format!("{method} requires source-location/stack-trace support, not yet implemented")),
    _=>return Err(format!("unsupported exception method {method}")),
   }
 }
 if result_ty==Ty::Json {return Ok((result,result_ty));}

 return Ok((result,result_ty));}
 if self.take("true"){return Ok(("true".into(),Ty::Bool));}if self.take("false"){return Ok(("false".into(),Ty::Bool));}
 if self.cur().kind==Kind::Id{let n=self.name(Kind::Id)?;self.need("(")?;
 if let Some(result)=crate::php::compile_builtin(&n,self){return result;}
 let sig=self.fns.iter().find(|f|f.name==n).cloned().ok_or_else(||format!("unknown function {n}"))?;let mut args=Vec::new();if !self.is(")"){loop{let(s,t)=self.expr(0)?;if sig.args.get(args.len()).map(|a|a.1)!=Some(t){return Err(format!("argument type mismatch for {n}"));}args.push(s);if !self.take(","){break;}}}self.need(")")?;if args.len()!=sig.args.len(){return Err(format!("wrong argument count for {n}"));}// PHP evaluates arguments left-to-right and does not invoke the callee
 // when an argument throws. The pending runtime exception is propagated
 // by the caller's statement guard, rather than running the callee.
 // Evaluate call arguments in source order. A PHP exception raised by an
 // earlier argument must prevent evaluation of all later arguments, not just
 // prevent the eventual function invocation.
 // Every call, including a zero- or single-argument call, must observe
 // the pending exception before invoking the callee. For an argument-bearing
 // call, evaluate each argument exactly once and in source order.
 let invocation=if args.is_empty() {
  let call=format!("fn_{n}(ctx)");
  let call=if sig.ret==Ty::Void {call} else {format!("phix_unwrap_function_outcome({call})")};
  if sig.ret==Ty::Void {
   format!("{{ if !phix_has_runtime_error() {{ {call}; }} }}")
  } else {
   format!("{{ if phix_has_runtime_error() {{ Default::default() }} else {{ {call} }} }}")
  }
 } else {
  let raw_call=format!("fn_{n}(ctx, {})",(0..args.len()).map(|i|format!("phix_arg_{i}")).collect::<Vec<_>>().join(", "));
  let call=if sig.ret==Ty::Void {raw_call} else {format!("phix_unwrap_function_outcome({raw_call})")};
  let mut next=format!("Some({call})");
  for (i,arg) in args.iter().enumerate().rev() {
   next=format!("{{ let phix_arg_{i} = {arg}; if phix_has_runtime_error() {{ None }} else {{ {next} }} }}");
  }
  if sig.ret==Ty::Void {
   format!("{{ if !phix_has_runtime_error() {{ let _ = {next}; }} }}")
  } else {
   format!("{{ if phix_has_runtime_error() {{ Default::default() }} else {{ {next}.unwrap_or_default() }} }}")
  }
 };
 return Ok((invocation,sig.ret));}
 Err(format!("line {}: expected expression, got {}",self.cur().line,self.cur().text))
 }
 /// Lower PHP boolean conversion using the statically known operand type.
 /// Dynamic values retain the runtime truthiness semantics; no interpreter
 /// dispatch is needed for statically typed scalar operands.
 /// In a typed Result-returning function, a complete zero-argument
 /// function-call argument can propagate its exception with `?` directly.
 /// Other expression forms retain the transitional lowering path.
 pub(super) fn forwarded_argument(&mut self)->Result<(String,Ty),String>{
  if self.cur().kind==Kind::Id
   && self.ts.get(self.at+1).is_some_and(|t|t.text=="(")
   && self.ts.get(self.at+2).is_some_and(|t|t.text==")")
   && self.ts.get(self.at+3).is_some_and(|t|matches!(t.text.as_str(),","| ")")) {
   let name=self.cur().text.clone();
   if let Some(sig)=self.fns.iter().find(|f|f.name==name && f.args.is_empty() && f.ret!=Ty::Void) {
    let ty=sig.ret;
    self.at+=3;
    return Ok((format!("fn_{name}(ctx)?"),ty));
   }
  }
  self.expr(0)
 }
 pub(super) fn php_truthy_expr(expr:String,ty:Ty)->Result<String,String>{
  match ty {
   Ty::Bool=>Ok(expr),
   Ty::Int=>Ok(format!("({expr} != 0)")),
   Ty::Str=>Ok(format!("{{ let phix_truthy_string = {expr}; !phix_truthy_string.is_empty() && phix_truthy_string != \"0\" }}")),
   Ty::Json=>Ok(format!("({expr}).truthy()")),
   _=>Err("unsupported operand for PHP boolean conversion".into()),
  }
 }
 /// Evaluate both operands once, in PHP source order. A thrown left operand
 /// prevents evaluation of the right operand, including its side effects.
 /// The pending exception is observed at the enclosing statement boundary.
 pub(super) fn ordered_binary(left:String,right:String,body:String)->String{
  format!("{{ let phix_left = {left}; if phix_has_runtime_error() {{ Default::default() }} else {{ let phix_right = {right}; if phix_has_runtime_error() {{ Default::default() }} else {{ {body} }} }} }}")
 }
 pub(super) fn precedence(s:&str)->i32{match s{"??"=>1,"||"=>2,"&&"=>3,"=="|"==="|"!="|"!=="=>4,"<"|">"|"<="|">="=>5,"+"|"-"|"."=>6,"*"|"/"=>7,_=>-1}}
 pub(crate) fn expr(&mut self,min:i32)->Result<(String,Ty),String>{let(mut s,mut t)=self.primary()?;loop{
  // PHP 8 ternary is non-associative without parentheses. The shorthand
  // variant reuses its condition value rather than evaluating it twice.
  if min<=0 && self.take("?") {
   let short=self.is(":");
   let yes=if short {None} else {Some(self.expr(1)?)};
   self.need(":")?;
   let (no,nt)=self.expr(1)?;
   let target=if short {if t==nt {t} else {Ty::Json}} else {
    let yt=yes.as_ref().unwrap().1;
    if yt==nt {yt} else {Ty::Json}
   };
   if target==Ty::Void {return Err("ternary cannot produce void".into());}
   let yes_expr=if let Some((v,ty))=yes {if target==Ty::Json {Self::as_json_expr(v,ty)?}else{v}} else if target==Ty::Json {"phix_ternary_value".into()}else{"phix_ternary_value".into()};
   let no_expr=if target==Ty::Json {Self::as_json_expr(no,nt)?}else{no};
   let truthy=Self::php_truthy_expr("phix_ternary_value.clone()".into(),if target==Ty::Json {Ty::Json} else {t})?;
   let lhs=if target==Ty::Json {Self::as_json_expr(s,t)?}else{s};
   s=format!("{{ let phix_ternary_value = {lhs}; if phix_has_runtime_error() {{ Default::default() }} else if {truthy} {{ {yes_expr} }} else {{ {no_expr} }} }}");
   t=target;
   if self.is("?") {return Err("PHP 8 requires parentheses for nested ternary expressions".into());}
   continue;
  }
  let p=Self::precedence(&self.cur().text);if p<min{break;}let op=self.cur().text.clone();self.at+=1;let(rhs,rt)=self.expr(if op=="??" {p} else {p+1})?;if op=="??" {
  if t==Ty::Void || t==Ty::Bytes {return Err("unsupported null coalescing operand".into());}
  s=Self::as_json_expr(s,t)?;
  let rhs_json=Self::as_json_expr(rhs,rt)?;
  s=super::ir::TypedExpr::coalesce(s,rhs_json).emit_rust();
  t=Ty::Json;continue;
 }if op=="."{if t==Ty::Void||rt==Ty::Void||t==Ty::Bytes||rt==Ty::Bytes{return Err("cannot concatenate void or bytes".into());}s=Self::ordered_binary(s,rhs,"phix_string(&phix_left) + &phix_string(&phix_right)".into());t=Ty::Str;continue;}if op=="+" && (t==Ty::Json || rt==Ty::Json) {
  let lhs_expr=Self::as_json_expr(s,t)?;
  let rhs_expr=Self::as_json_expr(rhs,rt)?;
  s=Self::ordered_binary(lhs_expr,rhs_expr,"phix_array_union_checked(phix_left, phix_right)".into());
  t=Ty::Json;continue;
 }let lhs_ty=t;let logical=op=="&&"||op=="||";let cmp=["==","===","!=","!==","<",">","<=",">="].contains(&op.as_str());if logical{s=Self::php_truthy_expr(s,t)?;let rhs_truthy=Self::php_truthy_expr(rhs,rt)?;s=super::ir::TypedExpr::logical(&op,s,rhs_truthy)?.emit_rust();t=Ty::Bool;continue;}else if cmp{if (t!=Ty::Json && rt!=Ty::Json) && (t!=rt||t==Ty::Str||t==Ty::Void){return Err("invalid comparison types".into());}t=Ty::Bool;}else if t!=Ty::Int||rt!=Ty::Int{return Err("arithmetic requires int".into());}if cmp && (lhs_ty==Ty::Json || rt==Ty::Json) {
  // Checked comparison prevents recursive reference cells from deadlocking.
  // A recursive comparison records a request-local PHP error without a Rust panic.
  let lhs_expr=Self::as_json_expr(s,lhs_ty)?;
  let rhs_expr=Self::as_json_expr(rhs,rt)?;
  if matches!(op.as_str(),"<"|">"|"<="|">=") {
   s=Self::ordered_binary(lhs_expr,rhs_expr,format!("phix_checked_order(&phix_left, &phix_right, \"{op}\")"));
   t=Ty::Bool;continue;
  }
  let strict=op=="===" || op=="!==";
  let negate=op=="!=" || op=="!==";
  s=Self::ordered_binary(lhs_expr,rhs_expr,format!("{}phix_checked_equality(&phix_left, &phix_right, {strict})",if negate {"!"} else {""}));
  t=Ty::Bool;continue;
 }
 if logical {
  s=super::ir::TypedExpr::logical(&op,s,rhs)?.emit_rust();
  continue;
 }
 if cmp && lhs_ty==Ty::Int && rt==Ty::Int {
  s=super::ir::TypedExpr::integer_comparison(&op,s,rhs)?.emit_rust();
  continue;
 }
 if !cmp && !logical && t==Ty::Int && rt==Ty::Int {
  s=super::ir::TypedExpr::integer_binary(&op,s,rhs)?.emit_rust();
  continue;
 }
 let op=match op.as_str(){"==="=>"==","!=="=>"!=",_=>&op};s=format!("({s} {op} {rhs})");}Ok((s,t))}
 pub(super) fn block(&mut self,ret:Ty)->Result<String,String>{self.need("{")?;let saved=self.vars.clone();let mut out=String::from("{\n");while !self.is("}"){if self.cur().kind==Kind::End{return Err("unterminated block".into());}out.push_str(&self.statement(ret)?);out.push_str(if ret==Ty::Void {"if phix_has_runtime_error() { return; }\n"} else {"if phix_has_runtime_error() { return Err(phix_take_comparison_error().unwrap()); }\n"});}self.need("}")?;self.vars=saved;out.push_str("}\n");Ok(out)}
 pub(super) fn statement(&mut self,ret:Ty)->Result<String,String>{
 if self.is("break") || self.is("continue") {
  return Err(format!("line {}: {} requires structured loop-control unwinding (not yet implemented)",self.cur().line,self.cur().text));
 }
 if self.take("throw") {
  if self.take("new") {
   let class=self.name(Kind::Id)?;
   if !matches!(class.as_str(),"Exception"|"Error"|"TypeError"|"ValueError"|"ParseError"|"RuntimeException"|"LogicException"|"InvalidArgumentException"|"DomainException"|"LengthException"|"OutOfRangeException"|"BadFunctionCallException"|"BadMethodCallException"|"OutOfBoundsException"|"OverflowException"|"UnderflowException"|"RangeException"|"UnexpectedValueException") {return Err(format!("unsupported thrown exception class {class}"));}
   let has_arguments = self.take("(");
   let (message,code,previous)=if !has_arguments || self.is(")") {
    ("String::new()".to_string(),"0i64".to_string(),"Json::Null".to_string())
   } else {
    let (message,ty)=self.expr(0)?;
    if ty!=Ty::Str {return Err("exception constructor currently requires a string message".into());}
    let code=if self.take(",") {let (code,code_ty)=self.expr(0)?;if code_ty!=Ty::Int{return Err("exception constructor code must be an integer".into());}code}else{"0i64".into()};
    let previous=if self.take(",") {let (previous,previous_ty)=self.expr(0)?;if previous_ty!=Ty::Json{return Err("exception constructor previous must be Throwable or null".into());}previous}else{"Json::Null".into()};
    (message,code,previous)
   };
   if has_arguments { self.need(")")?; }
   self.need(";")?;
   let guard=if ret==Ty::Void {"return;"} else {"return Err(phix_take_comparison_error().unwrap());"};
   return Ok(format!("{{ let phix_throw_message = {message}; if phix_has_runtime_error() {{ {guard} }} let phix_throw_code = {code}; if phix_has_runtime_error() {{ {guard} }} let phix_throw_previous = {previous}; if phix_has_runtime_error() {{ {guard} }} phix_throw_builtin_with_previous(\"{class}\", phix_throw_message, phix_throw_code, phix_throw_previous); }}\n"));
  }
  let (value,ty)=self.expr(0)?;self.need(";")?;
  if ty!=Ty::Json {return Err("throw expects a Throwable object".into());}
  return Ok(format!("{{ let phix_throw_operand = {value}; if !phix_has_runtime_error() {{ phix_throw_value(phix_throw_operand); }} }}\n"));
 }
 if self.take("try") {
  // Each try/catch/finally body is emitted as a closure. A typed function's
  // error guard must return its own type's default *from the closure* while
  // leaving the pending exception available to the surrounding catch.
  // Returning a value from these closures is not yet supported: doing so
  // would incorrectly return from the closure rather than the PHP function.
  let body_start=self.at;
  let body=self.block(Ty::Void)?;
  if self.ts[body_start..self.at].iter().any(|t|t.text=="return") {
   return Err("return inside try/catch/finally requires structured unwinding".into());
  }
  let mut catches=Vec::new();
  while self.take("catch") {
   self.need("(")?;
   let mut classes=vec![self.name(Kind::Id)?];
   while self.take("|") { classes.push(self.name(Kind::Id)?); }
   for class in &classes {
    if !matches!(class.as_str(),"Throwable"|"Exception"|"Error"|"TypeError"|"ValueError"|"ParseError"|"RuntimeException"|"LogicException"|"InvalidArgumentException"|"DomainException"|"LengthException"|"OutOfRangeException"|"BadFunctionCallException"|"BadMethodCallException"|"OutOfBoundsException"|"OverflowException"|"UnderflowException"|"RangeException"|"UnexpectedValueException") {return Err(format!("unsupported exception catch class {class}"));}
   }
   let variable=if self.cur().kind==Kind::Var {Some(self.name(Kind::Var)?)} else {None};
   self.need(")")?;
   let saved=variable.as_ref().map(|v|(v.clone(),self.vars.insert(v.clone(),Ty::Json)));
   let handler_start=self.at;
   let handler=self.block(Ty::Void)?;
   if self.ts[handler_start..self.at].iter().any(|t|t.text=="return") {
    return Err("return inside try/catch/finally requires structured unwinding".into());
   }
   if let Some((variable,old))=saved {if let Some(old)=old {self.vars.insert(variable,old);} else {self.vars.remove(&variable);}}
   let predicate=classes.iter().map(|class|format!("phix_error_matches(&phix_error, \"{class}\")")).collect::<Vec<_>>().join(" || ");
   let binding=variable.map(|v|format!("let mut v_{v} = PhpSlot::new(phix_exception_json(&phix_error));")).unwrap_or_default();
   catches.push(format!("if !phix_handled && ({predicate}) {{ phix_handled = true; {binding} (|| {{ {handler} }})(); }}"));
  }
  let finally=if self.take("finally") {
   let start=self.at;
   let block=self.block(Ty::Void)?;
   if self.ts[start..self.at].iter().any(|t|t.text=="return") {
    return Err("return inside try/catch/finally requires structured unwinding".into());
   }
   Some(block)
  }else{None};
  if catches.is_empty() && finally.is_none(){return Err("try requires catch or finally".into());}
  let handler=if catches.is_empty(){String::new()}else{format!("if let Some(phix_error) = phix_take_comparison_error() {{ let mut phix_handled = false; {} if !phix_handled {{ phix_restore_runtime_error(phix_error); }} }}",catches.join("\n"))};
  // A pending exception must not prevent the finally body from running.
  // Preserve it unless the finally body raises a replacement exception.
  let finish=finally.map(|b|format!("{{ let phix_deferred_error = phix_take_comparison_error(); (|| {{ {b} }})(); if !phix_has_runtime_error() {{ if let Some(phix_original_error) = phix_deferred_error {{ phix_restore_runtime_error(phix_original_error); }} }} }}")).unwrap_or_default();
  return Ok(format!("{{ (|| {{ {body} }})(); {handler} {finish} }}\n"));
 }
 if self.take("unset") {
  self.need("(")?;
  let mut operations=Vec::new();
  loop {
   let n=self.name(Kind::Var)?;
   if n=="_PHIX" || n=="_GET" || n=="_SERVER" || n=="_FILES" {return Err("superglobals are read-only".into());}
   if self.vars.get(&n)!=Some(&Ty::Json){return Err(format!("cannot unset array element of ${n}: not an array-compatible variable"));}
   let mut keys=Vec::new();
   while self.take("[") {let key=self.array_key()?;self.need("]")?;keys.push(key);}
   if keys.is_empty(){return Err("unset currently requires an array element".into());}
   // PHP evaluates each dimension in order. A throwing key must not
   // evaluate later dimensions or mutate the array.
   let guard=if ret==Ty::Void {"return;"} else {"return Err(phix_take_comparison_error().unwrap());"};
   let mut setup=String::new();
   let mut names=Vec::new();
   for (i,key) in keys.iter().enumerate() {
    let name=format!("phix_unset_key_{i}");
    setup.push_str(&format!("let {name} = {key}; if phix_has_runtime_error() {{ {guard} }} "));
    names.push(name);
   }
   operations.push(format!("{{ {setup} v_{n}.unset_path(&[{}]); }}\n",names.join(", ")));
   if !self.take(","){break;}
  }
  self.need(")")?;self.need(";")?;
  return Ok(operations.join(""));
 }
 if self.take("echo"){let mut s=String::new();loop{let(e,t)=self.expr(0)?;if t==Ty::Void || t==Ty::Bytes{return Err("cannot echo void or raw bytes; use asset_write() for binary output".into());}let guard=if ret==Ty::Void {"return;"} else {"return Err(phix_take_comparison_error().unwrap());"};s.push_str(&format!("{{ let phix_echo_value = {e}; if phix_has_runtime_error() {{ {guard} }} ctx.echo(&phix_echo_value); }}\n"));if !self.take(","){break;}}self.need(";")?;return Ok(s);}
 if self.take("return"){
 // A direct, zero-argument user-function return can propagate its native
 // Result without converting Err into a pending flag or fabricating a value.
 // Keep this narrow until all expressions are lowered to typed outcomes.
 if ret!=Ty::Void && self.cur().kind==Kind::Id
    && self.ts.get(self.at+1).is_some_and(|t| t.text=="(")
    && self.ts.get(self.at+2).is_some_and(|t| t.text==")")
    && self.ts.get(self.at+3).is_some_and(|t| t.text==";") {
   let callee=self.cur().text.clone();
   if let Some(sig)=self.fns.iter().find(|f| f.name==callee) {
    if sig.args.is_empty() && sig.ret==ret {
     self.at+=4;
     return Ok(format!("return fn_{callee}(ctx);\n"));
    }
   }
 }
 // Directly forward a matching callee Result even when the call has arguments.
 // Evaluate each argument exactly once, in PHP source order. Any exception
 // raised during argument evaluation is returned before the callee executes.
 if ret!=Ty::Void && self.cur().kind==Kind::Id
    && self.ts.get(self.at+1).is_some_and(|t| t.text=="(") {
  let callee=self.cur().text.clone();
  if let Some(sig)=self.fns.iter().find(|f| f.name==callee).cloned() {
   if sig.ret==ret && !sig.args.is_empty() {
    let mut cursor=self.at+2;
    let mut depth=1usize;
    while cursor<self.ts.len() && depth>0 {
     match self.ts[cursor].text.as_str() { "("=>depth+=1, ")"=>depth-=1, _=>{} }
     cursor+=1;
    }
    if depth==0 && self.ts.get(cursor).is_some_and(|t|t.text==";") {
     self.at+=2;
     let mut setup=String::new();
     let mut count=0usize;
     if !self.is(")") { loop {
      let (arg,ty)=self.forwarded_argument()?;
      if sig.args.get(count).map(|a|a.1)!=Some(ty) {
       return Err(format!("argument type mismatch for {callee}"));
      }
      setup.push_str(&format!("let phix_forward_arg_{count} = {arg}; if phix_has_runtime_error() {{ return Err(phix_take_comparison_error().unwrap()); }} "));
      count+=1;
      if !self.take(",") {break;}
     }}
     self.need(")")?;self.need(";")?;
     if count!=sig.args.len(){return Err(format!("wrong argument count for {callee}"));}
     let arguments=(0..count).map(|i|format!("phix_forward_arg_{i}")).collect::<Vec<_>>().join(", ");
     return Ok(format!("{{ {setup} return fn_{callee}(ctx, {arguments}); }}\n"));
    }
   }
  }
 }
 if self.take(";"){if ret!=Ty::Void{return Err("return value required".into());}return Ok("return;\n".into());}let(e,t)=self.expr(0)?;self.need(";")?;if t!=ret{return Err("return type mismatch".into());}return Ok(format!("{{ let phix_return_value = {e}; if phix_has_runtime_error() {{ return Err(phix_take_comparison_error().unwrap()); }} return Ok(phix_return_value); }}\n"));}
 if self.take("if") || self.take("elseif"){self.need("(")?;let(e,t)=self.expr(0)?;self.need(")")?;let cond=Self::php_truthy_expr(e,t)?;let yes = self.block(ret)?; let no = if self.is("elseif") { Some(format!("{{ {} }}", self.statement(ret)?)) } else if self.take("else") { Some(if self.is("if") { format!("{{ {} }}", self.statement(ret)?) } else { self.block(ret)? }) } else { None }; let guard = if ret == Ty::Void { "return;" } else { "return Err(phix_take_comparison_error().unwrap());" }; return Ok(format!("{{ let phix_condition = {cond}; if phix_has_runtime_error() {{ {guard} }} if phix_condition {yes} {} }}", no.map(|body| format!("else {body}")).unwrap_or_default()));}
 if self.take("while"){self.need("(")?;let(e,t)=self.expr(0)?;self.need(")")?;let cond=Self::php_truthy_expr(e,t)?;let guard=if ret==Ty::Void {"return;"} else {"return Err(phix_take_comparison_error().unwrap());"};return Ok(format!("while {{ let phix_condition = {cond}; if phix_has_runtime_error() {{ {guard} }} phix_condition }} {}",self.block(ret)?));}
 if self.cur().kind==Kind::Var{
  let n=self.name(Kind::Var)?;
  if n=="_PHIX" || n=="_GET" || n=="_SERVER" || n=="_FILES"{return Err("superglobals are read-only".into());}
  if self.take("[") {
   if self.vars.get(&n)!=Some(&Ty::Json){return Err(format!("cannot assign array element of ${n}: not an array-compatible variable"));}
   let mut keys=Vec::new();
   let mut append=false;
   loop {
    if self.take("]") {append=true;} else {let key=self.array_key()?;self.need("]")?;keys.push(key);}
    if append || !self.take("["){break;}
   }
   // Evaluate every key before the RHS, stopping immediately on a throw.
   // Do not mutate the target until all expressions have succeeded.
   let guard=if ret==Ty::Void {"return;"} else {"return Err(phix_take_comparison_error().unwrap());"};
   let mut key_setup=String::new();
   let mut key_names=Vec::new();
   for (i,key) in keys.iter().enumerate() {
    let name=format!("phix_write_key_{i}");
    key_setup.push_str(&format!("let {name} = {key}; if phix_has_runtime_error() {{ {guard} }} "));
    key_names.push(name);
   }
   let key_values=key_names.join(", ");
   if self.take("??=") {
    if append {return Err("??= cannot target an append expression".into());}
    let (rhs,rt)=self.expr(0)?;
    self.need(";")?;
    let rhs=Self::as_json_expr(rhs,rt)?;
    // A missing offset reads as null in a coalescing context. Do not
    // create intermediate arrays unless the RHS completed successfully.
    return Ok(format!("{{ {key_setup} let phix_keys = vec![{key_values}]; let phix_current = v_{n}.read().phix_index_path(&phix_keys); if matches!(phix_current, Json::Null) {{ let phix_rhs = {rhs}; if phix_has_runtime_error() {{ {guard} }} v_{n}.set_path(&phix_keys, phix_rhs); }} }}\n"));
   }
   if self.take("+=") {
    if append {return Err("array += cannot target an append expression".into());}
    let (rhs,rt)=self.expr(0)?;
    self.need(";")?;
    let rhs=Self::as_json_expr(rhs,rt)?;
    return Ok(format!("{{ {key_setup} let phix_keys = vec![{key_values}]; let phix_rhs = {rhs}; if phix_has_runtime_error() {{ {guard} }} let phix_target = v_{n}.bind_element(&phix_keys); let phix_left = phix_target.read(); if let Some(phix_result)=phix_array_union_assignment_checked(phix_left,phix_rhs) {{ phix_target.replace(phix_result); }} }}\n"));
   }
   self.need("=")?;let (value,ty)=self.expr(0)?;self.need(";")?;
   let value=Self::as_json_expr(value,ty)?;
   return Ok(if append {
    format!("{{ {key_setup} let phix_write_value = {value}; if phix_has_runtime_error() {{ {guard} }} v_{n}.append_path(&[{key_values}], phix_write_value); }}\n")
   }else{format!("{{ {key_setup} let phix_write_value = {value}; if phix_has_runtime_error() {{ {guard} }} v_{n}.set_path(&[{key_values}], phix_write_value); }}\n")});
  }
  // PHP null-coalescing assignment: evaluate the RHS only for null or
  // undefined variables, and commit only after successful evaluation.
  if self.take("??=") {
   let (rhs,rt)=self.expr(0)?;
   self.need(";")?;
   let rhs=Self::as_json_expr(rhs,rt)?;
   let guard=if ret==Ty::Void {"return;"} else {"return Err(phix_take_comparison_error().unwrap());"};
   let prior=self.vars.get(&n).copied();
   if prior.is_none() {
    self.vars.insert(n.clone(),Ty::Json);
    return Ok(format!("let mut v_{n} = PhpSlot::new(Json::Null);\nif matches!(v_{n}.read(), Json::Null) {{ let phix_rhs = {rhs}; if phix_has_runtime_error() {{ {guard} }} v_{n}.replace(phix_rhs); }}\n"));
   }
   let promotion=match prior.unwrap() {
    Ty::Json=>String::new(),
    Ty::Int=>format!("let mut v_{n} = PhpSlot::new(Json::Int(v_{n}));\n"),
    Ty::Bool=>format!("let mut v_{n} = PhpSlot::new(Json::Bool(v_{n}));\n"),
    Ty::Str=>format!("let mut v_{n} = PhpSlot::new(Json::Text(v_{n}));\n"),
    _=>return Err("unsupported ??= variable type".into()),
   };
   self.vars.insert(n.clone(),Ty::Json);
   return Ok(format!("{promotion}if matches!(v_{n}.read(), Json::Null) {{ let phix_rhs = {rhs}; if phix_has_runtime_error() {{ {guard} }} v_{n}.replace(phix_rhs); }}\n"));
  }
  if self.take("+=") {
   let lhs_ty=*self.vars.get(&n).ok_or_else(||format!("undefined variable ${n}"))?;
   let (rhs,rt)=self.expr(0)?;
   self.need(";")?;
   // PHP integer += integer remains arithmetic. Array union assignment is
   // dynamically type-checked, including scalar-left invalid operands.
   if lhs_ty==Ty::Int && rt==Ty::Int {
    let guard=if ret==Ty::Void {"return;"} else {"return Err(phix_take_comparison_error().unwrap());"};
    return Ok(format!("{{ let phix_rhs = {rhs}; if phix_has_runtime_error() {{ {guard} }} v_{n} += phix_rhs; }}\n"));
   }
   let rhs=Self::as_json_expr(rhs,rt)?;
   let promotion=match lhs_ty {
    Ty::Json=>String::new(),
    Ty::Int=>format!("let mut v_{n} = PhpSlot::new(Json::Int(v_{n}));\n"),
    Ty::Bool=>format!("let mut v_{n} = PhpSlot::new(Json::Bool(v_{n}));\n"),
    Ty::Str=>format!("let mut v_{n} = PhpSlot::new(Json::Text(v_{n}));\n"),
    _=>return Err("unsupported += operand type".into()),
   };
   if lhs_ty!=Ty::Json {self.vars.insert(n.clone(),Ty::Json);}
   let guard=if ret==Ty::Void {"return;"} else {"return Err(phix_take_comparison_error().unwrap());"};
   return Ok(format!("{promotion}{{ let phix_rhs = {rhs}; if phix_has_runtime_error() {{ {guard} }} let phix_left = v_{n}.read(); if let Some(phix_result)=phix_array_union_assignment_checked(phix_left,phix_rhs) {{ v_{n}.replace(phix_result); }} }}\n"));
  }
  self.need("=")?;
  if self.take("&") {
   let target=self.name(Kind::Var)?;
   // A scalar variable becomes a PHP reference cell when first bound by reference.
   // Shadow its native Rust binding with a PhpSlot so subsequent PHP operations
   // use the shared cell, rather than copying the old native scalar.
   let promotion=match self.vars.get(&target).copied(){
    Some(Ty::Json)=>String::new(),
    Some(Ty::Int)=>format!("let mut v_{target} = PhpSlot::new(Json::Int(v_{target}));\n"),
    Some(Ty::Bool)=>format!("let mut v_{target} = PhpSlot::new(Json::Bool(v_{target}));\n"),
    Some(Ty::Str)=>format!("let mut v_{target} = PhpSlot::new(Json::Text(v_{target}));\n"),
    _=>return Err("reference target must be a PHP-compatible variable".into()),
   };
   if !promotion.is_empty(){self.vars.insert(target.clone(),Ty::Json);}
   let mut keys=Vec::new();
   while self.take("["){let key=self.array_key()?;self.need("]")?;keys.push(key);}
   self.need(";")?;
   let binding=if keys.is_empty(){format!("v_{target}.bind_reference()")}
     else{format!("v_{target}.bind_element(&[{}])",keys.join(", "))};
   if let Some(old)=self.vars.get(&n){if *old!=Ty::Json{return Err("reference assignment type mismatch".into());}
    return Ok(format!("{promotion}v_{n} = {binding};\n"));}
   self.vars.insert(n.clone(),Ty::Json);
   return Ok(format!("{promotion}let mut v_{n} = {binding};\n"));
  }
  // Promote scalar variables referenced inside array literals before generating
  // the expression. Rust bindings cannot change type inside an expression block.
  let mut promotion=String::new();
  let mut pos=self.at;
  while pos+1<self.ts.len() && self.ts[pos].text!=";" {
   if self.ts[pos].text=="&" && self.ts[pos+1].kind==Kind::Var {
    let target=self.ts[pos+1].text.clone();
    let code=match self.vars.get(&target).copied(){
     Some(Ty::Int)=>format!("let mut v_{target} = PhpSlot::new(Json::Int(v_{target}));\n"),
     Some(Ty::Bool)=>format!("let mut v_{target} = PhpSlot::new(Json::Bool(v_{target}));\n"),
     Some(Ty::Str)=>format!("let mut v_{target} = PhpSlot::new(Json::Text(v_{target}));\n"),
     _=>String::new(),
    };
    if !code.is_empty(){self.vars.insert(target,Ty::Json);promotion.push_str(&code);}
   }
   pos+=1;
  }
  let(e,t)=self.expr(0)?;self.need(";")?;
  if t==Ty::Void{return Err("cannot assign void".into());}
  if let Some(old)=self.vars.get(&n){
   if *old==Ty::Json {let value=Self::as_json_expr(e,t)?;let guard=if ret==Ty::Void {"return;"} else {"return Err(phix_take_comparison_error().unwrap());"};return Ok(format!("{promotion}{{ let phix_rhs = {value}; if phix_has_runtime_error() {{ {guard} }} v_{n}.replace(phix_rhs); }}\n"));}
   if *old!=t{return Err(format!("assignment type mismatch for ${n}"));}
   let guard=if ret==Ty::Void {"return;"} else {"return Err(phix_take_comparison_error().unwrap());"};return Ok(format!("{promotion}{{ let phix_rhs = {e}; if phix_has_runtime_error() {{ {guard} }} v_{n} = phix_rhs; }}\n"));}
  self.vars.insert(n.clone(),t);
  let guard=if ret==Ty::Void {"return;"} else {"return Err(phix_take_comparison_error().unwrap());"};
  return Ok(if t==Ty::Json{format!("{promotion}let phix_rhs = {e}; if phix_has_runtime_error() {{ {guard} }} let mut v_{n} = PhpSlot::new(phix_rhs);\n")}else{format!("{promotion}let phix_rhs = {e}; if phix_has_runtime_error() {{ {guard} }} let mut v_{n}: {} = phix_rhs;\n",t.rust())});
 }
 let(e,_)=self.expr(0)?;self.need(";")?;Ok(format!("{e};\n"))
 }
}

#[cfg(test)]
mod ordered_binary_exception_tests {
 use super::Parser;
 #[test]
 fn right_operand_is_guarded_after_left() {
  let code=Parser::ordered_binary("left_effect()".into(),"right_effect()".into(),"phix_left + phix_right".into());
  assert!(code.find("left_effect()").unwrap()<code.find("right_effect()").unwrap());
  assert!(code.find("phix_has_runtime_error()").unwrap()<code.find("right_effect()").unwrap());
 }
 #[test]
 fn right_operand_is_checked_before_combining() {
  let code=Parser::ordered_binary("left()".into(),"right()".into(),"combine(phix_left,phix_right)".into());
  assert!(code.rfind("phix_has_runtime_error()").unwrap()<code.find("combine(").unwrap());
 }
}

#[cfg(test)]
mod undefined_coalescing_tests {
 use super::*;
 use crate::lexer::lex;
 #[test]
 fn undefined_left_operand_uses_null_without_reading_a_binding() {
  let tokens=lex("<?php $missing ?? 42;").unwrap();
  let mut parser=Parser { ts:&tokens, at:0, vars:std::collections::HashMap::new(), fns:Vec::new() };
  let (rust,ty)=parser.expr(0).unwrap();
  assert_eq!(ty,Ty::Json);
  assert!(rust.contains("phix_coalesce_left = Json::Null"));
  assert!(!rust.contains("v_missing"));
 }
 #[test]
 fn ordinary_undefined_read_still_fails() {
  let tokens=lex("<?php $missing;").unwrap();
  let mut parser=Parser { ts:&tokens, at:0, vars:std::collections::HashMap::new(), fns:Vec::new() };
  assert!(parser.expr(0).unwrap_err().contains("undefined variable"));
 }
}

#[cfg(test)]
mod coalescing_assignment_tests {
 use crate::lexer::lex;
 use super::Parser;
 #[test]
 fn undefined_variable_is_initialized_lazily() {
  let tokens=lex("<?php $missing ??= 42;").unwrap();
  let mut p=Parser{ts:&tokens,at:0,vars:Default::default(),fns:Vec::new()};
  let generated=p.statement(super::Ty::Void).unwrap();
  assert!(generated.contains("PhpSlot::new(Json::Null)"));
  assert!(generated.contains("if matches!(v_missing.read(), Json::Null)"));
 }
 #[test]
 fn existing_variable_only_evaluates_rhs_for_null() {
  let tokens=lex("<?php $value = null; $value ??= 7;").unwrap();
  let mut p=Parser{ts:&tokens,at:0,vars:Default::default(),fns:Vec::new()};
  p.statement(super::Ty::Void).unwrap();
  let generated=p.statement(super::Ty::Void).unwrap();
  assert!(generated.find("if matches!").unwrap()<generated.find("let phix_rhs").unwrap());
 }
}

#[cfg(test)]
mod offset_coalescing_assignment_tests {
 use crate::lexer::lex;
 use super::{Parser,Ty};
 #[test]
 fn missing_nested_offset_is_read_before_rhs_and_written_only_after_success() {
  let tokens=lex("<?php $a = []; $a['parent']['child'] ??= 42;").unwrap();
  let mut p=Parser{ts:&tokens,at:0,vars:Default::default(),fns:Vec::new()};
  p.statement(Ty::Void).unwrap();
  let generated=p.statement(Ty::Void).unwrap();
  assert!(generated.contains("phix_index_path(&phix_keys)"));
  assert!(generated.contains("set_path(&phix_keys, phix_rhs)"));
  assert!(generated.find("phix_index_path").unwrap()<generated.find("let phix_rhs").unwrap());
  assert!(generated.find("let phix_rhs").unwrap()<generated.find("set_path").unwrap());
 }
 #[test]
 fn coalescing_accepts_typed_nonnull_operands() {
  let tokens=lex("<?php $a = 12; echo $a ?? 5;").unwrap();
  let mut p=Parser{ts:&tokens,at:0,vars:Default::default(),fns:Vec::new()};
  p.statement(Ty::Void).unwrap();
  let generated=p.statement(Ty::Void).unwrap();
  assert!(generated.contains("Json::Int(v_a)"));
 }
 #[test]
 fn offset_coalescing_rejects_append_target() {
  let tokens=lex("<?php $a = []; $a[] ??= 1;").unwrap();
  let mut p=Parser{ts:&tokens,at:0,vars:Default::default(),fns:Vec::new()};
  p.statement(Ty::Void).unwrap();
  assert!(p.statement(Ty::Void).is_err());
 }
}

#[cfg(test)]
mod missing_root_coalescing_tests {
 use crate::lexer::lex;
 use super::{Parser,Ty};
 #[test]
 fn undefined_array_root_is_safe_in_coalescing_context() {
  let tokens=lex("<?php echo $missing['user']['name'] ?? 'Guest';").unwrap();
  let mut p=Parser{ts:&tokens,at:0,vars:Default::default(),fns:Vec::new()};
  let generated=p.statement(Ty::Void).unwrap();
  assert!(generated.contains("phix_missing_key_0"));
  assert!(generated.contains("phix_missing_key_1"));
  assert!(generated.contains("phix_coalesce_left"));
 }
 #[test]
 fn undefined_array_root_is_not_silenced_without_coalescing() {
  let tokens=lex("<?php echo $missing['user'];").unwrap();
  let mut p=Parser{ts:&tokens,at:0,vars:Default::default(),fns:Vec::new()};
  assert!(p.statement(Ty::Void).is_err());
 }
}
