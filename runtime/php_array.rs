// PHP-source array storage. JSON configuration remains a separate representation.
#[derive(Clone,Debug,PartialEq,Eq)]
pub(crate) enum PhpKey { Int(i64), String(String) }
#[derive(Debug)]
pub(crate) struct PhpArray { entries: Vec<(PhpKey, PhpCell)>, next: Option<i64>, numeric_seen: bool }
impl Clone for PhpArray {
 fn clone(&self)->Self {Self{entries:self.entries.iter().map(|(k,v)|(k.clone(),v.clone_for_array_copy())).collect(),next:self.next,numeric_seen:self.numeric_seen}}
}
impl PhpKey {
 fn from_value(v:&Json)->Result<Self,String>{match v {
  Json::Int(n)=>Ok(Self::Int(*n)),Json::Bool(b)=>Ok(Self::Int(i64::from(*b))),
  Json::Null=>Ok(Self::String(String::new())),
  Json::Number(n)=>{
   // PHP converts float keys to signed platform integers, wrapping outside
   // the i64 range; Rust float-to-int casts otherwise saturate.
   let key=if !n.is_finite(){0}
    else if *n >= -9223372036854775808.0 && *n < 9223372036854775808.0 {n.trunc() as i64}
    else {let wrapped=n.trunc().rem_euclid(18446744073709551616.0);
     if wrapped>=9223372036854775808.0 {(wrapped-18446744073709551616.0) as i64}
     else {wrapped as i64}};
   Ok(Self::Int(key))
  },
  Json::Text(s)=>{let digits=s.strip_prefix('-').unwrap_or(s);
   if !digits.is_empty() && digits.bytes().all(|b|b.is_ascii_digit()) && !(digits.len()>1 && digits.starts_with('0')) && s!="-0" {
    if let Ok(n)=s.parse::<i64>() {return Ok(Self::Int(n));}
   } Ok(Self::String(s.clone()))},
  _=>Err("illegal array key type".into())
 }}
}
impl PhpArray {
 pub(crate) fn new()->Self{Self{entries:Vec::new(),next:Some(0),numeric_seen:false}}
 pub(crate) fn from_parts(entries:Vec<(Option<Json>,Json)>)->Self{
  let mut a=Self::new();for part in entries {match part {(Some(key),value)=>a.set(&key,value),(None,value)=>a.append(value)}}a
 }
 pub(crate) fn from_cell_parts(entries:Vec<(Option<Json>,PhpCell)>)->Self{
  let mut a=Self::new();
  for (key,cell) in entries {
   let key=match key {
    Some(raw)=>match PhpKey::from_value(&raw){Ok(key)=>key,Err(_)=>continue},
    None=>match a.next {Some(next)=>PhpKey::Int(next),None=>continue},
   };
   a.update_index(&key);
   if let Some((_,existing))=a.entries.iter_mut().find(|(k,_)|*k==key){
    // PHP duplicate keys replace the entry without changing its position.
    *existing=cell;
   }else{a.entries.push((key,cell));}
  }
  a
 }
 /// PHP array union (+): preserve every left-hand key and its insertion
 /// position, then append only keys absent from the left. Reference cells
 /// retain alias identity; ordinary cells are copied as values.
 pub(crate) fn union(&self, rhs:&Self)->Self {
  let mut result=self.clone();
  for (key,cell) in &rhs.entries {
   if result.entries.iter().any(|(existing,_)|existing==key){continue;}
   result.update_index(key);
   result.entries.push((key.clone(),cell.clone_for_array_copy()));
  }
  result
 }
 pub(crate) fn from_entries(entries:Vec<(Json,Json)>)->Self{
  let mut a=Self::new();for (key,value) in entries {a.set(&key,value);}a
 }
 fn update_index(&mut self,key:&PhpKey){if let PhpKey::Int(n)=key {
  if !self.numeric_seen {self.next=n.checked_add(1);}
  else if let Some(next)=self.next {if *n>=next {self.next=n.checked_add(1);}}
  self.numeric_seen=true;
 }}
 pub(crate) fn get(&self,key:&Json)->Json{
  let Ok(key)=PhpKey::from_value(key) else {return Json::Null};
  self.entries.iter().find(|(k,_)|*k==key).map(|(_,v)|v.read()).unwrap_or(Json::Null)
 }
 pub(crate) fn set(&mut self,key:&Json,value:Json){
  let Ok(key)=PhpKey::from_value(key) else {return};
  self.update_index(&key);
  if let Some((_,v))=self.entries.iter_mut().find(|(k,_)|*k==key){v.write(value);}
  else{self.entries.push((key,PhpCell::new(value)));}
 }
 pub(crate) fn cell(&self,key:&Json)->Option<PhpCell>{
  let key=PhpKey::from_value(key).ok()?;
  self.entries.iter().find(|(k,_)|*k==key).map(|(_,v)|v.alias())
 }
 pub(crate) fn ensure_cell(&mut self,key:&Json)->Option<PhpCell>{
  let key=PhpKey::from_value(key).ok()?;
  if let Some((_,v))=self.entries.iter().find(|(k,_)|*k==key){return Some(v.alias());}
  self.update_index(&key);
  let cell=PhpCell::new(Json::Null);
  self.entries.push((key,cell.alias()));
  Some(cell)
 }
 pub(crate) fn remove(&mut self,key:&Json)->bool{
  let Ok(key)=PhpKey::from_value(key) else {return false};
  if let Some(index)=self.entries.iter().position(|(k,_)|*k==key){self.entries.remove(index);true}else{false}
 }
 pub(crate) fn append(&mut self,value:Json){
  if let Some(next)=self.next {self.set(&Json::Int(next),value);}
 }
 pub(crate) fn is_empty(&self)->bool{self.entries.is_empty()}
}
#[cfg(test)]
mod php_array_tests {
 use super::*;
 #[test] fn array_copy_preserves_referenced_cells_but_copies_normal_cells(){
  let mut a=PhpArray::new();
  a.set(&Json::Text("reference".into()),Json::Int(10));
  a.set(&Json::Text("ordinary".into()),Json::Int(20));
  let reference=a.cell(&Json::Text("reference".into())).unwrap();
  reference.mark_reference();
  let mut b=a.clone();
  b.set(&Json::Text("reference".into()),Json::Int(30));
  b.set(&Json::Text("ordinary".into()),Json::Int(40));
  assert!(matches!(a.get(&Json::Text("reference".into())),Json::Int(30)));
  assert!(matches!(a.get(&Json::Text("ordinary".into())),Json::Int(20)));
  assert!(reference.same_identity(&b.cell(&Json::Text("reference".into())).unwrap()));
  assert!(!a.cell(&Json::Text("ordinary".into())).unwrap().same_identity(&b.cell(&Json::Text("ordinary".into())).unwrap()));
 }
 #[test] fn float_keys_match_php_integer_wrapping(){
  let cases=[(f64::INFINITY,0), (f64::NEG_INFINITY,0), (f64::NAN,0),
   (3.9,3),(-3.9,-3),(1e19,-8446744073709551616),
   (-1e19,8446744073709551616),(1e20,7766279631452241920)];
  for (value,expected) in cases {
   assert_eq!(PhpKey::from_value(&Json::Number(value)).unwrap(),PhpKey::Int(expected));
  }
 }
 #[test] fn mixed_keys_and_numeric_strings(){
  let a=PhpArray::from_parts(vec![
   (Some(Json::Int(5)),Json::Text("five".into())),
   (Some(Json::Text("8".into())),Json::Text("eight".into())),
   (Some(Json::Text("08".into())),Json::Text("zero-eight".into())),
   (None,Json::Text("last".into()))]);
  assert!(matches!(a.get(&Json::Int(8)),Json::Text(s) if s=="eight"));
  assert!(matches!(a.get(&Json::Text("08".into())),Json::Text(s) if s=="zero-eight"));
  assert!(matches!(a.get(&Json::Int(9)),Json::Text(s) if s=="last"));
 }
 #[test] fn sparse_assignment_and_append(){
  let mut a=PhpArray::new();a.set(&Json::Int(100),Json::Int(1));a.append(Json::Int(2));
  assert!(matches!(a.get(&Json::Int(101)),Json::Int(2)));
  assert!(matches!(a.get(&Json::Int(0)),Json::Null));
 }
 #[test] fn replacement_preserves_order(){
  let mut a=PhpArray::new();a.set(&Json::Text("a".into()),Json::Int(1));a.set(&Json::Int(3),Json::Int(2));
  a.set(&Json::Text("a".into()),Json::Int(5));
  assert_eq!(a.entries.len(),2);
  assert_eq!(a.entries[0].0,PhpKey::String("a".into()));
 }
 #[test] fn deletion_keeps_append_history(){
  let mut a=PhpArray::new();a.set(&Json::Int(5),Json::Text("old".into()));
  assert!(a.remove(&Json::Int(5)));a.append(Json::Text("new".into()));
  assert!(matches!(a.get(&Json::Int(6)),Json::Text(s) if s=="new"));
  assert!(matches!(a.get(&Json::Int(5)),Json::Null));
 }
 #[test] fn deletion_preserves_remaining_order(){
  let mut a=PhpArray::new();a.set(&Json::Text("first".into()),Json::Int(1));
  a.set(&Json::Text("middle".into()),Json::Int(2));a.set(&Json::Text("last".into()),Json::Int(3));
  assert!(a.remove(&Json::Text("middle".into())));
  assert_eq!(a.entries.iter().map(|(k,_)|k.clone()).collect::<Vec<_>>(),vec![PhpKey::String("first".into()),PhpKey::String("last".into())]);
 }
 #[test] fn negative_index_and_overflow(){
  let mut a=PhpArray::new();a.set(&Json::Int(-5),Json::Null);a.append(Json::Int(4));
  assert!(matches!(a.get(&Json::Int(-4)),Json::Int(4)));
  let mut b=PhpArray::new();b.set(&Json::Int(i64::MAX),Json::Null);
  assert_eq!(b.next,None);
 }
}

