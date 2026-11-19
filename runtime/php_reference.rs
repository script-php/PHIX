// PHP array variable and element aliases share a root binding and an optional path.
#[derive(Clone, Debug)]
pub(crate) struct PhpSlot { root: PhpCell, path: Vec<Json>, cell: Option<PhpCell> }
impl PhpSlot {
 pub(crate) fn new(value: Json)->Self {Self{root:PhpCell::new(value),path:Vec::new(),cell:None}}
 pub(crate) fn copy_value(&self)->Self {Self::new(self.read())}
 pub(crate) fn bind_reference(&self)->Self {Self{root:self.root.alias(),path:self.path.clone(),cell:self.cell.clone()}}
 pub(crate) fn bind_element(&self,keys:&[Json])->Self {
  if keys.is_empty(){return self.bind_reference();}
  if let Some(existing)=&self.cell {
   if let Some(cell)=existing.with_mut(|value|value.phix_ensure_path_cell(keys)) {
    cell.mark_reference();
    return Self{root:self.root.alias(),path:Vec::new(),cell:Some(cell)};
   }
  } else {
   let mut path=self.path.clone();path.extend_from_slice(keys);
   if let Some(cell)=self.root.with_mut(|value|value.phix_ensure_path_cell(&path)) {
    cell.mark_reference();
    return Self{root:self.root.alias(),path:Vec::new(),cell:Some(cell)};
   }
  }
  // Invalid reference paths retain the existing behavior until PHP error
  // semantics are implemented; they are tracked in PHP_COMPATIBILITY.md.
  let mut path=self.path.clone();path.extend_from_slice(keys);
  Self{root:self.root.alias(),path,cell:None}
 }
 pub(crate) fn reference_cell(&self)->Option<PhpCell>{
  if let Some(cell)=&self.cell {cell.mark_reference();return Some(cell.alias());}
  if self.path.is_empty(){self.root.mark_reference();return Some(self.root.alias());}
  self.root.with_mut(|value|value.phix_ensure_path_cell(&self.path)).map(|cell|{cell.mark_reference();cell})
 }
 pub(crate) fn read(&self)->Json {
  if let Some(cell)=&self.cell {return cell.read();}
  if self.path.is_empty(){self.root.read()}else{self.root.read().phix_index_path(&self.path)}
 }
 pub(crate) fn replace(&self,value:Json){
  if let Some(cell)=&self.cell {cell.write(value);return;}
  if self.path.is_empty(){self.root.write(value)}else{self.root.with_mut(|root|root.phix_set_path(&self.path,value))}
 }
 pub(crate) fn set_path(&self,keys:&[Json],value:Json){
  if let Some(cell)=&self.cell {cell.with_mut(|v|v.phix_set_path(keys,value));return;}
  let mut path=self.path.clone();path.extend_from_slice(keys);self.root.with_mut(|root|root.phix_set_path(&path,value))
 }
 pub(crate) fn unset_path(&self,keys:&[Json]){
  if let Some(cell)=&self.cell {cell.with_mut(|v|v.phix_unset_path(keys));return;}
  let mut path=self.path.clone();path.extend_from_slice(keys);self.root.with_mut(|root|root.phix_unset_path(&path))
 }
 pub(crate) fn append_path(&self,keys:&[Json],value:Json){
  if let Some(cell)=&self.cell {cell.with_mut(|v|v.phix_append_path(keys,value));return;}
  let mut path=self.path.clone();path.extend_from_slice(keys);self.root.with_mut(|root|root.phix_append_path(&path,value))
 }
 pub(crate) fn same_binding(&self,other:&Self)->bool{(match (&self.cell,&other.cell){(Some(a),Some(b))=>return a.same_identity(b),(None,None)=>true,_=>false})&&self.root.same_identity(&other.root)&&self.path.len()==other.path.len()&&self.path.iter().zip(&other.path).all(|(a,b)|a.phix_strict_eq(b))}
}
#[cfg(test)]
mod reference_tests {
    use super::*;

    fn original() -> PhpSlot {
        PhpSlot::new(Json::PhpArray(PhpArray::from_parts(vec![(
            Some(Json::Text("count".into())), Json::Int(1),
        )])))
    }

    #[test]
    fn normal_array_assignment_is_independent() {
        let a = original();
        let b = a.copy_value();
        assert!(!a.same_binding(&b));
        b.set_path(&[Json::Text("count".into())], Json::Int(2));
        assert_eq!(a.read().phix_index(&Json::Text("count".into())).as_php_string(), "1");
        assert_eq!(b.read().phix_index(&Json::Text("count".into())).as_php_string(), "2");
    }

