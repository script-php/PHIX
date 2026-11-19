//! Native compiler IR for explicit function exits. This is an analysis IR, not
//! a PHP interpreter: control transfers are edges, never fabricated values.
use crate::lexer::Token;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Exit { Fallthrough, Return, Throw }

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Terminator {
    Next(usize),
    Return,
    Throw,
    Branch { yes: usize, no: usize },
    /// A potentially throwing operation has separate normal and exceptional successors.
    Invoke { normal: usize, unwind: usize },
    Exit,
}

#[derive(Clone, Debug)]
pub(crate) struct Block { pub terminator: Terminator }

#[derive(Clone, Debug)]
pub(crate) struct FunctionIr { pub blocks: Vec<Block>, pub entry: usize }

impl FunctionIr {
    /// Validate every control-flow target before code generation. Invalid
    /// edges must be compiler errors, never silently treated as exits.
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.blocks.is_empty() { return Err("IR has no basic blocks".into()); }
        if self.entry >= self.blocks.len() { return Err("IR entry is out of bounds".into()); }
        for (index, block) in self.blocks.iter().enumerate() {
            let check = |target: usize| -> Result<(), String> {
                if target >= self.blocks.len() {
                    Err(format!("IR block {index} has invalid successor {target}"))
                } else { Ok(()) }
            };
            match block.terminator {
                Terminator::Next(target) => check(target)?,
                Terminator::Branch { yes, no } => { check(yes)?; check(no)?; }
                Terminator::Invoke { normal, unwind } => { check(normal)?; check(unwind)?; }
                Terminator::Return | Terminator::Throw | Terminator::Exit => {}
            }
        }
        Ok(())
    }
    /// Follow explicit control-flow edges, accounting for cycles. A function
    /// may fall through if an Exit node is reachable from its entry.
    pub(crate) fn may_fall_through(&self) -> bool {
        let mut visited = vec![false; self.blocks.len()];
        let mut stack = vec![self.entry];
        while let Some(index) = stack.pop() {
            if index >= self.blocks.len() || visited[index] { continue; }
            visited[index] = true;
            match self.blocks[index].terminator {
                Terminator::Exit => return true,
                Terminator::Next(next) => stack.push(next),
                Terminator::Branch { yes, no } => { stack.push(yes); stack.push(no); }
                Terminator::Invoke { normal, unwind } => { stack.push(normal); stack.push(unwind); }
                Terminator::Return | Terminator::Throw => {},
            }
        }
        false
    }
}

