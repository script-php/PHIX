//! Structured PHP control flow. This module deliberately does not use a default
//! return value to encode a thrown exception or a nonlocal control transfer.
//! The code generator will migrate to this representation in stages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PhpFlow<T, E> {
    Normal(T),
    Return(T),
    Break(u32),
    Continue(u32),
    Throw(E),
    /// Terminates the current PHP script, not necessarily the native server.
    Exit(PhpExit),
}

/// Script termination requested by PHP exit/die. The argument has already
/// been evaluated using the target PHP version's conversion rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PhpExit {
    Status(i32),
    Message(String),
}

impl<T, E> PhpFlow<T, E> {
    /// PHP exit bypasses finally blocks. The caller must not execute the
    /// finally body when this returns false.
    pub(crate) fn runs_finally(&self) -> bool {
        !matches!(self, PhpFlow::Exit(_))
    }

    /// PHP finally semantics: a normal finally leaves the pending transfer
    /// intact; any transfer from finally replaces it, including a new throw.
    pub(crate) fn after_finally(self, finally: PhpFlow<T, E>) -> Self {
        if !self.runs_finally() { return self; }
        match finally {
            PhpFlow::Normal(_) => self,
            transfer => transfer,
        }
    }

    /// Execute a PHP finally body only for transfers that enter it.  In
    /// particular, PHP exit/die bypasses finally, so the closure must not
    /// be evaluated in that case (it may contain observable side effects).
    /// A transfer from finally replaces the pending transfer.
    pub(crate) fn with_finally(self, body: impl FnOnce() -> Self) -> Self {
        if !self.runs_finally() { return self; }
        self.after_finally(body())
    }

    /// Sequence a normal step with its successor without executing the
    /// successor after a nonlocal transfer. No placeholder value is needed.
    pub(crate) fn and_then(self, next: impl FnOnce(T) -> PhpFlow<T, E>) -> Self {
        match self {
            PhpFlow::Normal(value) => next(value),
            PhpFlow::Return(value) => PhpFlow::Return(value),
            PhpFlow::Break(level) => PhpFlow::Break(level),
            PhpFlow::Continue(level) => PhpFlow::Continue(level),
            PhpFlow::Throw(error) => PhpFlow::Throw(error),
            PhpFlow::Exit(exit) => PhpFlow::Exit(exit),
        }
    }

    /// A matching catch handles only a thrown value. Return and loop
    /// transfers bypass catch handlers, exactly as PHP requires.
    pub(crate) fn catch_throw(self, handler: impl FnOnce(E) -> Self) -> Self {
        match self {
            PhpFlow::Throw(error) => handler(error),
            other => other,
        }
    }

    /// Compose PHP catch and finally in execution order, without evaluating
    /// skipped branches or fabricating return values.
    pub(crate) fn try_catch_finally(
        self,
        catch: impl FnOnce(E) -> Self,
        finally: impl FnOnce() -> Self,
    ) -> Self {
        self.catch_throw(catch).with_finally(finally)
    }

