//! Single inventory of PHP-compatible built-ins recognized by the compiler.
//! This registry is metadata only: implementations remain in their category modules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Category { Strings, Assets, Filesystem }
#[derive(Debug, Clone, Copy)]
pub(crate) struct Builtin {
    pub name: &'static str,
    pub category: Category,
    pub min_args: usize,
    pub max_args: usize,
}
pub(crate) const BUILTINS: &[Builtin] = &[
    Builtin { name: "htmlspecialchars", category: Category::Strings, min_args: 1, max_args: 1 },
    Builtin { name: "asset_text", category: Category::Assets, min_args: 1, max_args: 1 },
    Builtin { name: "asset_exists", category: Category::Assets, min_args: 1, max_args: 1 },
    Builtin { name: "asset_size", category: Category::Assets, min_args: 1, max_args: 1 },
    Builtin { name: "asset_bytes", category: Category::Assets, min_args: 1, max_args: 1 },
    Builtin { name: "asset_write", category: Category::Assets, min_args: 1, max_args: 1 },
    Builtin { name: "move_uploaded_file", category: Category::Filesystem, min_args: 2, max_args: 2 },
];
pub(crate) fn lookup(name: &str) -> Option<&'static Builtin> {
    BUILTINS.iter().find(|entry| entry.name == name)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_unique_and_valid() {
        let mut names = std::collections::HashSet::new();
        for builtin in BUILTINS {
            assert!(names.insert(builtin.name), "duplicate built-in: {}", builtin.name);
            assert!(!builtin.name.is_empty());
            assert!(builtin.min_args <= builtin.max_args);
            assert!(builtin.name.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'));
        }
    }
    #[test]
    fn known_and_unknown_functions() {
        assert_eq!(lookup("htmlspecialchars").unwrap().category, Category::Strings);
        assert_eq!(lookup("move_uploaded_file").unwrap().min_args, 2);
        assert!(lookup("unimplemented_function").is_none());
    }
}