    #[test]
    fn reference_assignment_shares_mutations() {
        let a = original();
        let b = a.bind_reference();
        assert!(a.same_binding(&b));
        b.set_path(&[Json::Text("count".into())], Json::Int(5));
        assert_eq!(a.read().phix_index(&Json::Text("count".into())).as_php_string(), "5");
        a.unset_path(&[Json::Text("count".into())]);
        assert!(matches!(b.read().phix_index(&Json::Text("count".into())), Json::Null));
    }

    #[test]
    fn rebinding_a_copy_does_not_modify_original() {
        let a = original();
        let b = a.copy_value();
        b.replace(Json::Int(42));
        assert!(matches!(b.read(), Json::Int(42)));
        assert!(matches!(a.read(), Json::PhpArray(_)));
    }

    #[test]
    fn copied_nested_arrays_do_not_share_storage() {
        let a = PhpSlot::new(Json::PhpArray(PhpArray::from_parts(vec![(
            Some(Json::Text("child".into())),
            Json::PhpArray(PhpArray::from_parts(vec![(None, Json::Int(1))])),
        )])));
        let b = a.copy_value();
        b.set_path(&[Json::Text("child".into()), Json::Int(0)], Json::Int(99));
        let key = Json::Text("child".into());
        assert_eq!(a.read().phix_index(&key).phix_index(&Json::Int(0)).as_php_string(), "1");
        assert_eq!(b.read().phix_index(&key).phix_index(&Json::Int(0)).as_php_string(), "99");
    }
}

#[cfg(test)]
mod element_reference_tests {
    use super::*;
    fn key(s: &str) -> Json { Json::Text(s.to_owned()) }
    #[test]
    fn nested_element_alias_mutates_parent() {
        let root = PhpSlot::new(Json::PhpArray(PhpArray::new()));
        let alias = root.bind_element(&[key("profile"), key("name")]);
        alias.replace(key("John"));
        assert_eq!(root.read().phix_index_path(&[key("profile"), key("name")]).as_php_string(), "John");
    }
    #[test]
    fn copied_array_preserves_element_reference_alias() {
        let root = PhpSlot::new(Json::PhpArray(PhpArray::new()));
        let alias = root.bind_element(&[key("name")]);
        alias.replace(key("Alex"));
        let copied = root.copy_value();
        alias.replace(key("Jane"));
        assert_eq!(copied.read().phix_index(&key("name")).as_php_string(), "Jane");
        assert_eq!(root.read().phix_index(&key("name")).as_php_string(), "Jane");
    }
}

#[cfg(test)]
mod stable_cell_integration_tests {
 use super::*;
 fn key()->Json {Json::Text("name".into())}
 #[test] fn alias_survives_unset_and_reinsertion() {
  let a=PhpSlot::new(Json::PhpArray(PhpArray::new()));
  a.set_path(&[key()],Json::Text("Alex".into()));
  let alias=a.bind_element(&[key()]);
  a.unset_path(&[key()]);
  alias.replace(Json::Text("John".into()));
  assert!(matches!(a.read().phix_index(&key()),Json::Null));
  assert_eq!(alias.read().as_php_string(),"John");
  a.set_path(&[key()],Json::Text("Jane".into()));
  assert_eq!(a.read().phix_index(&key()).as_php_string(),"Jane");
  assert_eq!(alias.read().as_php_string(),"John");
 }
 #[test] fn normal_copy_does_not_share_element_cell() {
  let a=PhpSlot::new(Json::PhpArray(PhpArray::new()));
  a.set_path(&[key()],Json::Int(1));
  let b=a.copy_value();
  let alias=a.bind_element(&[key()]);
  alias.replace(Json::Int(9));
  assert_eq!(b.read().phix_index(&key()).as_php_string(),"1");
  assert_eq!(a.read().phix_index(&key()).as_php_string(),"9");
 }
}

