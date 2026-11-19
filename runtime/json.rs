// Shared no-dependency JSON parser used for build manifests and generated runtime.
// Compare integer operands to integer-form numeric strings without converting
// both sides to f64 (which loses precision beyond 2^53).
fn phix_integer_string_order(integer: i64, string: &str) -> Option<std::cmp::Ordering> {
 let trimmed=string.trim();
 if trimmed.is_empty() {return None}
 let digits=trimmed.strip_prefix(['+','-']).unwrap_or(trimmed);
 if digits.is_empty() || !digits.bytes().all(|c|c.is_ascii_digit()) {return None}
 trimmed.parse::<i64>().ok().map(|number|integer.cmp(&number))
}
// Exact decimal integer comparison avoids f64 rounding for PHP numeric strings.
// Only plain signed decimal integers use this path; decimal/exponent notation
// retains the existing floating-point conversion behavior.
fn phix_decimal_integer_string_order(left: &str, right: &str) -> Option<std::cmp::Ordering> {
 use std::cmp::Ordering;
 fn normalized(raw: &str) -> Option<(bool, &str)> {
  let s=raw.trim();
  let (negative,digits)=if let Some(v)=s.strip_prefix('-'){(true,v)}
    else if let Some(v)=s.strip_prefix('+'){(false,v)} else {(false,s)};
  if digits.is_empty() || !digits.bytes().all(|b|b.is_ascii_digit()){return None}
  let digits=digits.trim_start_matches('0');
  if digits.is_empty(){Some((false,"0"))}else{Some((negative,digits))}
 }
 let (ln,ld)=normalized(left)?;let (rn,rd)=normalized(right)?;
 if ln!=rn{return Some(if ln{Ordering::Less}else{Ordering::Greater})}
 let magnitude=ld.len().cmp(&rd.len()).then_with(||ld.cmp(rd));
 Some(if ln{magnitude.reverse()}else{magnitude})
}
#[derive(Debug)]
pub(crate) struct PhixRuntimeError { pub(crate) class: &'static str, pub(crate) message: String, pub(crate) code: i64, pub(crate) previous: Option<std::sync::Arc<PhixRuntimeError>> }
impl PhixRuntimeError { pub(crate) fn new(class: &'static str, message: String)->Self {Self{class,message,code:0,previous:None}} pub(crate) fn with_code(class: &'static str,message:String,code:i64)->Self{Self{class,message,code,previous:None}} }
#[derive(Clone,Debug)]
pub(crate) enum Json {Null,Bool(bool),Int(i64),Number(f64),Text(String),Array(Vec<Json>),Object(Vec<(String,Json)>),PhpArray(PhpArray),Throwable(std::sync::Arc<PhixRuntimeError>)}
impl Json {
 /// Detect reference cycles without recursively locking a cell already on the path.
 /// This is an inspection primitive, not yet PHP's recursive comparison semantics.
 pub(crate) fn phix_has_reference_cycle(&self) -> bool {
  fn visit(value: &Json, active: &mut std::collections::HashSet<usize>,
           completed: &mut std::collections::HashSet<usize>) -> bool {
   match value {
    Json::PhpArray(array) => {
     for (_, cell) in &array.entries {
      let id = cell.identity();
      if active.contains(&id) { return true; }
      if completed.contains(&id) { continue; }
      active.insert(id);
      // Each read releases the mutex before visiting descendants.
      let child = cell.read();
      if visit(&child, active, completed) { return true; }
      active.remove(&id);
      completed.insert(id);
     }
     false
    }
    Json::Array(items) => items.iter().any(|v| visit(v, active, completed)),
    Json::Object(items) => items.iter().any(|(_, v)| visit(v, active, completed)),
    _ => false,
   }
  }
  visit(self, &mut std::collections::HashSet::new(), &mut std::collections::HashSet::new())
 }
 /// Compare array graphs lazily, detecting only recursion actually traversed.
 /// A mismatch before a cyclic element must short-circuit normally.
 pub(crate) fn phix_checked_array_eq(&self, other:&Self, strict:bool)->Result<bool,String> {
  fn compare(a:&Json,b:&Json,strict:bool,active:&mut std::collections::HashSet<(usize,usize)>)->Result<bool,String>{
   if let (Json::PhpArray(left),Json::PhpArray(right))=(a,b){
    if left.entries.len()!=right.entries.len(){return Ok(false)}
    for (index,(key,cell)) in left.entries.iter().enumerate(){
     let rhs=if strict {
      let (rk,rc)=&right.entries[index];
      if key!=rk{return Ok(false)}
      rc
     } else {
      let Some((_,rc))=right.entries.iter().find(|(rk,_)|rk==key) else{return Ok(false)};
      rc
     };
     // PHP compares the very same referenced zval as equal without
     // descending into it. This also handles self-referential arrays
     // copied with their embedded reference identity preserved.
     if cell.same_identity(rhs) { continue; }
     let pair=(cell.identity(),rhs.identity());
     if !active.insert(pair){return Err("Nesting level too deep - recursive dependency?".into())}
     let lv=cell.read();let rv=rhs.read();
     let equal=compare(&lv,&rv,strict,active)?;
     active.remove(&pair);
     if !equal{return Ok(false)}
    }
    Ok(true)
   } else {
    // JSON-backed collections can contain PHP arrays as descendants. Never
    // fall back to unchecked recursive equality for their children: that
    // would bypass the active-reference-pair guard and may deadlock on cycles.
    match (a,b) {
     (Json::Array(left),Json::Array(right)) => {
      if left.len()!=right.len(){return Ok(false)}
      for (lv,rv) in left.iter().zip(right) {
       if !compare(lv,rv,strict,active)?{return Ok(false)}
      }
      Ok(true)
     }
     (Json::Object(left),Json::Object(right)) => {
      if left.len()!=right.len(){return Ok(false)}
      for (index,(key,lv)) in left.iter().enumerate(){
       let rv=if strict {
        let (rk,rv)=&right[index];
        if key!=rk{return Ok(false)}
        rv
       } else {
        let Some((_,rv))=right.iter().find(|(rk,_)|rk==key) else{return Ok(false)};
        rv
       };
       if !compare(lv,rv,strict,active)?{return Ok(false)}
      }
      Ok(true)
     }
     _=>Ok(if strict{a.phix_strict_eq(b)}else{a.phix_loose_eq(b)})
    }
   }
  }
  compare(self,other,strict,&mut std::collections::HashSet::new())
 }
 /// PHP array ordering is a partial order: arrays with different keys and
 /// equal sizes are incomparable, not ordered by insertion position.
 pub(crate) fn phix_array_order(&self, other:&Self)->Result<Option<std::cmp::Ordering>,String> {
  use std::cmp::Ordering;
  fn cmp(a:&Json,b:&Json,active:&mut std::collections::HashSet<(usize,usize)>)->Result<Option<Ordering>,String>{
   match (a,b){
    (Json::PhpArray(x),Json::PhpArray(y))=>{
     if x.entries.len()!=y.entries.len(){return Ok(Some(x.entries.len().cmp(&y.entries.len())))}
     for (key,cell) in &x.entries {
      let Some((_,rhs))=y.entries.iter().find(|(k,_)|k==key) else{return Ok(None)};
      if cell.same_identity(rhs){continue}
      let pair=(cell.identity(),rhs.identity());
      if !active.insert(pair){return Err("Nesting level too deep - recursive dependency?".into())}
      let left=cell.read();let right=rhs.read();
      let result=cmp(&left,&right,active)?;
      active.remove(&pair);
      match result {Some(Ordering::Equal)=>{},other=>return Ok(other)}
     }
     Ok(Some(Ordering::Equal))
    }
    // PHP compares arrays against bool/null using boolean conversion; against
    // numbers and strings, arrays are always greater (even when empty).
    (Json::PhpArray(x),Json::Bool(y))=>Ok(Some((!x.entries.is_empty()).cmp(y))),
    (Json::Bool(x),Json::PhpArray(y))=>Ok(Some(x.cmp(&(!y.entries.is_empty())))),
    (Json::PhpArray(x),Json::Null)=>Ok(Some((!x.entries.is_empty()).cmp(&false))),
    (Json::Null,Json::PhpArray(y))=>Ok(Some(false.cmp(&(!y.entries.is_empty())))),
    (Json::PhpArray(_),Json::Int(_) | Json::Number(_) | Json::Text(_))=>Ok(Some(Ordering::Greater)),
    (Json::Int(_) | Json::Number(_) | Json::Text(_),Json::PhpArray(_))=>Ok(Some(Ordering::Less)),
    // PHP 8 scalar ordering within arrays: booleans and null coerce both
    // operands to boolean, except null/string which compares against "".
    (Json::Null,Json::Text(y))=>Ok(Some("".cmp(y.as_str()))),
    (Json::Text(x),Json::Null)=>Ok(Some(x.as_str().cmp(""))),
    (Json::Bool(_),_)|(_,Json::Bool(_))|(Json::Null,_)|(_,Json::Null)=>
     Ok(Some(a.truthy().cmp(&b.truthy()))),
    (Json::Text(x),Json::Text(y))=>{
     let nx=x.trim().parse::<f64>().ok().filter(|_|!x.trim().is_empty());
     let ny=y.trim().parse::<f64>().ok().filter(|_|!y.trim().is_empty());
     Ok(match phix_decimal_integer_string_order(x,y) {
      Some(exact)=>Some(exact),
      None=>match (nx,ny){(Some(v),Some(w)) if v.is_finite() && w.is_finite()=>v.partial_cmp(&w),_=>Some(x.cmp(y))}
     })
    },
    (Json::Int(x),Json::Text(y))=>{
     let n=y.trim().parse::<f64>().ok().filter(|_|!y.trim().is_empty());
     Ok(match phix_integer_string_order(*x,y){Some(exact)=>Some(exact),None=>match n{Some(v)=>(*x as f64).partial_cmp(&v),None=>Some(x.to_string().cmp(y))}})
    },
    (Json::Text(x),Json::Int(y))=>{
     let n=x.trim().parse::<f64>().ok().filter(|_|!x.trim().is_empty());
     Ok(match phix_integer_string_order(*y,x){Some(exact)=>Some(exact.reverse()),None=>match n{Some(v)=>v.partial_cmp(&(*y as f64)),None=>Some(x.cmp(&y.to_string()))}})
    },
    (Json::Number(x),Json::Text(y))=>{
     let n=y.trim().parse::<f64>().ok().filter(|_|!y.trim().is_empty());
     Ok(match n{Some(v)=>x.partial_cmp(&v),None=>Some(x.to_string().cmp(y))})
    },
    (Json::Text(x),Json::Number(y))=>{
     let n=x.trim().parse::<f64>().ok().filter(|_|!x.trim().is_empty());
     Ok(match n{Some(v)=>v.partial_cmp(y),None=>Some(x.cmp(&y.to_string()))})
    },
    (Json::Int(x),Json::Int(y))=>Ok(Some(x.cmp(y))),
    (Json::Int(x),Json::Number(y))=>Ok((*x as f64).partial_cmp(y)),
    (Json::Number(x),Json::Int(y))=>Ok(x.partial_cmp(&(*y as f64))),
    (Json::Number(x),Json::Number(y))=>Ok(x.partial_cmp(y)),
    _=>Ok(None),
   }
  }
  cmp(self,other,&mut std::collections::HashSet::new())
 }
 /// PHP strict identity for scalar values and ordered PHP arrays.
 pub(crate) fn phix_strict_eq(&self, other: &Self) -> bool {
  match (self,other) {
   (Self::Throwable(a),Self::Throwable(b))=>std::sync::Arc::ptr_eq(a,b),
   (Self::Null,Self::Null)=>true,
   (Self::Bool(a),Self::Bool(b))=>a==b,
   (Self::Int(a),Self::Int(b))=>a==b,
   (Self::Number(a),Self::Number(b))=>a==b,
   (Self::Text(a),Self::Text(b))=>a==b,
   (Self::PhpArray(a),Self::PhpArray(b))=>a.entries.len()==b.entries.len() && a.entries.iter().zip(&b.entries).all(|((ak,av),(bk,bv))| ak==bk && av.read().phix_strict_eq(&bv.read())),
   _=>false,
  }
 }
 /// PHP loose array equality: key order is ignored; values are compared recursively.
 /// Scalar comparisons here cover PHP 8-style bool/null and numeric-string cases.
 pub(crate) fn phix_loose_eq(&self, other:&Self)->bool {
  match (self,other) {
   (Self::PhpArray(a),Self::PhpArray(b))=>{
    a.entries.len()==b.entries.len() && a.entries.iter().all(|(key,val)|
     b.entries.iter().find(|(k,_)|k==key).is_some_and(|(_,rhs)|val.read().phix_loose_eq(&rhs.read())))
   },
   // PHP 8 compares arrays to bool/null using their truthiness, but
   // arrays compared with numbers or strings are always unequal.
   (Self::PhpArray(a),Self::Bool(b))|(Self::Bool(b),Self::PhpArray(a))=>!a.is_empty()==*b,
   (Self::PhpArray(a),Self::Null)|(Self::Null,Self::PhpArray(a))=>a.is_empty(),
   (Self::PhpArray(_),_)|(_,Self::PhpArray(_))=>false,
   // PHP compares null with strings as an empty string, not via boolean coercion.
   (Self::Null,Self::Text(s))|(Self::Text(s),Self::Null)=>s.is_empty(),
   (Self::Bool(_),_)|(_,Self::Bool(_))|(Self::Null,_)|(_,Self::Null)=>self.truthy()==other.truthy(),
   (Self::Int(a),Self::Int(b))=>a==b,
   (Self::Number(a),Self::Number(b))=>a==b,
   (Self::Int(a),Self::Number(b))|(Self::Number(b),Self::Int(a))=>(*a as f64)==*b,
   (Self::Text(a),Self::Text(b))=>{
     if let Some(exact)=phix_decimal_integer_string_order(a,b) {
      exact==std::cmp::Ordering::Equal
     } else {
      match (a.trim().parse::<f64>(),b.trim().parse::<f64>()) {
       (Ok(x),Ok(y)) if !a.trim().is_empty() && !b.trim().is_empty() && x.is_finite() && y.is_finite()=>x==y,
       _=>a==b,
      }
     }
   },
   (Self::Text(a),Self::Int(b))|(Self::Int(b),Self::Text(a))=>
    phix_integer_string_order(*b,a).is_some_and(|o|o==std::cmp::Ordering::Equal) || (phix_integer_string_order(*b,a).is_none() && a.trim().parse::<f64>().ok().is_some_and(|n|!a.trim().is_empty() && n==(*b as f64))),
   (Self::Text(a),Self::Number(b))|(Self::Number(b),Self::Text(a))=>
    a.trim().parse::<f64>().ok().is_some_and(|n|!a.trim().is_empty() && n==*b),
   _=>self.phix_strict_eq(other),
  }
 }
 pub(crate) fn truthy(&self)->bool{match self{Self::Null=>false,Self::Bool(v)=>*v,Self::Int(v)=>*v!=0,Self::Number(v)=>*v!=0.0,Self::Text(v)=>!v.is_empty()&&v!="0",Self::Array(v)=>!v.is_empty(),Self::Object(v)=>!v.is_empty(),Self::PhpArray(v)=>!v.is_empty(),Self::Throwable(_)=>true}}
 pub(crate) fn as_php_string(&self)->String{match self{Self::Null=>String::new(),Self::Bool(true)=>"1".into(),Self::Bool(false)=>String::new(),Self::Int(v)=>v.to_string(),Self::Number(v)=>v.to_string(),Self::Text(v)=>v.clone(),Self::Array(_)=>"Array".into(),Self::Object(_)=>"Object".into(),Self::PhpArray(_)=>"Array".into(),Self::Throwable(_)=>"Object".into()}}