/// Lower supported lexical statement control flow into basic blocks. This
/// analysis is deliberately conservative: an unrecognized construct is an
/// opaque statement that can fall through, never proof of a return.
/// Conditional branches are represented explicitly, including nested if/else.
pub(crate) fn lower_terminal_body(tokens: &[Token]) -> Result<FunctionIr, String> {
    let words: Vec<&str> = tokens.iter().map(|t| t.text.as_str()).collect();
    if words.first() != Some(&"{") || words.last() != Some(&"}") {
        return Err("expected braced function body".into());
    }
    fn matching(w: &[&str], start: usize, open: &str, close: &str) -> Result<usize, String> {
        if w.get(start) != Some(&open) { return Err(format!("expected {open}")); }
        let mut depth = 0;
        for i in start..w.len() {
            if w[i] == open { depth += 1; }
            if w[i] == close {
                depth -= 1;
                if depth == 0 { return Ok(i); }
            }
        }
        Err(format!("unterminated {open}"))
    }
    #[derive(Debug)] enum Stmt { Return, Throw, Opaque, Invoke { returns: bool }, Branch(Vec<Stmt>, Vec<Stmt>, bool) }
    fn parse(w: &[&str], mut i: usize, end: usize) -> Result<Vec<Stmt>, String> {
        let mut stmts = Vec::new();
        while i < end {
            if w[i] == ";" { i += 1; continue; }
            if w[i] == "if" && w.get(i + 1) == Some(&"(") {
                // Parse a complete if / elseif / else chain. A conditional
                // without an else has an implicit fallthrough path. This is
                // structural analysis only: expression evaluation is lowered
                // separately by the compiler.
                fn parse_if(w: &[&str], i: &mut usize, end: usize) -> Result<Stmt, String> {
                    if *i >= end || w.get(*i) != Some(&"if") && w.get(*i) != Some(&"elseif") {
                        return Err("expected conditional branch".into());
                    }
                    *i += 1;
                    let close = matching(w, *i, "(", ")")?;
                    // The condition itself may call PHP functions and throw
                    // before either branch executes. Preserve that edge in IR.
                    let condition_invokes = (*i + 1..close).any(|k| w[k] == "throw") || (*i + 1..close).any(|k| {
                        w.get(k + 1) == Some(&"(")
                            && w[k].chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                            && !matches!(w[k], "isset" | "empty")
                    });
                    *i = close + 1;
                    if w.get(*i) != Some(&"{") {
                        return Err("conditional IR analysis requires braced branches".into());
                    }
                    let yes_end = matching(w, *i, "{", "}")?;
                    let yes = parse(w, *i + 1, yes_end)?;
                    *i = yes_end + 1;
                    let no = if w.get(*i) == Some(&"elseif") ||
                        (w.get(*i) == Some(&"else") && w.get(*i + 1) == Some(&"if")) {
                        if w[*i] == "else" { *i += 1; }
                        vec![parse_if(w, i, end)?]
                    } else if w.get(*i) == Some(&"else") {
                        *i += 1;
                        if w.get(*i) != Some(&"{") {
                            return Err("conditional IR analysis requires braced else".into());
                        }
                        let no_end = matching(w, *i, "{", "}")?;
                        let no = parse(w, *i + 1, no_end)?;
                        *i = no_end + 1;
                        no
                    } else { Vec::new() };
                    Ok(Stmt::Branch(yes, no, condition_invokes))
                }
                stmts.push(parse_if(w, &mut i, end)?);
                continue;
            }
            // Opaque constructs (including loops, try and switch) are never
            // interpreted as a guaranteed return, even when nested returns exist.
            let is_return = w[i] == "return";
            let is_throw = w[i] == "throw" || (w[i] == "return" && w.get(i + 1) == Some(&"throw"));
            let mut parens = 0usize;
            let mut brackets = 0usize;
            let mut j = i;
            while j < end {
                match w[j] {
                    "(" => parens += 1,
                    ")" => parens = parens.saturating_sub(1),
                    "[" => brackets += 1,
                    "]" => brackets = brackets.saturating_sub(1),
                    "{" if parens == 0 && brackets == 0 => {
                        j = matching(w, j, "{", "}")? + 1;
                        if w.get(j) == Some(&";") { j += 1; }
                        break;
                    }
                    ";" if parens == 0 && brackets == 0 => { j += 1; break; }
                    _ => {}
                }
                j += 1;
            }
            if j == i { return Err("unable to advance statement parser".into()); }
            // A call expression has two possible outcomes: normal completion and
            // exceptional unwinding. This is a conservative effect analysis;
            // it does not assume a call is pure or incapable of throwing.
            let invokes = (i..j.saturating_sub(1)).any(|k| w[k] == "throw") || (i..j.saturating_sub(1)).any(|k| {
                w.get(k + 1) == Some(&"(")
                    && w[k].chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                    && !matches!(w[k], "if" | "while" | "for" | "switch" | "isset" | "empty")
            });
            stmts.push(if is_throw { Stmt::Throw }
                else if invokes { Stmt::Invoke { returns: is_return } }
                else if is_return { Stmt::Return } else { Stmt::Opaque });
            i = j;
        }
        Ok(stmts)
    }
    fn emit(stmts: &[Stmt], next: usize, blocks: &mut Vec<Block>) -> usize {
        let mut successor = next;
        for stmt in stmts.iter().rev() {
            let terminator = match stmt {
                Stmt::Return => Terminator::Return,
                Stmt::Throw => Terminator::Throw,
                Stmt::Opaque => Terminator::Next(successor),
                Stmt::Invoke { returns } => {
                    let normal = if *returns {
                        let index = blocks.len();
                        blocks.push(Block { terminator: Terminator::Return });
                        index
                    } else { successor };
                    let unwind = blocks.len();
                    blocks.push(Block { terminator: Terminator::Throw });
                    Terminator::Invoke { normal, unwind }
                },
                Stmt::Branch(yes, no, condition_invokes) => {
                    let yes = emit(yes, successor, blocks);
                    let no = emit(no, successor, blocks);
                    if *condition_invokes {
                        let normal = blocks.len();
                        blocks.push(Block { terminator: Terminator::Branch { yes, no } });
                        let unwind = blocks.len();
                        blocks.push(Block { terminator: Terminator::Throw });
                        Terminator::Invoke { normal, unwind }
                    } else {
                        Terminator::Branch { yes, no }
                    }
                }
            };
            successor = blocks.len();
            blocks.push(Block { terminator });
        }
        successor
    }
    let end = matching(&words, 0, "{", "}")?;
    if end != words.len() - 1 { return Err("tokens after function body".into()); }
    let stmts = parse(&words, 1, end)?;
    let mut blocks = vec![Block { terminator: Terminator::Exit }];
    let entry = emit(&stmts, 0, &mut blocks);
    let ir = FunctionIr { blocks, entry };
    ir.validate()?;
    Ok(ir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    fn lower(src: &str) -> FunctionIr {
        let tokens = lex(&format!("<?php {src}")).unwrap();
        lower_terminal_body(&tokens[..tokens.len()-1]).unwrap()
    }
    #[test] fn condition_call_has_explicit_exception_edge() {
        let ir = lower("{ if (check()) { return 1; } else { return 2; } }");
        assert!(matches!(ir.blocks[ir.entry].terminator, Terminator::Invoke { .. }));
        assert!(!ir.may_fall_through());
    }
    #[test] fn condition_call_with_missing_else_can_fall_through() {
        let ir = lower("{ if (check()) { return 1; } }");
        assert!(matches!(ir.blocks[ir.entry].terminator, Terminator::Invoke { .. }));
        assert!(ir.may_fall_through());
    }
    #[test] fn nested_conditional_calls_each_have_unwind_edges() {
        let ir = lower("{ if (first()) { return 1; } elseif (second()) { return 2; } else { return 3; } }");
        let invoke_count = ir.blocks.iter().filter(|b| matches!(b.terminator, Terminator::Invoke { .. })).count();
        assert_eq!(invoke_count, 2);
        assert!(!ir.may_fall_through());
    }
    #[test] fn straight_line_return_has_no_fallthrough() {
        assert!(!lower("{ echo 1; return 5; }").may_fall_through());
    }
    #[test] fn conditional_return_does_not_prove_function_return() {
        assert!(lower("{ if (true) { return 5; } }").may_fall_through());
    }
    #[test] fn missing_return_is_reachable() {
        assert!(lower("{ echo 1; }").may_fall_through());
    }
    #[test] fn branch_checks_both_paths() {
        let ir = FunctionIr { entry: 0, blocks: vec![
            Block { terminator: Terminator::Branch { yes: 1, no: 2 } },
            Block { terminator: Terminator::Return },
            Block { terminator: Terminator::Exit },
        ]};
        assert!(ir.may_fall_through());
    }
    #[test] fn branch_with_two_returns_is_terminal() {
        let ir = FunctionIr { entry: 0, blocks: vec![
            Block { terminator: Terminator::Branch { yes: 1, no: 2 } },
            Block { terminator: Terminator::Return },
            Block { terminator: Terminator::Return },
        ]};
        assert!(!ir.may_fall_through());
    }
    #[test] fn both_if_else_branches_return() {
        assert!(!lower("{ if ($x) { return 1; } else { return 2; } }").may_fall_through());
    }
    #[test] fn one_if_else_branch_falls_through() {
        assert!(lower("{ if ($x) { return 1; } else { echo 2; } }").may_fall_through());
    }
    #[test] fn nested_if_else_branches_return() {
        assert!(!lower("{ if ($x) { if ($y) { return 1; } else { return 2; } } else { return 3; } }").may_fall_through());
    }
    #[test] fn else_if_chain_all_terminal() {
        assert!(!lower("{ if ($a) { return 1; } else if ($b) { throw new Exception('x'); } else { return 3; } }").may_fall_through());
    }
    #[test] fn elseif_keyword_chain_all_terminal() {
        assert!(!lower("{ if ($a) { return 1; } elseif ($b) { return 2; } else { return 3; } }").may_fall_through());
    }
    #[test] fn else_if_chain_without_final_else_falls_through() {
        assert!(lower("{ if ($a) { return 1; } elseif ($b) { return 2; } }").may_fall_through());
    }
    #[test] fn else_if_chain_with_nonterminal_branch_falls_through() {
        assert!(lower("{ if ($a) { return 1; } elseif ($b) { echo 2; } else { throw new Exception('x'); } }").may_fall_through());
    }
    #[test] fn conditional_with_trailing_return() {
        assert!(!lower("{ if ($x) { return 1; } return 2; }").may_fall_through());
    }
    #[test] fn loop_return_not_assumed_unconditional() {
        assert!(lower("{ while ($x) { return 1; } }").may_fall_through());
    }
    #[test] fn rejects_empty_ir() {
        assert!(FunctionIr { blocks: vec![], entry: 0 }.validate().is_err());
    }
    #[test] fn rejects_invalid_entry() {
        assert!(FunctionIr { blocks: vec![Block { terminator: Terminator::Exit }], entry: 1 }.validate().is_err());
    }
    #[test] fn rejects_invalid_next_target() {
        assert!(FunctionIr { blocks: vec![Block { terminator: Terminator::Next(3) }], entry: 0 }.validate().is_err());
    }
    #[test] fn rejects_invalid_branch_target() {
        assert!(FunctionIr { blocks: vec![Block { terminator: Terminator::Branch { yes: 0, no: 5 } }], entry: 0 }.validate().is_err());
    }
    #[test] fn accepts_valid_cycle() {
        assert!(FunctionIr { blocks: vec![Block { terminator: Terminator::Next(0) }], entry: 0 }.validate().is_ok());
    }
    #[test] fn loops_do_not_hang_graph_analysis() {
        let ir = FunctionIr { entry: 0, blocks: vec![
            Block { terminator: Terminator::Branch { yes: 1, no: 2 } },
            Block { terminator: Terminator::Next(0) },
            Block { terminator: Terminator::Exit },
        ]};
        assert!(ir.may_fall_through());
    }
    #[test] fn unconditional_throw_is_terminal() {
        assert!(!lower("{ throw new Exception('failed'); }").may_fall_through());
    }
    #[test] fn conditional_throw_with_fallthrough_is_not_terminal() {
        assert!(lower("{ if ($x) { throw new Exception('failed'); } }").may_fall_through());
    }
    #[test] fn both_branches_throw_or_return_are_terminal() {
        assert!(!lower("{ if ($x) { throw new Exception('failed'); } else { return 7; } }").may_fall_through());
    }
    #[test] fn throw_in_loop_is_not_unconditionally_terminal() {
        assert!(lower("{ while ($x) { throw new Exception('failed'); } }").may_fall_through());
    }

}