#[cfg(test)]
mod php_array_scalar_loose_equality_tests {
 use super::*;
 #[test] fn php_array_boolean_and_null_loose_comparisons() {
  let empty=Json::PhpArray(PhpArray::new());
  let populated=Json::PhpArray(PhpArray::from_parts(vec![(None,Json::Int(1))]));
  for (lhs,rhs,expected) in [
   (&empty,&Json::Null,true),(&empty,&Json::Bool(false),true),
   (&empty,&Json::Bool(true),false),(&populated,&Json::Bool(true),true),
   (&populated,&Json::Bool(false),false),(&populated,&Json::Null,false),
   (&empty,&Json::Int(0),false),(&empty,&Json::Text(String::new()),false),
  ] {
   assert_eq!(lhs.phix_loose_eq(rhs),expected);
   assert_eq!(rhs.phix_loose_eq(lhs),expected);
  }
 }
}

#[cfg(test)]
mod php_array_key_boundary_tests {
 use super::*;
 #[test]
 fn numeric_string_key_boundaries_match_php() {
  let cases = [
   ("0", PhpKey::Int(0)), ("-1", PhpKey::Int(-1)),
   ("9223372036854775807", PhpKey::Int(i64::MAX)),
   ("-9223372036854775808", PhpKey::Int(i64::MIN)),
   ("00", PhpKey::String("00".into())),
   ("01", PhpKey::String("01".into())),
   ("-0", PhpKey::String("-0".into())),
   ("+1", PhpKey::String("+1".into())),
   (" 1", PhpKey::String(" 1".into())),
   ("1 ", PhpKey::String("1 ".into())),
   ("1.0", PhpKey::String("1.0".into())),
   ("1e2", PhpKey::String("1e2".into())),
   ("9223372036854775808", PhpKey::String("9223372036854775808".into())),
   ("-9223372036854775809", PhpKey::String("-9223372036854775809".into())),
  ];
  for (input, expected) in cases {
   assert_eq!(PhpKey::from_value(&Json::Text(input.into())).unwrap(), expected, "{input}");
  }
 }
 #[test]
 fn converted_keys_collide_but_preserve_first_insertion_position() {
  let array = PhpArray::from_parts(vec![
   (Some(Json::Text("1".into())), Json::Int(1)),
   (Some(Json::Text("01".into())), Json::Int(2)),
   (Some(Json::Bool(true)), Json::Int(3)),
   (Some(Json::Null), Json::Int(4)),
   (Some(Json::Text("".into())), Json::Int(5)),
  ]);
  assert_eq!(array.entries.len(),3);
  assert_eq!(array.entries[0].0, PhpKey::Int(1));
  assert_eq!(array.entries[1].0, PhpKey::String("01".into()));
  assert_eq!(array.entries[2].0, PhpKey::String("".into()));
  assert!(matches!(array.get(&Json::Int(1)), Json::Int(3)));
  assert!(matches!(array.get(&Json::Null), Json::Int(5)));
 }
}