 // Resolve nested PHP array elements to their original identity cells.
 // Never clone intermediate arrays: cloning would lose reference identity.
 pub(crate) fn phix_ensure_path_cell(&mut self,path:&[Json])->Option<PhpCell>{
  let (first,rest)=path.split_first()?;
  if matches!(self,Self::Null){*self=Self::PhpArray(PhpArray::new());}
  let Self::PhpArray(array)=self else{return None};
  let cell=array.ensure_cell(first)?;
  if rest.is_empty(){return Some(cell);}
  cell.with_mut(|child|child.phix_ensure_path_cell(rest))
 }
 pub(crate) fn phix_index_path(&self,path:&[Json])->Json {
  path.iter().fold(self.clone(),|value,key|value.phix_index(key))
 }
 pub(crate) fn phix_index(&self,key:&Json)->Json{
  match (self,key){
   (Self::PhpArray(items),key)=>items.get(key),
   (Self::Array(items),Self::Int(n))=>usize::try_from(*n).ok().and_then(|i|items.get(i)).cloned().unwrap_or(Self::Null),
   (Self::Object(items),Self::Text(key))=>items.iter().find(|(k,_)|k==key).map(|(_,v)|v.clone()).unwrap_or(Self::Null),
   _=>Self::Null,
  }
 }
 pub(crate) fn phix_set(&mut self,key:&Json,value:Json){
  match (self,key){
   (Self::PhpArray(items),key)=>items.set(key,value),
   (Self::Array(items),Self::Int(n))=>{
    if let Ok(i)=usize::try_from(*n){if i<items.len(){items[i]=value;}else if i==items.len(){items.push(value);}}
   },
   (Self::Object(items),Self::Text(key))=>{
    if let Some((_,existing))=items.iter_mut().find(|(k,_)|k==key){*existing=value;}else{items.push((key.clone(),value));}
   },
   _=>{},
  }
 }
 // Traverse the original cell, not a cloned intermediate array. This is
 // essential when an intermediate array contains referenced children.
 pub(crate) fn phix_set_path(&mut self,path:&[Json],value:Json){
  let Some((first,rest))=path.split_first() else{return};
  // PHP auto-vivifies a null intermediate into an array even when the
  // remaining path contains only its final key.
  if matches!(self,Self::Null){*self=Self::PhpArray(PhpArray::new());}
  if rest.is_empty(){self.phix_set(first,value);return;}
  if let Self::PhpArray(array)=self {
   if let Some(cell)=array.ensure_cell(first){cell.with_mut(|child|child.phix_set_path(rest,value));}
  }
 }
 pub(crate) fn phix_append_path(&mut self,path:&[Json],value:Json){
  if matches!(self,Self::Null){*self=Self::PhpArray(PhpArray::new());}
  let Some((first,rest))=path.split_first() else{self.phix_append(value);return;};
  if let Self::PhpArray(array)=self {
   if let Some(cell)=array.ensure_cell(first){cell.with_mut(|child|child.phix_append_path(rest,value));}
  }
 }
 pub(crate) fn phix_unset_path(&mut self,path:&[Json]){
  let Some((first,rest))=path.split_first() else{return};
  if rest.is_empty(){self.phix_unset(first);return;}
  if let Self::PhpArray(array)=self {
   if let Some(cell)=array.cell(first){cell.with_mut(|child|child.phix_unset_path(rest));}
  }
 }
 pub(crate) fn phix_unset(&mut self,key:&Json){
  if let Self::PhpArray(items)=self {items.remove(key);}
 }
 pub(crate) fn phix_append(&mut self,value:Json){match self {Self::PhpArray(items)=>items.append(value),Self::Array(items)=>items.push(value),_=>{}}}
 pub(crate) fn get(&self,key:&str)->Option<&Json>{match self {Json::Object(fields)=>fields.iter().find(|(k,_)|k==key).map(|(_,v)|v),_=>None}}
}
pub(crate) struct JsonParser<'a>{src:&'a [u8],at:usize}
impl<'a> JsonParser<'a>{
 fn new(s:&'a str)->Self{Self{src:s.as_bytes(),at:0}}
 fn ws(&mut self){while self.at<self.src.len()&&self.src[self.at].is_ascii_whitespace(){self.at+=1;}}
 fn eat(&mut self,b:u8)->bool{self.ws();if self.src.get(self.at)==Some(&b){self.at+=1;true}else{false}}
 fn need(&mut self,b:u8)->Result<(),String>{if self.eat(b){Ok(())}else{Err(format!("expected '{}' at byte {}",b as char,self.at))}}
 fn string(&mut self)->Result<String,String>{self.need(b'"')?;let mut out=String::new();while let Some(&c)=self.src.get(self.at){self.at+=1;match c {
 b'"'=>return Ok(out),b'\\'=>{let e=*self.src.get(self.at).ok_or("invalid escape")?;self.at+=1;match e {b'"'=>out.push('"'),b'\\'=>out.push('\\'),b'/'=>out.push('/'),b'n'=>out.push('\n'),b'r'=>out.push('\r'),b't'=>out.push('\t'),b'b'=>out.push('\u{0008}'),b'f'=>out.push('\u{000c}'),b'u'=>{let cp=self.hex4()?;if (0xD800..=0xDBFF).contains(&cp){if self.src.get(self.at..self.at+2)!=Some(&b"\\u"[..]){return Err("missing low surrogate".into());}self.at+=2;let low=self.hex4()?;if !(0xDC00..=0xDFFF).contains(&low){return Err("invalid low surrogate".into());}out.push(char::from_u32(0x10000+((cp-0xD800)<<10)+(low-0xDC00)).ok_or("bad Unicode scalar")?);}else{out.push(char::from_u32(cp).ok_or("invalid Unicode scalar")?);}},_=>return Err("invalid escape".into())}},
 0..=31=>return Err("control byte in JSON string".into()),_=>{let pos=self.at-1;let s=std::str::from_utf8(&self.src[pos..]).map_err(|_|"invalid UTF-8")?;let ch=s.chars().next().ok_or("invalid UTF-8")?;out.push(ch);self.at=pos+ch.len_utf8();}
 }}Err("unterminated JSON string".into())}
 fn hex4(&mut self)->Result<u32,String>{let mut v=0u32;for _ in 0..4{let c=*self.src.get(self.at).ok_or("short Unicode escape")?;self.at+=1;v=v*16+((c as char).to_digit(16).ok_or("bad Unicode hex")?);}Ok(v)}
 fn value(&mut self,depth:usize)->Result<Json,String>{if depth>64{return Err("JSON depth limit exceeded".into());}self.ws();match self.src.get(self.at).copied(){
 Some(b'{')=>{self.at+=1;let mut fields:Vec<(String,Json)>=Vec::new();if self.eat(b'}'){return Ok(Json::Object(fields));}loop{let key=self.string()?;if fields.iter().any(|(k,_)|k==&key){return Err(format!("duplicate configuration key: {key}"));}self.need(b':')?;let v=self.value(depth+1)?;fields.push((key,v));if self.eat(b'}'){break;}self.need(b',')?;}Ok(Json::Object(fields))},
 Some(b'[')=>{self.at+=1;let mut a=Vec::new();if self.eat(b']'){return Ok(Json::Array(a));}loop{a.push(self.value(depth+1)?);if self.eat(b']'){break;}self.need(b',')?;}Ok(Json::Array(a))},
 Some(b'"')=>Ok(Json::Text(self.string()?)),
 Some(b't') if self.src.get(self.at..self.at+4)==Some(&b"true"[..])=>{self.at+=4;Ok(Json::Bool(true))},
 Some(b'f') if self.src.get(self.at..self.at+5)==Some(&b"false"[..])=>{self.at+=5;Ok(Json::Bool(false))},
 Some(b'n') if self.src.get(self.at..self.at+4)==Some(&b"null"[..])=>{self.at+=4;Ok(Json::Null)},
 Some(b'-'|b'0'..=b'9')=>{let start=self.at;if self.src[self.at]==b'-'{self.at+=1;}if self.src.get(self.at)==Some(&b'0'){self.at+=1;}else {let mut any=false;while self.src.get(self.at).map_or(false,|b|b.is_ascii_digit()){self.at+=1;any=true;}if !any{return Err("invalid number".into());}}
 let mut float=false;if self.src.get(self.at)==Some(&b'.'){float=true;self.at+=1;let s=self.at;while self.src.get(self.at).map_or(false,|b|b.is_ascii_digit()){self.at+=1;}if s==self.at{return Err("invalid fractional number".into());}}
 if matches!(self.src.get(self.at),Some(b'e'|b'E')){float=true;self.at+=1;if matches!(self.src.get(self.at),Some(b'+'|b'-')){self.at+=1;}let s=self.at;while self.src.get(self.at).map_or(false,|b|b.is_ascii_digit()){self.at+=1;}if s==self.at{return Err("invalid exponent".into());}}
 let raw=std::str::from_utf8(&self.src[start..self.at]).map_err(|_|"invalid numeric encoding")?;if float{let n=raw.parse::<f64>().map_err(|_|"invalid floating number")?;if !n.is_finite(){return Err("non-finite number".into());}Ok(Json::Number(n))}else{Ok(Json::Int(raw.parse().map_err(|_|"integer out of range")?))}},
 _=>Err(format!("invalid JSON at byte {}",self.at))}}
 pub(crate) fn parse(s:&'a str)->Result<Json,String>{let mut p=Self::new(s);let v=p.value(0)?;p.ws();if p.at!=p.src.len(){return Err("trailing data after config JSON".into());}Ok(v)}
}

#[cfg(test)]
mod loose_comparison_tests {
 use super::*;
 #[test]
 fn array_key_order_is_ignored_but_strict_order_matters() {
  let a=Json::PhpArray(PhpArray::from_parts(vec![
   (Some(Json::Text("a".into())),Json::Int(1)),
   (Some(Json::Text("b".into())),Json::Int(2))]));
  let b=Json::PhpArray(PhpArray::from_parts(vec![
   (Some(Json::Text("b".into())),Json::Int(2)),
   (Some(Json::Text("a".into())),Json::Int(1))]));
  assert!(a.phix_loose_eq(&b));
  assert!(!a.phix_strict_eq(&b));
 }
 #[test]
 fn nested_values_use_loose_comparison() {
  let a=Json::PhpArray(PhpArray::from_parts(vec![(None,Json::PhpArray(PhpArray::from_parts(vec![(None,Json::Int(1))])))]));
  let b=Json::PhpArray(PhpArray::from_parts(vec![(None,Json::PhpArray(PhpArray::from_parts(vec![(None,Json::Text("1".into()))])))]));
  assert!(a.phix_loose_eq(&b));
  assert!(!a.phix_strict_eq(&b));
 }
 #[test]
 fn missing_key_is_not_equal() {
  let a=Json::PhpArray(PhpArray::from_parts(vec![(Some(Json::Text("a".into())),Json::Null)]));
  let b=Json::PhpArray(PhpArray::from_parts(vec![(Some(Json::Text("b".into())),Json::Null)]));
  assert!(!a.phix_loose_eq(&b));
 }
}

#[cfg(test)]
mod null_string_array_parity_tests {
 use super::*;
 #[test]
 fn php_null_string_comparison_inside_arrays() {
  for (s, expected) in [("",true),("0",false),("0.0",false),(" ",false),("foo",false)] {
   let a=Json::PhpArray(PhpArray::from_parts(vec![(None,Json::Null)]));
   let b=Json::PhpArray(PhpArray::from_parts(vec![(None,Json::Text(s.into()))]));
   assert_eq!(a.phix_loose_eq(&b),expected,"null compared with {s:?}");
   assert_eq!(b.phix_loose_eq(&a),expected,"reverse comparison with {s:?}");
  }
 }
}

#[cfg(test)]
mod checked_recursive_comparison_tests {
 use super::*;
 #[test]
 fn identical_cyclic_array_does_not_recurse() {
  let mut a=PhpArray::new();
  let cell=a.ensure_cell(&Json::Text("self".into())).unwrap();
  cell.mark_reference();
  let root=Json::PhpArray(a);
  cell.write(root.clone());
  assert_eq!(root.phix_checked_array_eq(&root,false),Ok(true));
  assert_eq!(root.phix_checked_array_eq(&root,true),Ok(true));
 }
 #[test]
 fn acyclic_comparison_still_matches_existing_semantics() {
  let a=Json::PhpArray(PhpArray::from_parts(vec![(None,Json::Int(1))]));
  let b=Json::PhpArray(PhpArray::from_parts(vec![(None,Json::Text("1".into()))]));
  assert_eq!(a.phix_checked_array_eq(&b,false),Ok(true));
  assert_eq!(a.phix_checked_array_eq(&b,true),Ok(false));
 }
}

#[cfg(test)]
mod pairwise_checked_cycle_tests {
 use super::*;
 fn recursive(first:i64)->Json {
  let mut a=PhpArray::new();
  a.set(&Json::Text("first".into()),Json::Int(first));
  let self_cell=a.ensure_cell(&Json::Text("self".into())).unwrap();
  self_cell.mark_reference();
  self_cell.write(Json::PhpArray(a.clone()));
  Json::PhpArray(a)
 }
 #[test] fn mismatch_before_cycle_is_false(){
  let a=recursive(1);let b=recursive(2);
  assert_eq!(a.phix_checked_array_eq(&b,false),Ok(false));
  assert_eq!(a.phix_checked_array_eq(&b,true),Ok(false));
 }
 #[test] fn reached_cycle_returns_error(){
  let a=recursive(1);let b=recursive(1);
  assert!(a.phix_checked_array_eq(&b,false).is_err());
  assert!(a.phix_checked_array_eq(&b,true).is_err());
 }
}