/// Typed expression IR used by native code generation. These nodes describe
/// operations, not PHP interpreter instructions; operands are evaluated once
/// in PHP's left-to-right order when emitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IntegerOp { Add, Subtract, Multiply, Divide }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ComparisonOp { Equal, NotEqual, Less, Greater, LessEqual, GreaterEqual }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LogicalOp { And, Or }


#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TypedExpr {
    IntegerOperand(String),
    IntegerBinary { op: IntegerOp, left: Box<TypedExpr>, right: Box<TypedExpr> },
    IntegerComparison { op: ComparisonOp, left: Box<TypedExpr>, right: Box<TypedExpr> },
    Logical { op: LogicalOp, left: Box<TypedExpr>, right: Box<TypedExpr> },
    Conditional { condition: Box<TypedExpr>, yes: Box<TypedExpr>, no: Box<TypedExpr> },
    Coalesce { left: Box<TypedExpr>, right: Box<TypedExpr> },
}

impl TypedExpr {
    pub(crate) fn integer_binary(op: &str, left: String, right: String) -> Result<Self, String> {
        let op = match op {
            "+" => IntegerOp::Add,
            "-" => IntegerOp::Subtract,
            "*" => IntegerOp::Multiply,
            "/" => IntegerOp::Divide,
            _ => return Err(format!("unsupported integer IR operation: {op}")),
        };
        Ok(Self::IntegerBinary {
            op,
            left: Box::new(Self::IntegerOperand(left)),
            right: Box::new(Self::IntegerOperand(right)),
        })
    }