#[cfg(test)]
mod nested_identity_regression {
 use super::*;
 fn k(s:&str)->Json{Json::Text(s.into())}
 #[test] fn nested_alias_survives_unset_and_reinsertion(){
  let root=PhpSlot::new(Json::PhpArray(PhpArray::from_parts(vec![(Some(k("parent")),Json::PhpArray(PhpArray::from_parts(vec![(Some(k("name")),Json::Text("Alex".into()))])))])));
  let alias=root.bind_element(&[k("parent"),k("name")]);
  root.unset_path(&[k("parent"),k("name")]);
  alias.replace(Json::Text("John".into()));
  root.set_path(&[k("parent"),k("name")],Json::Text("Jane".into()));
  assert_eq!(alias.read().as_php_string(),"John");
  assert_eq!(root.read().phix_index_path(&[k("parent"),k("name")]).as_php_string(),"Jane");
 }
 #[test] fn nested_alias_survives_parent_replacement(){
  let root=PhpSlot::new(Json::PhpArray(PhpArray::new()));
  let alias=root.bind_element(&[k("parent"),k("child")]);
  alias.replace(Json::Int(7));
  root.set_path(&[k("parent")],Json::PhpArray(PhpArray::new()));
  alias.replace(Json::Int(9));
  assert_eq!(alias.read().as_php_string(),"9");
  assert!(matches!(root.read().phix_index_path(&[k("parent"),k("child")]),Json::Null));
 }
 #[test] fn nested_aliases_share_original_cell(){
  let root=PhpSlot::new(Json::PhpArray(PhpArray::new()));
  let a=root.bind_element(&[k("parent"),k("child")]);
  let b=root.bind_element(&[k("parent"),k("child")]);
  assert!(a.same_binding(&b));
  a.replace(Json::Int(42));
  assert_eq!(b.read().as_php_string(),"42");
 }
}

#[cfg(test)]
mod nested_mutation_identity_tests {
 use super::*;
 fn k(s:&str)->Json{Json::Text(s.into())}
 #[test] fn nested_assignment_keeps_existing_reference_cell(){
  let a=PhpSlot::new(Json::PhpArray(PhpArray::new()));
  a.set_path(&[k("parent"),k("value")],Json::Int(1));
  let alias=a.bind_element(&[k("parent"),k("value")]);
  a.set_path(&[k("parent"),k("other")],Json::Int(2));
  alias.replace(Json::Int(7));
  assert_eq!(a.read().phix_index_path(&[k("parent"),k("value")]).as_php_string(),"7");
 }
 #[test] fn nested_unset_detaches_only_target_cell(){
  let a=PhpSlot::new(Json::PhpArray(PhpArray::new()));
  a.set_path(&[k("parent"),k("value")],Json::Int(1));
  let alias=a.bind_element(&[k("parent"),k("value")]);
  a.unset_path(&[k("parent"),k("value")]);
  alias.replace(Json::Int(9));
  a.set_path(&[k("parent"),k("value")],Json::Int(3));
  assert_eq!(alias.read().as_php_string(),"9");
  assert_eq!(a.read().phix_index_path(&[k("parent"),k("value")]).as_php_string(),"3");
 }
 #[test] fn nested_append_preserves_sibling_reference(){
  let a=PhpSlot::new(Json::PhpArray(PhpArray::new()));
  a.set_path(&[k("parent"),k("value")],Json::Int(1));
  let alias=a.bind_element(&[k("parent"),k("value")]);
  a.append_path(&[k("parent")],Json::Int(5));
  alias.replace(Json::Int(8));
  assert_eq!(a.read().phix_index_path(&[k("parent"),k("value")]).as_php_string(),"8");
  assert_eq!(a.read().phix_index_path(&[k("parent"),Json::Int(0)]).as_php_string(),"5");
 }
}

#[cfg(test)]
mod nested_slot_mutation_regressions {
 use super::*;
 fn k(s:&str)->Json { Json::Text(s.into()) }
 fn setup()->(PhpSlot,PhpSlot,PhpSlot) {
  let root=PhpSlot::new(Json::PhpArray(PhpArray::new()));
  root.set_path(&[k("parent"),k("value")],Json::Int(1));
  let parent=root.bind_element(&[k("parent")]);
  let value=root.bind_element(&[k("parent"),k("value")]);
  (root,parent,value)
 }
 #[test] fn mutation_through_element_slot_preserves_sibling_alias() {
  let (root,parent,value)=setup();
  parent.set_path(&[k("other")],Json::Int(2));
  value.replace(Json::Int(7));
  assert_eq!(root.read().phix_index_path(&[k("parent"),k("value")]).as_php_string(),"7");
 }
 #[test] fn append_through_element_slot_preserves_sibling_alias() {
  let (root,parent,value)=setup();
  parent.append_path(&[],Json::Int(5));
  value.replace(Json::Int(8));
  assert_eq!(root.read().phix_index_path(&[k("parent"),k("value")]).as_php_string(),"8");
 }
 #[test] fn unset_through_element_slot_detaches_only_removed_alias() {
  let (root,parent,value)=setup();
  parent.unset_path(&[k("value")]);
  value.replace(Json::Int(9));
  parent.set_path(&[k("value")],Json::Int(3));
  assert_eq!(value.read().as_php_string(),"9");
  assert_eq!(root.read().phix_index_path(&[k("parent"),k("value")]).as_php_string(),"3");
 }
}