#[cfg(test)]
mod identical_reference_comparison_tests {
 use super::*;
 #[test]
 fn cyclic_array_compares_equal_to_its_own_copy() {
  let root = PhpCell::new(Json::PhpArray(PhpArray::new()));
  root.mark_reference();
  root.with_mut(|v| if let Json::PhpArray(a)=v {
   *a = PhpArray::from_cell_parts(vec![(Some(Json::Text("self".into())),root.alias())]);
  });
  let value = root.read();
  let copy = value.clone();
  assert_eq!(value.phix_checked_array_eq(&value,false),Ok(true));
  assert_eq!(value.phix_checked_array_eq(&copy,false),Ok(true));
  assert_eq!(value.phix_checked_array_eq(&copy,true),Ok(true));
 }
 #[test]
 fn same_cell_identity_short_circuits_but_different_cycles_error() {
  let make = || {
   let root=PhpCell::new(Json::PhpArray(PhpArray::new()));
   root.mark_reference();
   root.with_mut(|v| if let Json::PhpArray(a)=v {
    *a=PhpArray::from_cell_parts(vec![(Some(Json::Text("self".into())),root.alias())]);
   });
   root.read()
  };
  let a=make();let b=make();
  assert!(a.phix_checked_array_eq(&b,false).is_err());
 }
}