    pub(crate) fn logical(op: &str, left: String, right: String) -> Result<Self, String> {
        let op = match op {
            "&&" => LogicalOp::And,
            "||" => LogicalOp::Or,
            _ => return Err(format!("unsupported logical IR operation: {op}")),
        };
        Ok(Self::Logical {
            op,
            left: Box::new(Self::IntegerOperand(left)),
            right: Box::new(Self::IntegerOperand(right)),
        })
    }

    pub(crate) fn integer_comparison(op: &str, left: String, right: String) -> Result<Self, String> {
        let op = match op {
            "==" | "===" => ComparisonOp::Equal,
            "!=" | "!==" => ComparisonOp::NotEqual,
            "<" => ComparisonOp::Less,
            ">" => ComparisonOp::Greater,
            "<=" => ComparisonOp::LessEqual,
            ">=" => ComparisonOp::GreaterEqual,
            _ => return Err(format!("unsupported integer comparison IR operation: {op}")),
        };
        Ok(Self::IntegerComparison {
            op,
            left: Box::new(Self::IntegerOperand(left)),
            right: Box::new(Self::IntegerOperand(right)),
        })
    }

    /// Structured lazy branch: exactly one arm is evaluated after the condition.
    pub(crate) fn conditional(condition: String, yes: String, no: String) -> Self {
        Self::Conditional { condition: Box::new(Self::IntegerOperand(condition)), yes: Box::new(Self::IntegerOperand(yes)), no: Box::new(Self::IntegerOperand(no)) }
    }