#[cfg(test)]
mod php_array_union_tests {
 use super::*;
 #[test] fn union_keeps_left_values_and_order_and_appends_new_right_keys(){
  let left=PhpArray::from_parts(vec![(Some(Json::Int(2)),Json::Int(20)),(Some(Json::Text("x".into())),Json::Int(1))]);
  let right=PhpArray::from_parts(vec![(Some(Json::Text("x".into())),Json::Int(99)),(Some(Json::Int(4)),Json::Int(40))]);
  let result=left.union(&right);
  assert_eq!(result.entries.iter().map(|(k,_)|k.clone()).collect::<Vec<_>>(),vec![PhpKey::Int(2),PhpKey::String("x".into()),PhpKey::Int(4)]);
  assert!(matches!(result.get(&Json::Text("x".into())),Json::Int(1)));
  assert!(matches!(result.get(&Json::Int(4)),Json::Int(40)));
 }
 #[test] fn union_preserves_reference_aliases_but_copies_ordinary_cells(){
  let mut left=PhpArray::new();left.set(&Json::Int(0),Json::Int(1));
  let mut right=PhpArray::new();right.set(&Json::Text("ref".into()),Json::Int(2));
  let reference=right.cell(&Json::Text("ref".into())).unwrap();reference.mark_reference();
  let mut result=left.union(&right);
  result.set(&Json::Text("ref".into()),Json::Int(3));
  result.set(&Json::Int(0),Json::Int(9));
  assert!(matches!(right.get(&Json::Text("ref".into())),Json::Int(3)));
  assert!(matches!(left.get(&Json::Int(0)),Json::Int(1)));
 }
 #[test] fn union_keeps_numeric_append_history(){
  let left=PhpArray::from_parts(vec![(Some(Json::Int(5)),Json::Int(1))]);
  let right=PhpArray::from_parts(vec![(Some(Json::Int(9)),Json::Int(2))]);
  let mut union=left.union(&right);union.append(Json::Int(3));
  assert!(matches!(union.get(&Json::Int(10)),Json::Int(3)));
 }
}
