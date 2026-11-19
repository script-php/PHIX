//! Structural PHP expression syntax. This layer preserves source-level
//! evaluation order; it does not generate Rust or assign runtime types.
use crate::lexer::{Kind, Token};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Expr {
    Atom { kind: Kind, text: String },
    Unary { op: String, value: Box<Expr> },
    Binary { op: String, left: Box<Expr>, right: Box<Expr> },
    Conditional { condition: Box<Expr>, yes: Option<Box<Expr>>, no: Box<Expr> },
    Call { callee: Box<Expr>, args: Vec<Expr> },
    Index { target: Box<Expr>, key: Option<Box<Expr>> },
    Member { target: Box<Expr>, name: String },
    Array(Vec<(Option<Expr>, Expr)>),
}

pub(crate) struct ExpressionParser<'a> {
    tokens: &'a [Token],
    at: usize,
}

impl<'a> ExpressionParser<'a> {
    pub(crate) fn new(tokens: &'a [Token]) -> Self { Self { tokens, at: 0 } }
    pub(crate) fn consumed(&self) -> usize { self.at }
    fn current(&self) -> &str { self.tokens.get(self.at).map_or("<eof>", |t| t.text.as_str()) }
    fn take(&mut self, expected: &str) -> bool {
        if self.current() == expected { self.at += 1; true } else { false }
    }
    fn expect(&mut self, expected: &str) -> Result<(), String> {
        if self.take(expected) { Ok(()) } else { Err(format!("expected {expected}, found {}", self.current())) }
    }
    fn precedence(op: &str) -> Option<(u8, bool)> {
        Some(match op {
            "=" | "+=" | "??=" => (1, true),
            "??" => (3, true),
            "||" => (4, false), "&&" => (5, false),
            "==" | "===" | "!=" | "!==" => (6, false),
            "<" | ">" | "<=" | ">=" => (7, false),
            "+" | "-" | "." => (8, false),
            "*" | "/" => (9, false),
            _ => return None,
        })
    }
    pub(crate) fn parse(&mut self) -> Result<Expr, String> { self.expression(0) }
    fn expression(&mut self, min: u8) -> Result<Expr, String> {
        let mut left = self.prefix()?;
        let mut ternary_seen = false;
        loop {
            // Postfix expressions bind more tightly than every infix operator.
            if 11 >= min && self.take("(") {
                let mut args = Vec::new();
                if !self.take(")") {
                    loop {
                        args.push(self.expression(0)?);
                        if self.take(")") { break; }
                        self.expect(",")?;
                    }
                }
                left = Expr::Call { callee: Box::new(left), args };
                continue;
            }
            if 11 >= min && self.take("[") {
                let key = if self.current() == "]" { None } else { Some(Box::new(self.expression(0)?)) };
                self.expect("]")?;
                left = Expr::Index { target: Box::new(left), key };
                continue;
            }
            if 11 >= min && self.take("->") {
                let name = self.tokens.get(self.at).ok_or("missing property name")?;
                if name.kind != Kind::Id { return Err("property name must be an identifier".into()); }
                let name = name.text.clone(); self.at += 1;
                left = Expr::Member { target: Box::new(left), name };
                continue;
            }
            if min <= 2 && self.take("?") {
                if ternary_seen { return Err("PHP 8 requires parentheses around nested ternary expressions".into()); }
                ternary_seen = true;
                let yes = if self.current() == ":" { None } else { Some(Box::new(self.expression(0)?)) };
                self.expect(":")?;
                let no = Box::new(self.expression(3)?);
                left = Expr::Conditional { condition: Box::new(left), yes, no };
                continue;
            }
            let Some((precedence, right_assoc)) = Self::precedence(self.current()) else { break; };
            if precedence < min { break; }
            let op = self.current().to_owned(); self.at += 1;
            let right = self.expression(precedence + u8::from(!right_assoc))?;
            left = Expr::Binary { op, left: Box::new(left), right: Box::new(right) };
        }
        Ok(left)
    }
    fn prefix(&mut self) -> Result<Expr, String> {
        if matches!(self.current(), "!" | "-" | "+" | "throw") {
            let op = self.current().to_owned(); self.at += 1;
            return Ok(Expr::Unary { op, value: Box::new(self.expression(10)?) });
        }
        if self.take("(") {
            let value = self.expression(0)?; self.expect(")")?; return Ok(value);
        }
        if self.take("[") {
            let mut elements = Vec::new();
            if !self.take("]") {
                loop {
                    let first = self.expression(0)?;
                    let (key, value) = if self.take("=>") { (Some(first), self.expression(0)?) } else { (None, first) };
                    elements.push((key, value));
                    if self.take("]") { break; }
                    self.expect(",")?;
                    if self.take("]") { break; }
                }
            }
            return Ok(Expr::Array(elements));
        }
        let token = self.tokens.get(self.at).ok_or("missing expression")?;
        if !matches!(token.kind, Kind::Id | Kind::Var | Kind::Num | Kind::Str) {
            return Err(format!("expected expression, found {}", token.text));
        }
        self.at += 1;
        Ok(Expr::Atom { kind: token.kind.clone(), text: token.text.clone() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    fn parse(source: &str) -> Expr {
        let tokens = lex(source).unwrap();
        let mut parser = ExpressionParser::new(&tokens);
        let ast = parser.parse().unwrap();
        assert_eq!(parser.consumed(), tokens.len() - 1, "expression must consume all tokens");
        ast
    }
    #[test]
    fn nested_calls_and_arithmetic_preserve_tree() {
        let ast = parse("<?php outer(inner(10) + another(20))");
        let Expr::Call { args, .. } = ast else { panic!("outer call expected") };
        assert!(matches!(&args[0], Expr::Binary { op, .. } if op == "+"));
    }
    #[test]
    fn lazy_ternary_and_coalesce_are_structural() {
        let ast = parse("<?php $condition ? first() : ($fallback ?? other())");
        let Expr::Conditional { no, .. } = ast else { panic!("ternary expected") };
        assert!(matches!(*no, Expr::Binary { op, .. } if op == "??"));
        assert!(matches!(parse("<?php $a ?: $b"), Expr::Conditional { yes: None, .. }));
    }
    #[test]
    fn array_keys_writes_and_nested_indexes() {
        let ast = parse("<?php $data['user']['name'] ??= ['x' => inner(1), later()]");
        let Expr::Binary { op, left, right } = ast else { panic!("assignment expected") };
        assert_eq!(op, "??=");
        assert!(matches!(*left, Expr::Index { .. }));
        assert!(matches!(*right, Expr::Array(ref elements) if elements.len() == 2));
    }
    #[test]
    fn ambiguous_ternaries_are_rejected() {
        let tokens = lex("<?php $a ? $b : $c ? $d : $e").unwrap();
        assert!(ExpressionParser::new(&tokens).parse().is_err());
        parse("<?php ($a ? $b : $c) ? $d : $e");
    }
}

/// Semantic facts computed on the syntax tree before native lowering.
/// Effects are conservative: an unknown call or property access may throw.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Effects {
    pub may_throw: bool,
    pub may_write: bool,
    pub may_call: bool,
}
impl Effects {
    fn union(self, other: Self) -> Self {
        Self { may_throw: self.may_throw || other.may_throw,
               may_write: self.may_write || other.may_write,
               may_call: self.may_call || other.may_call }
    }
}
impl Expr {
    /// Determine whether an expression is a syntactically valid assignment target.
    /// Runtime checks for properties, array offsets and references are separate.
    pub(crate) fn assignable(&self) -> bool {
        match self {
            Self::Atom { kind: Kind::Var, .. } => true,
            Self::Index { target, .. } => target.assignable(),
            Self::Member { target, .. } => !matches!(**target, Self::Array(_)),
            _ => false,
        }
    }
    /// Reject invalid assignment targets and recursively validate all children.
    /// This pass is independent of the eventual Rust/native code backend.
    pub(crate) fn validate(&self) -> Result<(), String> {
        match self {
            Self::Atom { .. } => Ok(()),
            Self::Unary { value, .. } => value.validate(),
            Self::Binary { op, left, right } => {
                if matches!(op.as_str(), "=" | "+=" | "??=") && (!left.assignable() || (op == "??=" && matches!(**left, Self::Index { key: None, .. }))) {
                    return Err(format!("invalid assignment target for {op}"));
                }
                left.validate()?;
                right.validate()
            }
            Self::Conditional { condition, yes, no } => {
                condition.validate()?;
                if let Some(yes) = yes { yes.validate()?; }
                no.validate()
            }
            Self::Call { callee, args } => {
                callee.validate()?;
                for arg in args { arg.validate()?; }
                Ok(())
            }
            Self::Index { target, key } => {
                target.validate()?;
                if let Some(key) = key { key.validate()?; }
                Ok(())
            }
            Self::Member { target, .. } => target.validate(),
            Self::Array(elements) => {
                for (key, value) in elements {
                    if let Some(key) = key { key.validate()?; }
                    value.validate()?;
                }
                Ok(())
            }
        }
    }
    pub(crate) fn effects(&self) -> Effects {
        match self {
            Self::Atom { .. } => Effects::default(),
            Self::Unary { op, value } => {
                let mut effects = value.effects();
                if op == "throw" { effects.may_throw = true; }
                effects
            }
            Self::Binary { op, left, right } => {
                let mut effects = left.effects().union(right.effects());
                if matches!(op.as_str(), "=" | "+=" | "??=") { effects.may_write = true; }
                if matches!(op.as_str(), "/" | "+=") { effects.may_throw = true; }
                effects
            }
            Self::Conditional { condition, yes, no } => {
                let mut effects = condition.effects().union(no.effects());
                if let Some(yes) = yes { effects = effects.union(yes.effects()); }
                effects
            }
            Self::Call { callee, args } => {
                let mut effects = callee.effects();
                for arg in args { effects = effects.union(arg.effects()); }
                effects.may_throw = true;
                effects.may_call = true;
                effects
            }
            Self::Index { target, key } => {
                let mut effects = target.effects();
                if let Some(key) = key { effects = effects.union(key.effects()); }
                effects.may_throw = true;
                effects
            }
            Self::Member { target, .. } => {
                let mut effects = target.effects();
                effects.may_throw = true;
                effects
            }
            Self::Array(elements) => elements.iter().fold(Effects::default(), |effects, (key, value)| {
                let effects = if let Some(key) = key { effects.union(key.effects()) } else { effects };
                effects.union(value.effects())
            }),
        }
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;
    use crate::lexer::lex;
    fn parse(source: &str) -> Expr {
        let tokens = lex(&format!("<?php {source}")).unwrap();
        let mut parser = ExpressionParser::new(&tokens);
        let expr = parser.parse().unwrap();
        assert_eq!(parser.consumed(), tokens.len() - 1);
        expr
    }
    #[test]
    fn rejects_non_lvalues_without_special_casing_nested_syntax() {
        for source in ["(1 + 2) = 3", "foo() += 2", "($x ?: $y) ??= 1", "$a[] ??= 1"] {
            assert!(parse(source).validate().is_err(), "{source}");
        }
        for source in ["$a = 3", "$a[] = 1", "$a['x'] ??= 4", "$a['x']['y'] += 1", "$obj->value = 1"] {
            assert!(parse(source).validate().is_ok(), "{source}");
        }
    }
    #[test]
    fn tracks_nested_call_throw_and_write_effects() {
        let effects = parse("$result = outer(inner(10) + another(20))").effects();
        assert!(effects.may_call && effects.may_throw && effects.may_write);
        let effects = parse("$flag ? first() : $fallback ?? 5").effects();
        assert!(effects.may_call && effects.may_throw);
        assert!(!effects.may_write);
    }
}