    /// Coalescing is lazy and preserves the original left value.
    pub(crate) fn coalesce(left: String, right: String) -> Self {
        Self::Coalesce { left: Box::new(Self::IntegerOperand(left)), right: Box::new(Self::IntegerOperand(right)) }
    }

    /// Emit a compositional Rust Result expression. Leaf operands must already
    /// evaluate to Result<T, E>; errors propagate without pending-error flags
    /// or fabricated fallback values. This is backend-independent in semantics:
    /// the tree owns evaluation order and lazy control flow.
    pub(crate) fn emit_result_rust(&self) -> String {
        match self {
            Self::IntegerOperand(value) => value.clone(),
            Self::IntegerBinary { op, left, right } => {
                let operator = match op {
                    IntegerOp::Add => "+", IntegerOp::Subtract => "-",
                    IntegerOp::Multiply => "*", IntegerOp::Divide => "/",
                };
                format!("({}).and_then(|phix_left| ({}).map(|phix_right| phix_left {operator} phix_right))", left.emit_result_rust(), right.emit_result_rust())
            }
            Self::IntegerComparison { op, left, right } => {
                let operator = match op {
                    ComparisonOp::Equal => "==", ComparisonOp::NotEqual => "!=",
                    ComparisonOp::Less => "<", ComparisonOp::Greater => ">",
                    ComparisonOp::LessEqual => "<=", ComparisonOp::GreaterEqual => ">=",
                };
                format!("({}).and_then(|phix_left| ({}).map(|phix_right| phix_left {operator} phix_right))", left.emit_result_rust(), right.emit_result_rust())
            }
            Self::Logical { op, left, right } => {
                let take_right = match op { LogicalOp::And => "phix_left", LogicalOp::Or => "!phix_left" };
                format!("({}).and_then(|phix_left| if {take_right} {{ {} }} else {{ Ok(phix_left) }})", left.emit_result_rust(), right.emit_result_rust())
            }
            Self::Conditional { condition, yes, no } => format!(
                "({}).and_then(|phix_condition| if phix_condition {{ {} }} else {{ {} }})",
                condition.emit_result_rust(), yes.emit_result_rust(), no.emit_result_rust()),
            Self::Coalesce { left, right } => format!(
                "({}).and_then(|phix_left| if matches!(phix_left, Json::Null) {{ {} }} else {{ Ok(phix_left) }})",
                left.emit_result_rust(), right.emit_result_rust()),
        }
    }