#[cfg(test)]
mod nested_checked_collection_tests {
 use super::*;
 fn recursive() -> Json {
  let root=PhpCell::new(Json::PhpArray(PhpArray::new()));
  root.mark_reference();
  root.with_mut(|v| if let Json::PhpArray(a)=v {
   *a=PhpArray::from_cell_parts(vec![(Some(Json::Text("self".into())),root.alias())]);
  });
  root.read()
 }
 #[test]
 fn nested_json_array_detects_distinct_recursive_arrays() {
  let left=Json::Array(vec![recursive()]);
  let right=Json::Array(vec![recursive()]);
  assert!(left.phix_checked_array_eq(&right,false).is_err());
  assert!(left.phix_checked_array_eq(&right,true).is_err());
 }
 #[test]
 fn nested_json_object_short_circuits_before_recursion() {
  let left=Json::Object(vec![("first".into(),Json::Int(1)),("cycle".into(),recursive())]);
  let right=Json::Object(vec![("first".into(),Json::Int(2)),("cycle".into(),recursive())]);
  assert_eq!(left.phix_checked_array_eq(&right,false),Ok(false));
  assert_eq!(left.phix_checked_array_eq(&right,true),Ok(false));
 }
}

#[cfg(test)]
mod php_array_ordering_regressions {
 use super::*;
 use std::cmp::Ordering;
 fn arr(items: Vec<(&str,i64)>)->Json {Json::PhpArray(PhpArray::from_parts(items.into_iter().map(|(k,v)|(Some(Json::Text(k.into())),Json::Int(v))).collect()))}
 #[test] fn keyed_array_comparison_is_independent_of_insertion_order(){
  let a=arr(vec![("a",1),("b",2)]);let b=arr(vec![("b",2),("a",1)]);
  assert_eq!(a.phix_array_order(&b).unwrap(),Some(Ordering::Equal));
 }
 #[test] fn equal_size_different_keys_are_unordered(){
  assert_eq!(arr(vec![("a",1)]).phix_array_order(&arr(vec![("b",1)])).unwrap(),None);
 }
 #[test] fn array_scalar_order_matches_php(){
  let empty=arr(vec![]);let nonempty=arr(vec![("a",1)]);
  assert_eq!(empty.phix_array_order(&Json::Null).unwrap(),Some(Ordering::Equal));
  assert_eq!(empty.phix_array_order(&Json::Bool(true)).unwrap(),Some(Ordering::Less));
  assert_eq!(nonempty.phix_array_order(&Json::Bool(true)).unwrap(),Some(Ordering::Equal));
  assert_eq!(nonempty.phix_array_order(&Json::Null).unwrap(),Some(Ordering::Greater));
  assert_eq!(empty.phix_array_order(&Json::Int(0)).unwrap(),Some(Ordering::Greater));
  assert_eq!(Json::Text(String::new()).phix_array_order(&empty).unwrap(),Some(Ordering::Less));
 }
 #[test] fn length_precedes_element_comparison(){
  assert_eq!(arr(vec![]).phix_array_order(&arr(vec![("a",1)])).unwrap(),Some(Ordering::Less));
 }
}