    /// Convert a completed PHP function transfer into an explicit native
    /// success/exception result. Loop transfers must have been consumed by
    /// their enclosing loop before reaching this boundary.
    pub(crate) fn into_function_result(self) -> Result<Result<T, E>, &'static str> {
        match self {
            PhpFlow::Return(value) => Ok(Ok(value)),
            PhpFlow::Throw(error) => Ok(Err(error)),
            PhpFlow::Normal(_) => Err("function reached boundary without a return"),
            PhpFlow::Break(_) | PhpFlow::Continue(_) => Err("unresolved loop transfer at function boundary"),
            PhpFlow::Exit(_) => Err("script exit requires request-level handling"),
        }
    }

    /// One loop boundary consumes level 1, and decrements deeper break/continue.
    pub(crate) fn across_loop(self) -> Self {
        match self {
            PhpFlow::Break(n) if n > 1 => PhpFlow::Break(n - 1),
            PhpFlow::Continue(n) if n > 1 => PhpFlow::Continue(n - 1),
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PhpFlow::*;
    #[test]
    fn normal_finally_preserves_return() {
        assert_eq!(Return::<i32, &str>(7).after_finally(Normal(9)), Return(7));
    }
    #[test]
    fn normal_finally_preserves_throw() {
        assert_eq!(Throw::<i32, &str>("original").after_finally(Normal(9)), Throw("original"));
    }
    #[test]
    fn finally_throw_replaces_pending_return() {
        assert_eq!(Return::<i32, &str>(7).after_finally(Throw("replacement")), Throw("replacement"));
    }
    #[test]
    fn finally_return_replaces_pending_throw() {
        assert_eq!(Throw::<i32, &str>("original").after_finally(Return(42)), Return(42));
    }
    #[test]
    fn nested_finally_applies_in_execution_order() {
        let flow = Throw::<i32, &str>("inner")
            .after_finally(Return(10))
            .after_finally(Throw("outer"));
        assert_eq!(flow, Throw("outer"));
    }
    #[test]
    fn sequencing_does_not_run_after_return_or_throw() {
        let mut ran = false;
        let returned = Return::<i32, &str>(4).and_then(|_| { ran = true; Normal(9) });
        assert_eq!(returned, Return(4));
        let thrown = Throw::<i32, &str>("failure").and_then(|_| { ran = true; Normal(9) });
        assert_eq!(thrown, Throw("failure"));
        assert!(!ran);
    }
    #[test]
    fn sequencing_executes_only_normal_steps() {
        assert_eq!(Normal::<i32, &str>(3).and_then(|n| Normal(n + 2)), Normal(5));
        assert_eq!(Normal::<i32, &str>(3).and_then(|_| Throw("error")), Throw("error"));
    }
    #[test]
    fn finally_preserves_loop_transfer_unless_overridden() {
        assert_eq!(Break::<(), &str>(2).after_finally(Normal(())), Break(2));
        assert_eq!(Continue::<(), &str>(1).after_finally(Throw("new")), Throw("new"));
    }
    #[test]
    fn catch_only_handles_thrown_control_flow() {
        assert_eq!(Throw::<i32, &str>("old").catch_throw(|_| Normal(7)), Normal(7));
        assert_eq!(Return::<i32, &str>(9).catch_throw(|_| Normal(7)), Return(9));
        assert_eq!(Break::<i32, &str>(2).catch_throw(|_| Normal(7)), Break(2));
    }
    #[test]
    fn caught_exception_can_be_rethrown_and_replaced_in_finally() {
        let result = Throw::<i32, &str>("original")
            .catch_throw(|_| Throw("from catch"))
            .after_finally(Throw("from finally"));
        assert_eq!(result, Throw("from finally"));
    }
    #[test]
    fn loop_levels_decrement_without_sentinel_values() {
        assert_eq!(Break::<i32, &str>(3).across_loop(), Break(2));
        assert_eq!(Continue::<i32, &str>(2).across_loop(), Continue(1));
        assert_eq!(Break::<i32, &str>(1).across_loop(), Break(1));
    }
}

/// Analyze the top-level statements of a function body without treating a
/// return in a nested loop/conditional as an unconditional function return.
/// This is deliberately a *must-return* analysis, not a search for a `return`
/// token. Unsupported control flow is never accepted as a proof of return.
pub(crate) fn has_terminal_transfer(tokens: &[crate::lexer::Token]) -> bool {
    if tokens.first().map(|t| t.text.as_str()) != Some("{") { return false; }
    let mut depth = 0usize;
    let mut statement_start = None;
    let mut last_statement = None;
    let mut parens = 0usize;
    let mut brackets = 0usize;
    for (i, token) in tokens.iter().enumerate() {
        match token.text.as_str() {
            "{" => { depth += 1; if depth == 1 { continue; } },
            "}" => {
                if depth == 0 { return false; }
                depth -= 1;
                if depth == 0 {
                    // A final `if`, `while`, or `try` cannot prove an
                    // unconditional return without branch analysis.
                    return last_statement == Some("return");
                }
            },
            "(" if depth == 1 => parens += 1,
            ")" if depth == 1 => { if parens == 0 { return false; } parens -= 1; },
            "[" if depth == 1 => brackets += 1,
            "]" if depth == 1 => { if brackets == 0 { return false; } brackets -= 1; },
            _ => {}
        }
        if depth == 1 {
            if statement_start.is_none() && token.text != ";" && token.text != "}" {
                statement_start = Some(i);
            }
            if token.text == ";" && parens == 0 && brackets == 0 {
                last_statement = statement_start.map(|start| tokens[start].text.as_str());
                statement_start = None;
            }
            if token.text == "}" {
                // Nested block ended: it does not prove a return.
                last_statement = None;
                statement_start = None;
            }
        }
    }
    false
}

#[cfg(test)]
mod terminal_tests {
    use super::has_terminal_transfer;
    use crate::lexer::lex;
    fn check(body: &str) -> bool {
        let tokens = lex(&format!("<?php {body}")).unwrap();
        has_terminal_transfer(&tokens[..tokens.len()-1])
    }
    #[test] fn explicit_return_is_terminal() { assert!(check("{ echo 1; return 2; }")); }
    #[test] fn throw_only_requires_structured_unwinding() { assert!(!check("{ throw new Exception('x'); }")); }
    #[test] fn missing_return_is_not_synthesized() { assert!(!check("{ echo 1; }")); }
    #[test] fn nested_return_does_not_prove_terminal_return() { assert!(!check("{ if (true) { return 2; } }")); }
    #[test] fn return_inside_loop_does_not_prove_terminal_return() { assert!(!check("{ while (false) { return 2; } }")); }
    #[test] fn return_after_conditional_is_terminal() { assert!(check("{ if (true) { echo 1; } return 2; }")); }
    #[test] fn nested_parentheses_do_not_split_statements() { assert!(check("{ echo (1 + 2); return 3; }")); }
}

#[cfg(test)]
mod exit_tests {
    use super::{PhpExit, PhpFlow};
    #[test]
    fn exit_stops_sequence_without_running_next_operation() {
        let mut ran = false;
        let result = PhpFlow::<i32, &str>::Exit(PhpExit::Status(3))
            .and_then(|_| { ran = true; PhpFlow::Normal(9) });
        assert_eq!(result, PhpFlow::Exit(PhpExit::Status(3)));
        assert!(!ran);
    }
    #[test]
    fn exit_is_not_caught_by_exception_handlers() {
        let result = PhpFlow::<(), &str>::Exit(PhpExit::Message("done".into()))
            .catch_throw(|_| PhpFlow::Normal(()));
        assert_eq!(result, PhpFlow::Exit(PhpExit::Message("done".into())));
    }
    #[test]
    fn exit_bypasses_finally_and_cannot_be_replaced_by_return() {
        let exit = PhpFlow::<i32, &str>::Exit(PhpExit::Status(0));
        assert!(!exit.runs_finally());
        assert_eq!(exit.after_finally(PhpFlow::Return(9)), PhpFlow::Exit(PhpExit::Status(0)));
        assert!(PhpFlow::<i32, &str>::Throw("x").runs_finally());
    }
    #[test]
    fn exit_survives_loop_boundary() {
        let exit = PhpFlow::<(), &str>::Exit(PhpExit::Status(1));
        assert_eq!(exit.across_loop(), PhpFlow::Exit(PhpExit::Status(1)));
    }
}

#[cfg(test)]
mod lazy_finally_tests {
    use super::{PhpExit, PhpFlow};
    #[test]
    fn exit_does_not_evaluate_finally_body() {
        let mut invoked = false;
        let outcome = PhpFlow::<i64, String>::Exit(PhpExit::Status(3))
            .with_finally(|| { invoked = true; PhpFlow::Throw("unexpected".into()) });
        assert!(!invoked);
        assert_eq!(outcome, PhpFlow::Exit(PhpExit::Status(3)));
    }
    #[test]
    fn finally_return_replaces_pending_throw() {
        let mut invoked = 0;
        let outcome = PhpFlow::<i64, &str>::Throw("original")
            .with_finally(|| { invoked += 1; PhpFlow::Return(42) });
        assert_eq!(invoked, 1);
        assert_eq!(outcome, PhpFlow::Return(42));
    }
    #[test]
    fn finally_normal_preserves_pending_return() {
        let outcome = PhpFlow::<i64, &str>::Return(12)
            .with_finally(|| PhpFlow::Normal(99));
        assert_eq!(outcome, PhpFlow::Return(12));
    }
}

#[cfg(test)]
mod function_outcome_tests {
    use super::{PhpExit, PhpFlow};

    #[test]
    fn return_and_throw_are_distinct_results_without_default_values() {
        assert_eq!(PhpFlow::<i64, String>::Return(17).into_function_result(), Ok(Ok(17)));
        assert_eq!(PhpFlow::<i64, String>::Throw("failure".into()).into_function_result(), Ok(Err("failure".into())));
    }

    #[test]
    fn invalid_function_transfers_are_not_silently_converted() {
        assert!(PhpFlow::<i64, String>::Normal(0).into_function_result().is_err());
        assert!(PhpFlow::<i64, String>::Break(1).into_function_result().is_err());
        assert!(PhpFlow::<i64, String>::Continue(2).into_function_result().is_err());
        assert!(PhpFlow::<i64, String>::Exit(PhpExit::Status(0)).into_function_result().is_err());
    }
}

#[cfg(test)]
mod composed_unwinding_tests {
    use super::{PhpExit, PhpFlow};

    #[test]
    fn caught_throw_runs_finally_and_keeps_handler_return() {
        let mut order = Vec::new();
        let outcome = PhpFlow::<i32, &str>::Throw("original")
            .try_catch_finally(
                |error| { order.push(error); PhpFlow::Return(7) },
                || { PhpFlow::Normal(0) },
            );
        assert_eq!(order, ["original"]);
        assert_eq!(outcome.into_function_result(), Ok(Ok(7)));
    }

    #[test]
    fn uncaught_transfer_skips_catch_and_finally_overrides_return() {
        let mut caught = false;
        let outcome = PhpFlow::<i32, &str>::Return(4)
            .try_catch_finally(
                |_| { caught = true; PhpFlow::Return(5) },
                || PhpFlow::Throw("finally"),
            );
        assert!(!caught);
        assert_eq!(outcome.into_function_result(), Ok(Err("finally")));
    }

    #[test]
    fn exit_skips_both_catch_and_finally_callbacks() {
        let mut caught = false;
        let mut finalized = false;
        let outcome = PhpFlow::<i32, &str>::Exit(PhpExit::Status(3))
            .try_catch_finally(
                |_| { caught = true; PhpFlow::Return(5) },
                || { finalized = true; PhpFlow::Return(6) },
            );
        assert!(!caught);
        assert!(!finalized);
        assert_eq!(outcome, PhpFlow::Exit(PhpExit::Status(3)));
    }
}

/// Explicit expression evaluation. An error never manufactures a value of T.
/// These combinators are the lowering contract for nested PHP expressions;
/// generated code must evaluate each closure only when its predecessors succeed.
pub mod expression_outcome {
    pub type Outcome<T, E> = Result<T, E>;

    pub fn sequence<A, B, E>(
        left: impl FnOnce() -> Outcome<A, E>,
        right: impl FnOnce() -> Outcome<B, E>,
    ) -> Outcome<(A, B), E> {
        let a = left()?;
        let b = right()?;
        Ok((a, b))
    }

    pub fn binary<A, B, T, E>(
        left: impl FnOnce() -> Outcome<A, E>,
        right: impl FnOnce() -> Outcome<B, E>,
        combine: impl FnOnce(A, B) -> Outcome<T, E>,
    ) -> Outcome<T, E> {
        let (a, b) = sequence(left, right)?;
        combine(a, b)
    }

    pub fn conditional<T, E>(
        condition: impl FnOnce() -> Outcome<bool, E>,
        yes: impl FnOnce() -> Outcome<T, E>,
        no: impl FnOnce() -> Outcome<T, E>,
    ) -> Outcome<T, E> {
        if condition()? { yes() } else { no() }
    }

    pub fn coalesce<T, E>(
        left: impl FnOnce() -> Outcome<Option<T>, E>,
        fallback: impl FnOnce() -> Outcome<T, E>,
    ) -> Outcome<T, E> {
        match left()? { Some(value) => Ok(value), None => fallback() }
    }
}

#[cfg(test)]
mod expression_outcome_tests {
    use super::expression_outcome as expr;
    use std::cell::Cell;

    #[test]
    fn binary_does_not_run_right_or_combine_after_left_error() {
        let right = Cell::new(false);
        let combined = Cell::new(false);
        let result: Result<i64, &str> = expr::binary(
            || Err("left"),
            || { right.set(true); Ok(2) },
            |a: i64, b: i64| { combined.set(true); Ok(a + b) },
        );
        assert_eq!(result, Err("left"));
        assert!(!right.get());
        assert!(!combined.get());
    }

    #[test]
    fn nested_errors_propagate_without_default_value() {
        let result: Result<i64, &str> = expr::binary(
            || Ok(2),
            || expr::binary(|| Ok(3), || Err("nested"), |a: i64, b: i64| Ok(a + b)),
            |a, b| Ok(a + b),
        );
        assert_eq!(result, Err("nested"));
    }

    #[test]
    fn conditional_and_coalesce_only_evaluate_selected_branch() {
        let skipped = Cell::new(false);
        let result: Result<i32, &str> = expr::conditional(
            || Ok(true), || Ok(7), || { skipped.set(true); Err("unused") },
        );
        assert_eq!(result, Ok(7));
        let result = expr::coalesce(|| Ok(Some(9)), || { skipped.set(true); Err("unused") });
        assert_eq!(result, Ok(9));
        assert!(!skipped.get());
    }
}