    pub(crate) fn emit_rust(&self) -> String {
        match self {
            Self::IntegerOperand(value) => value.clone(),
            Self::Conditional { condition, yes, no } => format!(
                "{{ let phix_ternary_condition = {}; if phix_has_runtime_error() {{ Default::default() }} else if phix_ternary_condition {{ {} }} else {{ {} }} }}",
                condition.emit_rust(), yes.emit_rust(), no.emit_rust()),
            Self::Coalesce { left, right } => format!(
                "{{ let phix_coalesce_left = {}; if phix_has_runtime_error() {{ Json::Null }} else if !matches!(phix_coalesce_left, Json::Null) {{ phix_coalesce_left }} else {{ {} }} }}",
                left.emit_rust(), right.emit_rust()),

            Self::Logical { op, left, right } => {
                // Rust's conditional operators preserve PHP's lazy RHS evaluation
                // for already-typed boolean operands, without eager temporaries.
                let operator = match op { LogicalOp::And => "&&", LogicalOp::Or => "||" };
                format!("{{ let phix_ir_left = {}; if phix_has_runtime_error() {{ false }} else {{ phix_ir_left {operator} ({}) }} }}", left.emit_rust(), right.emit_rust())
            }
            Self::IntegerComparison { op, left, right } => {
                let operator = match op {
                    ComparisonOp::Equal => "==",
                    ComparisonOp::NotEqual => "!=",
                    ComparisonOp::Less => "<",
                    ComparisonOp::Greater => ">",
                    ComparisonOp::LessEqual => "<=",
                    ComparisonOp::GreaterEqual => ">=",
                };
                format!("{{ let phix_ir_left = {}; if phix_has_runtime_error() {{ Default::default() }} else {{ let phix_ir_right = {}; if phix_has_runtime_error() {{ Default::default() }} else {{ phix_ir_left {operator} phix_ir_right }} }} }}", left.emit_rust(), right.emit_rust())
            }
            Self::IntegerBinary { op, left, right } => {
                let operator = match op {
                    IntegerOp::Add => "+",
                    IntegerOp::Subtract => "-",
                    IntegerOp::Multiply => "*",
                    IntegerOp::Divide => "/",
                };
                // Explicit bindings ensure each operand is evaluated once,
                // in source order, including when it contains a function call.
                format!("{{ let phix_ir_left = {}; if phix_has_runtime_error() {{ Default::default() }} else {{ let phix_ir_right = {}; if phix_has_runtime_error() {{ Default::default() }} else {{ phix_ir_left {operator} phix_ir_right }} }} }}", left.emit_rust(), right.emit_rust())
            }
        }
    }
}

#[cfg(test)]
mod expression_ir_tests {
    use super::*;
    #[test] fn logical_ir_preserves_lazy_rhs_and_order() {
        for op in ["&&", "||"] {
            let code = TypedExpr::logical(op, "left_effect()".into(), "right_effect()".into()).unwrap().emit_rust();
            assert!(code.contains(&format!("phix_ir_left {op} (right_effect())")));
            assert!(code.contains("if phix_has_runtime_error() { false } else"));
            assert!(code.find("left_effect()").unwrap() < code.find("right_effect()").unwrap());
            assert_eq!(code.matches("left_effect()").count(), 1);
            assert_eq!(code.matches("right_effect()").count(), 1);
        }
    }
    #[test] fn logical_ir_rejects_invalid_operators() {
        assert!(TypedExpr::logical("&", "a".into(), "b".into()).is_err());
    }
    #[test] fn comparison_operands_evaluate_once_and_in_order() {
        let code = TypedExpr::integer_comparison("<", "left_effect()".into(), "right_effect()".into()).unwrap().emit_rust();
        assert!(code.find("left_effect()").unwrap() < code.find("right_effect()").unwrap());
        assert_eq!(code.matches("left_effect()").count(), 1);
        assert_eq!(code.matches("right_effect()").count(), 1);
    }
    #[test] fn comparison_operator_mapping() {
        for (source, rust) in [("==", "=="), ("===", "=="), ("!=", "!="), ("!==", "!="), ("<", "<"), (">", ">"), ("<=", "<="), (">=", ">=")] {
            let code = TypedExpr::integer_comparison(source, "a".into(), "b".into()).unwrap().emit_rust();
            assert!(code.contains(&format!("phix_ir_left {rust} phix_ir_right")));
        }
    }
    #[test] fn comparison_rejects_unknown_operator() {
        assert!(TypedExpr::integer_comparison("<=>", "a".into(), "b".into()).is_err());
    }
    #[test] fn emits_ordered_integer_operands() {
        let code = TypedExpr::integer_binary("+", "fn_left()".into(), "fn_right()".into()).unwrap().emit_rust();
        assert!(code.find("fn_left()").unwrap() < code.find("fn_right()").unwrap());
        assert_eq!(code.matches("fn_left()").count(), 1);
        assert_eq!(code.matches("fn_right()").count(), 1);
    }
    #[test] fn rejects_unknown_operator() {
        assert!(TypedExpr::integer_binary(".", "a".into(), "b".into()).is_err());
    }
    #[test] fn emits_all_integer_operations() {
        for op in ["+", "-", "*", "/"] {
            let code = TypedExpr::integer_binary(op, "a".into(), "b".into()).unwrap().emit_rust();
            assert!(code.contains(&format!("phix_ir_left {op} phix_ir_right")));
        }
    }
}