#[cfg(test)]
mod php_array_mixed_value_ordering {
 use super::*;
 use std::cmp::Ordering;
 fn a(v:Json)->Json {Json::PhpArray(PhpArray::from_parts(vec![(None,v)]))}
 #[test] fn numeric_string_and_integer(){assert_eq!(a(Json::Text("10".into())).phix_array_order(&a(Json::Int(2))).unwrap(),Some(Ordering::Greater));}
 #[test] fn nonnumeric_string_and_integer(){assert_eq!(a(Json::Text("abc".into())).phix_array_order(&a(Json::Int(0))).unwrap(),Some(Ordering::Greater));}
 #[test] fn null_and_string(){assert_eq!(a(Json::Null).phix_array_order(&a(Json::Text("x".into()))).unwrap(),Some(Ordering::Less));}
 #[test] fn bool_and_string(){assert_eq!(a(Json::Bool(false)).phix_array_order(&a(Json::Text("0".into()))).unwrap(),Some(Ordering::Equal));}
}

#[cfg(test)]
mod php_large_integer_numeric_string_tests {
 use super::*;
 use std::cmp::Ordering;
 fn arr(v:Json)->Json {Json::PhpArray(PhpArray::from_parts(vec![(None,v)]))}
 #[test] fn integer_string_order_preserves_precision(){
  let number=arr(Json::Int(9_007_199_254_740_993));
  assert_eq!(number.phix_array_order(&arr(Json::Text("9007199254740992".into()))).unwrap(),Some(Ordering::Greater));
  assert_eq!(number.phix_array_order(&arr(Json::Text("9007199254740994".into()))).unwrap(),Some(Ordering::Less));
  assert_eq!(number.phix_array_order(&arr(Json::Text("9007199254740993".into()))).unwrap(),Some(Ordering::Equal));
 }
 #[test] fn integer_string_loose_equality_preserves_precision(){
  assert!(!Json::Int(9_007_199_254_740_993).phix_loose_eq(&Json::Text("9007199254740992".into())));
  assert!(Json::Int(9_007_199_254_740_993).phix_loose_eq(&Json::Text("9007199254740993".into())));
 }
}

