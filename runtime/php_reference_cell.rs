// Experimental identity-preserving reference cells.
//
// Unlike path-based references, a cell survives removal from a container.
// Used by PHP array entries and reference-aware compiled website values.

#[derive(Clone)]
pub(crate) struct PhpCell {
    value: std::sync::Arc<std::sync::Mutex<Json>>,
    referenced: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

// Never recursively format the contained value: a PHP array may contain a
// reference to itself. Deriving Debug here would overflow the stack.
impl std::fmt::Debug for PhpCell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PhpCell")
            .field("identity", &self.identity())
            .field("referenced", &self.referenced.load(std::sync::atomic::Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl PhpCell {
    pub(crate) fn new(value: Json) -> Self { Self { value: std::sync::Arc::new(std::sync::Mutex::new(value)), referenced: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)) } }
    pub(crate) fn alias(&self) -> Self { Self { value: std::sync::Arc::clone(&self.value), referenced: std::sync::Arc::clone(&self.referenced) } }
    pub(crate) fn read(&self) -> Json { self.value.lock().expect("PHP cell mutex poisoned").clone() }
    pub(crate) fn write(&self, value: Json) { *self.value.lock().expect("PHP cell mutex poisoned") = value; }
    pub(crate) fn with_mut<R>(&self, f: impl FnOnce(&mut Json) -> R) -> R {
        let mut guard = self.value.lock().expect("PHP cell mutex poisoned");
        f(&mut guard)
    }
    /// PHP references inside copied arrays retain identity; ordinary values copy.
    pub(crate) fn mark_reference(&self) { self.referenced.store(true, std::sync::atomic::Ordering::Relaxed); }
    pub(crate) fn clone_for_array_copy(&self) -> Self {
        if self.referenced.load(std::sync::atomic::Ordering::Relaxed) { self.alias() }
        else { Self::new(self.read()) }
    }
    /// Stable identity for cycle detection; never dereferences the cell.
    pub(crate) fn identity(&self) -> usize { std::sync::Arc::as_ptr(&self.value) as usize }
    pub(crate) fn same_identity(&self, other: &Self) -> bool { std::sync::Arc::ptr_eq(&self.value, &other.value) }
}

/// Reference-aware array container. Removing an entry only removes the
/// container's ownership; aliases to that entry remain valid.
#[derive(Default, Debug)]
pub(crate) struct PhpCellEntries {
    entries: Vec<(PhpKey, PhpCell)>,
}

impl PhpCellEntries {
    pub(crate) fn set(&mut self, key: PhpKey, value: Json) {
        if let Some((_, cell)) = self.entries.iter().find(|(k, _)| *k == key) {
            cell.write(value);
        } else {
            self.entries.push((key, PhpCell::new(value)));
        }
    }
    pub(crate) fn alias(&self, key: &PhpKey) -> Option<PhpCell> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, c)| c.alias())
    }
    pub(crate) fn remove(&mut self, key: &PhpKey) -> bool {
        if let Some(i) = self.entries.iter().position(|(k, _)| k == key) {
            self.entries.remove(i);
            true
        } else { false }
    }
    pub(crate) fn len(&self) -> usize { self.entries.len() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn debugging_self_referential_cell_is_cycle_safe() {
        let cell = PhpCell::new(Json::Null);
        cell.mark_reference();
        let mut array = PhpArray::new();
        array.set(&Json::Text("value".into()), Json::Int(1));
        let cyclic = PhpArray::from_cell_parts(vec![(Some(Json::Text("self".into())), cell.alias())]);
        cell.write(Json::PhpArray(cyclic));
        let debug = format!("{:?}", cell);
        assert!(debug.starts_with("PhpCell { identity:"));
        assert!(debug.contains("referenced: true"));
        assert!(debug.len() < 200);
    }
    #[test]
    fn debugging_array_with_cycle_does_not_traverse_referenced_value() {
        let cell = PhpCell::new(Json::Null);
        cell.mark_reference();
        let array = PhpArray::from_cell_parts(vec![(Some(Json::Text("self".into())), cell.alias())]);
        cell.write(Json::PhpArray(array.clone()));
        let debug = format!("{:?}", array);
        assert!(debug.contains("PhpCell"));
        assert!(debug.len() < 500);
    }
    #[test]
    fn alias_survives_element_removal() {
        let key = PhpKey::String("name".into());
        let mut entries = PhpCellEntries::default();
        entries.set(key.clone(), Json::Text("Alex".into()));
        let alias = entries.alias(&key).unwrap();
        assert!(entries.remove(&key));
        assert_eq!(entries.len(), 0);
        alias.write(Json::Text("John".into()));
        assert_eq!(alias.read().as_php_string(), "John");
        assert!(entries.alias(&key).is_none());
    }
    #[test]
    fn replacing_key_after_unset_creates_new_identity() {
        let key = PhpKey::Int(5);
        let mut entries = PhpCellEntries::default();
        entries.set(key.clone(), Json::Int(1));
        let old = entries.alias(&key).unwrap();
        entries.remove(&key);
        entries.set(key.clone(), Json::Int(2));
        let new = entries.alias(&key).unwrap();
        assert!(!old.same_identity(&new));
        old.write(Json::Int(7));
        assert_eq!(new.read().as_php_string(), "2");
    }
    #[test]
    fn writing_existing_entry_preserves_alias_identity() {
        let key = PhpKey::String("x".into());
        let mut entries = PhpCellEntries::default();
        entries.set(key.clone(), Json::Int(1));
        let alias = entries.alias(&key).unwrap();
        entries.set(key.clone(), Json::Int(9));
        assert_eq!(alias.read().as_php_string(), "9");
        assert!(alias.same_identity(&entries.alias(&key).unwrap()));
    }
}