#[cfg(test)]
mod exception_edges_tests {
    use super::*;
    #[test]
    fn invoke_checks_both_successors() {
        let ir = FunctionIr { entry: 0, blocks: vec![
            Block { terminator: Terminator::Invoke { normal: 1, unwind: 2 } },
            Block { terminator: Terminator::Return },
            Block { terminator: Terminator::Throw },
        ]};
        assert!(ir.validate().is_ok());
        assert!(!ir.may_fall_through());
    }
    #[test]
    fn invoke_rejects_invalid_unwind_target() {
        let ir = FunctionIr { entry: 0, blocks: vec![
            Block { terminator: Terminator::Invoke { normal: 1, unwind: 7 } },
            Block { terminator: Terminator::Return },
        ]};
        assert!(ir.validate().unwrap_err().contains("invalid successor 7"));
    }
    #[test]
    fn invoke_detects_reachable_fallthrough() {
        let ir = FunctionIr { entry: 0, blocks: vec![
            Block { terminator: Terminator::Invoke { normal: 1, unwind: 2 } },
            Block { terminator: Terminator::Return },
            Block { terminator: Terminator::Exit },
        ]};
        assert!(ir.may_fall_through());
    }
    #[test]
    fn invoke_handles_cyclic_exceptional_paths() {
        let ir = FunctionIr { entry: 0, blocks: vec![
            Block { terminator: Terminator::Invoke { normal: 1, unwind: 0 } },
            Block { terminator: Terminator::Return },
        ]};
        assert!(ir.validate().is_ok());
        assert!(!ir.may_fall_through());
    }
}

#[cfg(test)]
mod throw_expression_cfg_tests {
    use super::*;
    use crate::lexer::lex;
    #[test]
    fn return_throw_is_exception_exit_not_value_return() {
        let tokens = lex("<?php { return throw new Exception('failed'); }").unwrap();
        let ir = lower_terminal_body(&tokens[..tokens.len()-1]).unwrap();
        assert_eq!(ir.blocks[ir.entry].terminator, Terminator::Throw);
        assert!(!ir.may_fall_through());
    }
    #[test]
    fn ordinary_return_remains_value_exit() {
        let tokens = lex("<?php { return 12; }").unwrap();
        let ir = lower_terminal_body(&tokens[..tokens.len()-1]).unwrap();
        assert_eq!(ir.blocks[ir.entry].terminator, Terminator::Return);
    }
}

#[cfg(test)]
mod call_effect_tests {
    use super::*;
    use crate::lexer::lex;
    fn ir(body: &str) -> FunctionIr {
        let source = format!("<?php {body}");
        let tokens = lex(&source).unwrap();
        lower_terminal_body(&tokens[..tokens.len()-1]).unwrap()
    }
    #[test]
    fn call_statement_has_normal_and_unwind_edges() {
        let graph = ir("{ work(); return 7; }");
        assert!(graph.blocks.iter().any(|b| matches!(b.terminator, Terminator::Invoke { .. })));
        assert!(!graph.may_fall_through());
        graph.validate().unwrap();
    }
    #[test]
    fn returned_call_unwinds_or_returns_but_never_falls_through() {
        let graph = ir("{ return work(); }");
        assert!(graph.blocks.iter().any(|b| matches!(b.terminator, Terminator::Invoke { .. })));
        assert!(!graph.may_fall_through());
    }
    #[test]
    fn call_without_return_still_has_fallthrough_path() {
        let graph = ir("{ work(); }");
        assert!(graph.may_fall_through());
    }
}