#[cfg(test)]
mod cycle_inspection_tests {
 use super::*;
 #[test]
 fn self_reference_is_detected_without_recursive_locking() {
  let root = PhpSlot::new(Json::PhpArray(PhpArray::new()));
  let alias = root.reference_cell().unwrap();
  root.set_path(&[Json::Text("self".into())], Json::Null);
  // Replace the newly-created element cell with the root's reference cell.
  root.root.with_mut(|value| {
   if let Json::PhpArray(array) = value {
    let key = PhpKey::String("self".into());
    array.entries.retain(|(k,_)| k != &key);
    array.entries.push((key, alias));
   }
  });
  assert!(root.read().phix_has_reference_cycle());
 }
 #[test]
 fn repeated_reference_without_cycle_is_not_recursive() {
  let shared = PhpCell::new(Json::Int(7));
  shared.mark_reference();
  let mut array = PhpArray::new();
  array.entries.push((PhpKey::Int(0), shared.alias()));
  array.entries.push((PhpKey::Int(1), shared));
  assert!(!Json::PhpArray(array).phix_has_reference_cycle());
 }
 #[test]
 fn ordinary_nested_arrays_are_not_recursive() {
  let nested = PhpArray::from_parts(vec![(None,Json::PhpArray(PhpArray::new()))]);
  assert!(!Json::PhpArray(nested).phix_has_reference_cycle());
 }
}

#[cfg(test)]
mod cyclic_array_copy_regressions {
 use super::*;
 fn self_referential_slot() -> PhpSlot {
  let root=PhpSlot::new(Json::PhpArray(PhpArray::new()));
  let root_ref=root.reference_cell().expect("root reference");
  root_ref.mark_reference();
  root.root.with_mut(|value| {
   if let Json::PhpArray(array)=value {
    *array=PhpArray::from_cell_parts(vec![(Some(Json::Text("self".into())),root_ref)]);
   }
  });
  root
 }
 #[test]
 fn copying_self_referential_array_keeps_reference_to_original_root() {
  let original=self_referential_slot();
  let copy=original.copy_value();
  copy.set_path(&[Json::Text("extra".into())],Json::Int(7));
  assert!(matches!(original.read().phix_index(&Json::Text("extra".into())),Json::Null));
  assert!(matches!(copy.read().phix_index_path(&[Json::Text("self".into()),Json::Text("extra".into())]),Json::Null));
  original.set_path(&[Json::Text("original".into())],Json::Int(9));
  assert!(matches!(copy.read().phix_index_path(&[Json::Text("self".into()),Json::Text("original".into())]),Json::Int(9)));
 }
 #[test]
 fn cyclic_copy_keeps_original_reference_identity_without_cycle_recursion() {
  let original=self_referential_slot();
  let copy=original.copy_value();
  let original_cell=original.reference_cell().unwrap();
  let copied_self=copy.read();
  let Json::PhpArray(copied_array)=copied_self else {panic!("expected array")};
  let self_cell=copied_array.cell(&Json::Text("self".into())).unwrap();
  assert!(self_cell.same_identity(&original_cell));
  assert!(original.read().phix_has_reference_cycle());
  assert!(copy.read().phix_has_reference_cycle());
 }
}

#[cfg(test)]
mod rebinding_regression_tests {
    use super::*;

    #[test]
    fn rebinding_variable_does_not_retarget_existing_aliases() {
        let a = PhpSlot::new(Json::Int(10));
        let b = PhpSlot::new(Json::Int(20));
        let mut reference = a.bind_reference();
        let original = reference.bind_reference();
        reference = b.bind_reference();
        reference.replace(Json::Int(30));
        assert_eq!(a.read().as_php_string(), "10");
        assert_eq!(original.read().as_php_string(), "10");
        assert_eq!(b.read().as_php_string(), "30");
        assert!(!original.same_binding(&reference));
    }

    #[test]
    fn rebinding_element_does_not_retarget_old_element_reference() {
        let key = Json::Text("value".into());
        let left = PhpSlot::new(Json::PhpArray(PhpArray::from_parts(vec![(Some(key.clone()), Json::Int(1))])));
        let right = PhpSlot::new(Json::PhpArray(PhpArray::from_parts(vec![(Some(key.clone()), Json::Int(2))])));
        let mut reference = left.bind_element(&[key.clone()]);
        let original = reference.bind_reference();
        reference = right.bind_element(&[key.clone()]);
        reference.replace(Json::Int(3));
        assert_eq!(left.read().phix_index(&key).as_php_string(), "1");
        assert_eq!(original.read().as_php_string(), "1");
        assert_eq!(right.read().phix_index(&key).as_php_string(), "3");
    }
}
