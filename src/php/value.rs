//! Shared PHP-style value semantics (foundation, not yet used by codegen).
//! No external dependencies: ordered PHP array keys and values.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(PhpArray),
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ArrayKey { Int(i64), String(String) }
/// Preserves insertion order and the historical next numeric index.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PhpArray {
    entries: Vec<(ArrayKey, Value)>,
    next_index: Option<i64>, // None means next append would overflow i64.
    has_numeric_history: bool,
}
impl PhpArray {
    fn new() -> Self { Self { entries: Vec::new(), next_index: Some(0), has_numeric_history: false } }
    fn track_explicit_key(&mut self, key: &ArrayKey) {
        if let ArrayKey::Int(key) = key {
            if !self.has_numeric_history {
                // PHP 8.3+: the first negative key establishes the next index.
                self.next_index = key.checked_add(1);
            } else if let Some(next) = self.next_index {
                if *key >= next { self.next_index = key.checked_add(1); }
            }
            self.has_numeric_history = true;
        }
    }
    fn set(&mut self, key: ArrayKey, value: Value) {
        self.track_explicit_key(&key);
        if let Some((_, old)) = self.entries.iter_mut().find(|(k, _)| *k == key) { *old = value; }
        else { self.entries.push((key, value)); }
    }
    fn append(&mut self, value: Value) -> Result<ArrayKey, &'static str> {
        let next = self.next_index.ok_or("array next index overflow")?;
        let key = ArrayKey::Int(next);
        self.set(key.clone(), value);
        Ok(key)
    }
    fn remove(&mut self, key: &ArrayKey) -> Option<Value> {
        let pos = self.entries.iter().position(|(k, _)| k == key)?;
        // Deletion does not reset the historical append index.
        Some(self.entries.remove(pos).1)
    }
}