#[cfg(test)]
mod exact_numeric_string_array_tests {
 use super::*;
 use std::cmp::Ordering;
 fn array(s:&str)->Json {Json::PhpArray(PhpArray::from_parts(vec![(None,Json::Text(s.into()))]))}
 #[test] fn distinct_large_integers_are_not_equal(){
  let a=array("9007199254740993");let b=array("9007199254740992");
  assert!(!a.phix_loose_eq(&b));
  assert_eq!(a.phix_array_order(&b).unwrap(),Some(Ordering::Greater));
 }
 #[test] fn huge_numeric_strings_compare_exactly(){
  let a=array("9223372036854775808");let b=array("9223372036854775809");
  assert!(!a.phix_loose_eq(&b));
  assert_eq!(a.phix_array_order(&b).unwrap(),Some(Ordering::Less));
 }
 #[test] fn signs_and_zero_padding_are_normalized(){
  assert!(array("+0001").phix_loose_eq(&array("1")));
  assert!(array("-000").phix_loose_eq(&array("0")));
  assert_eq!(array("-20").phix_array_order(&array("-3")).unwrap(),Some(Ordering::Less));
 }
}

#[cfg(test)]
mod php_overflow_numeric_string_array_tests {
 use super::*;
 use std::cmp::Ordering;
 fn array(value:&str)->Json {Json::PhpArray(PhpArray::from_parts(vec![(None,Json::Text(value.into()))]))}
 #[test] fn overflowed_exponent_strings_compare_lexically(){
  let a=array("1e309");let b=array("2e309");
  assert!(!a.phix_loose_eq(&b));
  assert_eq!(a.phix_array_order(&b).unwrap(),Some(Ordering::Less));
  assert_eq!(array("-1e309").phix_array_order(&array("-2e309")).unwrap(),Some(Ordering::Less));
 }
 #[test] fn overflowed_numeric_strings_do_not_equal_case_variants(){
  assert!(!array("1e309").phix_loose_eq(&array("1E309")));
  assert!(array("1e309").phix_loose_eq(&array("1e309")));
 }
}