#[cfg(test)]
mod throw_expression_effect_tests {
    use super::*;
    use crate::lexer::lex;
    fn lowered(src: &str) -> FunctionIr {
        let tokens = lex(&format!("<?php {src}")).unwrap();
        lower_terminal_body(&tokens[..tokens.len()-1]).unwrap()
    }
    #[test]
    fn conditional_throw_expression_has_unwind_edge() {
        let ir = lowered("{ if ($ok || throw new Exception('no')) { return 1; } else { return 2; } }");
        ir.validate().unwrap();
        assert!(ir.blocks.iter().any(|b| matches!(b.terminator, Terminator::Invoke { .. })));
        assert!(!ir.may_fall_through());
    }
    #[test]
    fn return_with_nested_throw_expression_has_both_exits() {
        let ir = lowered("{ return $value || throw new Exception('missing'); }");
        ir.validate().unwrap();
        assert!(ir.blocks.iter().any(|b| matches!(b.terminator, Terminator::Invoke { .. })));
        assert!(!ir.may_fall_through());
    }
}

#[cfg(test)]
mod integer_exception_order_tests {
    use super::TypedExpr;
    #[test]
    fn integer_arithmetic_skips_rhs_after_throw() {
        for op in ["+", "-", "*", "/"] {
            let code = TypedExpr::integer_binary(op, "left_effect()".into(), "right_effect()".into()).unwrap().emit_rust();
            assert!(code.find("left_effect()").unwrap() < code.find("phix_has_runtime_error()").unwrap());
            assert!(code.find("phix_has_runtime_error()").unwrap() < code.find("right_effect()").unwrap());
            assert_eq!(code.matches("right_effect()").count(), 1);
        }
    }
    #[test]
    fn integer_comparison_skips_rhs_after_throw() {
        for op in ["==", "===", "!=", "!==", "<", ">", "<=", ">="] {
            let code = TypedExpr::integer_comparison(op, "left_effect()".into(), "right_effect()".into()).unwrap().emit_rust();
            assert!(code.find("phix_has_runtime_error()").unwrap() < code.find("right_effect()").unwrap());
            assert_eq!(code.matches("right_effect()").count(), 1);
        }
    }
}

#[cfg(test)]
mod structured_branch_ir_tests {
    use super::*;
    #[test]
    fn conditional_ir_evaluates_one_branch_after_condition() {
        let generated = TypedExpr::conditional("test()".into(), "yes()".into(), "no()".into()).emit_rust();
        assert!(generated.find("test()").unwrap() < generated.find("yes()").unwrap());
        assert!(generated.contains("else if phix_ternary_condition"));
        assert!(generated.contains("else { no() }"));
    }
    #[test]
    fn coalescing_ir_keeps_lazy_fallback() {
        let generated = TypedExpr::coalesce("left()".into(), "fallback()".into()).emit_rust();
        assert!(generated.find("left()").unwrap() < generated.find("fallback()").unwrap());
        assert!(generated.contains("!matches!(phix_coalesce_left, Json::Null)"));
    }
}

#[cfg(test)]
mod result_emission_tests {
    use super::*;

    #[test]
    fn nested_result_operations_preserve_structure_and_order() {
        let nested = TypedExpr::IntegerBinary {
            op: IntegerOp::Add,
            left: Box::new(TypedExpr::IntegerOperand("first()".into())),
            right: Box::new(TypedExpr::IntegerComparison {
                op: ComparisonOp::Less,
                left: Box::new(TypedExpr::IntegerOperand("second()".into())),
                right: Box::new(TypedExpr::IntegerOperand("third()".into())),
            }),
        };
        let code = nested.emit_result_rust();
        assert!(code.find("first()").unwrap() < code.find("second()").unwrap());
        assert!(code.find("second()").unwrap() < code.find("third()").unwrap());
        for call in ["first()", "second()", "third()"] { assert_eq!(code.matches(call).count(), 1); }
        assert!(!code.contains("Default::default"));
        assert!(!code.contains("phix_has_runtime_error"));
    }

    #[test]
    fn lazy_result_branches_do_not_eagerly_evaluate_alternatives() {
        let ternary = TypedExpr::conditional("condition()".into(), "yes()".into(), "no()".into()).emit_result_rust();
        assert!(ternary.contains("if phix_condition { yes() } else { no() }"));
        let coalesce = TypedExpr::coalesce("lookup()".into(), "fallback()".into()).emit_result_rust();
        assert!(coalesce.contains("if matches!(phix_left, Json::Null) { fallback() }"));
        for op in ["&&", "||"] {
            let logical = TypedExpr::logical(op, "left()".into(), "right()".into()).unwrap().emit_result_rust();
            assert!(logical.contains(".and_then("));
            assert!(logical.contains("{ right() } else { Ok(phix_left) }"));
        }
    }
}