impl ArrayKey {
    /// PHP converts only canonical decimal integer strings to integer keys.
    pub(crate) fn from_php_value(value: &Value) -> Result<Self, &'static str> {
        match value {
            Value::Int(n) => Ok(Self::Int(*n)),
            Value::Bool(b) => Ok(Self::Int(i64::from(*b))),
            Value::Null => Ok(Self::String(String::new())),
            Value::Float(n) if n.is_finite() => Ok(Self::Int(*n as i64)),
            Value::Float(_) => Err("non-finite float array key not supported"),
            Value::String(s) => {
                if is_php_integer_key(s) {
                    if let Ok(n) = s.parse::<i64>() { return Ok(Self::Int(n)); }
                }
                Ok(Self::String(s.clone()))
            }
            Value::Array(_) => Err("illegal offset type: array"),
        }
    }
}
fn is_php_integer_key(s: &str) -> bool {
    let digits = s.strip_prefix('-').unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) { return false; }
    if digits.len() > 1 && digits.starts_with('0') { return false; }
    if s == "-0" { return false; }
    true
}
impl Value {
    pub(crate) fn truthy(&self) -> bool {
        match self {
            Self::Null => false,
            Self::Bool(v) => *v,
            Self::Int(v) => *v != 0,
            Self::Float(v) => *v != 0.0,
            Self::String(v) => !v.is_empty() && v != "0",
            Self::Array(v) => !v.entries.is_empty(),
        }
    }
    pub(crate) fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null", Self::Bool(_) => "bool", Self::Int(_) => "int",
            Self::Float(_) => "float", Self::String(_) => "string", Self::Array(_) => "array",
        }
    }
    pub(crate) fn php_string(&self) -> String {
        match self {
            Self::Null => String::new(),
            Self::Bool(false) => String::new(),
            Self::Bool(true) => "1".into(),
            Self::Int(v) => v.to_string(),
            Self::Float(v) => v.to_string(),
            Self::String(v) => v.clone(),
            Self::Array(_) => "Array".into(), // PHP also emits a warning in this case.
        }
    }
    pub(crate) fn array() -> Self { Self::Array(PhpArray::new()) }
    pub(crate) fn get(&self, key: &ArrayKey) -> Option<&Self> {
        match self { Self::Array(items) => items.entries.iter().find(|(k,_)| k == key).map(|(_,v)|v), _ => None }
    }
    pub(crate) fn set(&mut self, key: ArrayKey, value: Self) -> Result<(), &'static str> {
        match self { Self::Array(array) => { array.set(key, value); Ok(()) }, _ => Err("value is not an array") }
    }
    pub(crate) fn remove(&mut self, key: &ArrayKey) -> Result<Option<Self>, &'static str> {
        match self { Self::Array(array) => Ok(array.remove(key)), _ => Err("value is not an array") }
    }
    /// Appends using historical next index, including after deletions.
    pub(crate) fn append(&mut self, value: Self) -> Result<ArrayKey, &'static str> {
        match self { Self::Array(array) => array.append(value), _ => Err("value is not an array") }
    }
    pub(crate) fn len(&self) -> Option<usize> {
        match self { Self::Array(items) => Some(items.entries.len()), _ => None }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn truthiness_matches_php_scalar_rules() {
        for value in [Value::Null,Value::Bool(false),Value::Int(0),Value::Float(0.0),Value::String("".into()),Value::String("0".into()),Value::array()] {
            assert!(!value.truthy(), "{value:?}");
        }
        for value in [Value::Bool(true),Value::Int(-1),Value::Float(-0.5),Value::String("00".into()),Value::String("false".into())] {
            assert!(value.truthy(), "{value:?}");
        }
    }
    #[test] fn array_insertion_order_and_replacement() {
        let mut a=Value::array();
        a.set(ArrayKey::String("name".into()),Value::String("A".into())).unwrap();
        a.set(ArrayKey::Int(3),Value::Int(10)).unwrap();
        a.set(ArrayKey::String("name".into()),Value::String("B".into())).unwrap();
        assert_eq!(a.len(),Some(2));
        assert_eq!(a.get(&ArrayKey::String("name".into())),Some(&Value::String("B".into())));
        assert!(matches!(a,Value::Array(ref entries) if entries.entries[0].0==ArrayKey::String("name".into())));
    }
    #[test] fn scalar_string_conversion() {
        assert_eq!(Value::Null.php_string(),"");
        assert_eq!(Value::Bool(false).php_string(),"");
        assert_eq!(Value::Bool(true).php_string(),"1");
        assert_eq!(Value::Int(42).php_string(),"42");
        assert_eq!(Value::String("abc".into()).php_string(),"abc");
    }
    #[test] fn php_array_key_coercion() {
        for (text, expected) in [("8",ArrayKey::Int(8)),("-8",ArrayKey::Int(-8)),("0",ArrayKey::Int(0)),("08",ArrayKey::String("08".into())),("+8",ArrayKey::String("+8".into())),("-0",ArrayKey::String("-0".into())),(" 8",ArrayKey::String(" 8".into())),("9223372036854775808",ArrayKey::String("9223372036854775808".into()))] {
            assert_eq!(ArrayKey::from_php_value(&Value::String(text.into())),Ok(expected));
        }
        assert_eq!(ArrayKey::from_php_value(&Value::Null),Ok(ArrayKey::String(String::new())));
        assert_eq!(ArrayKey::from_php_value(&Value::Bool(true)),Ok(ArrayKey::Int(1)));
        assert_eq!(ArrayKey::from_php_value(&Value::Float(3.9)),Ok(ArrayKey::Int(3)));
        assert!(ArrayKey::from_php_value(&Value::array()).is_err());
    }
    #[test] fn array_append_and_overflow() {
        let mut a=Value::array();
        assert_eq!(a.append(Value::Int(1)),Ok(ArrayKey::Int(0)));
        a.set(ArrayKey::Int(5),Value::Int(2)).unwrap();
        assert_eq!(a.append(Value::Int(3)),Ok(ArrayKey::Int(6)));
        a.set(ArrayKey::Int(i64::MAX),Value::Null).unwrap();
        assert!(a.append(Value::Null).is_err());
    }
    #[test] fn deletion_keeps_historical_append_index() {
        let mut a = Value::array();
        a.set(ArrayKey::Int(5), Value::Int(1)).unwrap();
        assert_eq!(a.remove(&ArrayKey::Int(5)).unwrap(), Some(Value::Int(1)));
        assert_eq!(a.append(Value::Int(2)), Ok(ArrayKey::Int(6)));
        assert_eq!(a.len(), Some(1));
    }
    #[test] fn insertion_order_and_key_replacement() {
        let mut a = Value::array();
        a.set(ArrayKey::Int(5), Value::Null).unwrap();
        a.set(ArrayKey::Int(2), Value::Null).unwrap();
        a.set(ArrayKey::Int(5), Value::Bool(true)).unwrap();
        assert_eq!(a.append(Value::Null), Ok(ArrayKey::Int(6)));
        assert!(matches!(a, Value::Array(ref arr) if arr.entries.iter().map(|(k,_)| k.clone()).collect::<Vec<_>>() == vec![ArrayKey::Int(5),ArrayKey::Int(2),ArrayKey::Int(6)]));
    }
    #[test] fn negative_keys_follow_php_83_plus() {
        let mut a = Value::array();
        a.set(ArrayKey::Int(-5), Value::Null).unwrap();
        assert_eq!(a.append(Value::Null), Ok(ArrayKey::Int(-4)));
        let mut b = Value::array();
        b.set(ArrayKey::Int(5), Value::Null).unwrap();
        b.set(ArrayKey::Int(-5), Value::Null).unwrap();
        assert_eq!(b.append(Value::Null), Ok(ArrayKey::Int(6)));
    }
    #[test] fn deletion_of_non_array_is_error() {
        assert!(Value::Null.remove(&ArrayKey::Int(0)).is_err());
    }
    #[test] fn type_names() {
        assert_eq!(Value::Float(1.5).type_name(),"float");
        assert_eq!(Value::array().type_name(),"array");
    }
}