#[cfg(test)]
mod native_throwable_identity_tests {
 use super::*;
 use std::sync::Arc;
 #[test]
 fn cloned_throwable_retains_strict_identity() {
  let value=Json::Throwable(Arc::new(PhixRuntimeError::with_code("Exception","original".into(),42)));
  assert!(value.phix_strict_eq(&value.clone()));
  let distinct=Json::Throwable(Arc::new(PhixRuntimeError::with_code("Exception","original".into(),42)));
  assert!(!value.phix_strict_eq(&distinct));
 }
 #[test]
 fn ordinary_object_cannot_be_a_throwable() {
  let forged=Json::Object(vec![("class".into(),Json::Text("Exception".into()))]);
  assert!(!matches!(forged,Json::Throwable(_)));
 }
}

#[cfg(test)]
mod null_path_autovivification_tests {
 use super::{Json,PhpArray};
 fn key(name:&str)->Json {Json::Text(name.into())}
 #[test]
 fn nested_set_creates_final_array_from_null() {
  let mut root=Json::PhpArray(PhpArray::new());
  root.phix_set_path(&[key("user"),key("name")],Json::Text("Guest".into()));
  assert_eq!(root.phix_index_path(&[key("user"),key("name")]).as_php_string(),"Guest");
 }
 #[test]
 fn nested_append_creates_final_array_from_null() {
  let mut root=Json::PhpArray(PhpArray::new());
  root.phix_append_path(&[key("user"),key("items")],Json::Int(42));
  assert_eq!(root.phix_index_path(&[key("user"),key("items"),Json::Int(0)]).as_php_string(),"42");
 }
 #[test]
 fn null_root_accepts_final_key_and_append() {
  let mut root=Json::Null;
  root.phix_set_path(&[key("name")],Json::Text("Guest".into()));
  assert_eq!(root.phix_index(&key("name")).as_php_string(),"Guest");
  let mut appended=Json::Null;
  appended.phix_append_path(&[],Json::Int(9));
  assert_eq!(appended.phix_index(&Json::Int(0)).as_php_string(),"9");
 }
}
