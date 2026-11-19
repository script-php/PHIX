use super::{Parser,Ty};
use crate::lexer::{Token,Kind};
use std::collections::HashMap;
pub fn compile(tokens:&[Token])->Result<String,String>{let mut p=Parser{ts:tokens,at:0,vars:HashMap::new(),fns:Vec::new()};p.signatures()?;let fns=p.fns.clone();let mut out=String::new();for f in &fns{p.at=f.start;p.vars=f.args.iter().cloned().collect();let body=p.block(f.ret)?;if f.ret!=Ty::Void && { let ir = super::ir::lower_terminal_body(&p.ts[f.start..f.end])?; ir.validate()?; ir.may_fall_through() } { return Err(format!("function {} may fall through without returning a value",f.name)); }let params=f.args.iter().map(|(n,t)|format!(", mut v_{n}: {}",t.rust())).collect::<String>();let terminal = if f.ret == Ty::Void { String::new() } else { format!("unreachable!(\"Phix verified terminal function {} fell through\");", f.name) };if f.ret == Ty::Void {
 out.push_str(&format!("fn fn_{}(ctx: &mut RequestContext{params}) -> () {{ {body}; }}\n", f.name));
} else {
 // The compiled function boundary returns an explicit exception outcome.
 // Legacy expression/statement guards still operate inside the closure;
 // those will be migrated separately to direct Result propagation.
 out.push_str(&format!("fn fn_{}(ctx: &mut RequestContext{params}) -> Result<{}, std::sync::Arc<PhixRuntimeError>> {{ {body}; {terminal} }}\n", f.name, f.ret.rust()));
}if p.at!=f.end{return Err("function parsing mismatch".into());}}
 p.at=0;p.vars.clear();out.push_str("fn phix_app_request(ctx: &mut RequestContext) {\n");while p.cur().kind!=Kind::End{if p.take("function"){p.at+=1;p.need("(")?;let mut d=1;while d>0{if p.cur().kind==Kind::End{return Err("unterminated signature".into());}if p.take("("){d+=1;}else if p.take(")"){d-=1;}else{p.at+=1;}}p.need(":")?;p.at+=1;p.need("{")?;d=1;while d>0{if p.cur().kind==Kind::End{return Err("unterminated body".into());}if p.take("{"){d+=1;}else if p.take("}"){d-=1;}else{p.at+=1;}}continue;}out.push_str(&p.statement(Ty::Void)?);out.push_str("if phix_has_runtime_error() { return; }\n");}out.push_str("}\n");Ok(out)}

#[cfg(test)]
mod array_syntax_tests {
    use super::*;
    use crate::lexer::lex;
    fn build(source: &str) -> Result<String, String> { compile(&lex(source)?) }
    #[test]
    fn nested_else_if_is_accepted_with_complete_returns() {
        let src = "<?php function choice(): int { if (false) { return 1; } else if (true) { return 2; } else { return 3; } } echo choice();";
        assert!(build(src).is_ok());
    }
    #[test]
    fn nested_else_if_without_final_else_does_not_prove_return() {
        let src = "<?php function choice(): int { if (false) { return 1; } else if (true) { return 2; } } echo choice();";
        assert!(build(src).is_err());
    }
    #[test]
    fn elseif_keyword_compiles_with_complete_returns() {
        let src = "<?php function choice(): int { if (false) { return 1; } elseif (false) { return 2; } elseif (true) { return 3; } else { return 4; } } echo choice();";
        let generated = build(src).unwrap();
        assert!(generated.matches("phix_condition").count() >= 3);
    }
    #[test]
    fn elseif_keyword_without_else_does_not_prove_return() {
        let src = "<?php function choice(): int { if (false) { return 1; } elseif (true) { return 2; } } echo choice();";
        assert!(build(src).is_err());
    }
    #[test]
    fn mixed_else_if_and_elseif_chains_compile() {
        let src = "<?php function choice(): int { if (false) { return 1; } else if (false) { return 2; } elseif (true) { return 3; } else { return 4; } } echo choice();";
        assert!(build(src).is_ok());
    }
    #[test]
    fn indexed_array_literal_compiles() {
        let generated = build("<?php $a = [10, 20, true, 'ok', null]; echo $a[0]; echo $a[3];").unwrap();
        assert!(generated.contains("Json::PhpArray(PhpArray::from_cell_parts(vec!"));
        assert!(generated.contains("Json::Int(10i64)"));
        assert!(generated.contains("Json::Null"));
        assert!(generated.contains("phix_index("));
    }
    #[test]
    fn nested_arrays_and_empty_arrays_compile() {
        build("<?php $a = [[1, 2], []]; echo $a[0];").unwrap();
    }
    #[test]
    fn boolean_and_null_array_keys_compile() {
        assert!(build("<?php $a = [1]; echo $a[true]; echo $a[null];").is_ok());
    }
    #[test]
    fn array_assignment_compiles() {
        assert!(build("<?php $a = [1]; $a[0] = 2;").unwrap().contains("set_path"));
    }
}

#[cfg(test)]
mod associative_tests {
 use super::*;
 use crate::lexer::lex;
 fn generated(src:&str)->Result<String,String>{compile(&lex(src)?)}
 #[test] fn associative_literals_and_updates(){let s=generated("<?php $user = ['name' => 'Alex', 'age' => 25]; $user['age'] = 26; echo $user['name'];").unwrap();assert!(s.contains("Json::PhpArray"));assert!(s.contains("set_path"));assert!(s.contains("phix_index"));}
 #[test] fn append_and_nested_reads(){let s=generated("<?php $a = [[1, 2]]; $a[] = [3, 4]; echo $a[1][0];").unwrap();assert!(s.contains("append_path"));assert_eq!(s.matches(".phix_index(&").count(),2);}
 #[test] fn mixed_keys_supported(){assert!(generated("<?php $a = ['x' => 1, 2];").unwrap().contains("PhpArray::from_cell_parts"));}
}

#[cfg(test)]
mod literal_reference_tests {
 use super::*;
 use crate::lexer::lex;
 #[test]
 fn array_literal_element_reference_codegen() {
  let tokens=lex("<?php $a = ['x' => 10]; $b = ['ref' => &$a['x']]; $b['ref'] = 20;").unwrap();
  let generated=compile(&tokens).unwrap();
  assert!(generated.contains("reference_cell()"));
  assert!(generated.contains("from_cell_parts"));
 }
 #[test]
 fn whole_variable_reference_in_literal_is_compiled() {
  let tokens=lex("<?php $a = ['x' => 10]; $b = [&$a];").unwrap();
  let generated=compile(&tokens).unwrap();
  assert!(generated.contains("bind_element(&[]).reference_cell()"));
 }
}

#[cfg(test)]
mod scalar_reference_literal_tests {
 use super::*;
 use crate::lexer::lex;
 #[test]
 fn null_initialized_variable_can_be_referenced_in_array_literal() {
  let generated=compile(&lex("<?php $value = null; $a = ['ref' => &$value]; $a['ref'] = 25; echo $value;").unwrap()).unwrap();
  assert!(generated.contains("v_value.bind_element(&[]).reference_cell()"));
  assert!(generated.contains("PhpArray::from_cell_parts"));
 }
 #[test]
 fn native_integer_variable_reference_is_promoted() {
  let generated=compile(&lex("<?php $value = 10; $a = [&$value];").unwrap()).unwrap();
  assert!(generated.contains("let mut v_value = PhpSlot::new(Json::Int(v_value));"));
 }
}

#[cfg(test)]
mod native_scalar_variable_reference_tests {
 use super::*;
 use crate::lexer::lex;
 #[test]
 fn integer_variable_reference_promotes_to_shared_slot() {
  let generated=compile(&lex("<?php $value = 10; $alias = &$value; $alias = 25; echo $value;").unwrap()).unwrap();
  assert!(generated.contains("let mut v_value = PhpSlot::new(Json::Int(v_value));"));
  assert!(generated.contains("v_value.bind_reference()"));
 }
 #[test]
 fn string_variable_reference_promotes_to_shared_slot() {
  let generated=compile(&lex("<?php $value = 'old'; $alias = &$value; $alias = 'new'; echo $value;").unwrap()).unwrap();
  assert!(generated.contains("let mut v_value = PhpSlot::new(Json::Text(v_value));"));
 }
 #[test]
 fn boolean_variable_reference_promotes_to_shared_slot() {
  let generated=compile(&lex("<?php $value = true; $alias = &$value; $alias = false; echo $value;").unwrap()).unwrap();
  assert!(generated.contains("let mut v_value = PhpSlot::new(Json::Bool(v_value));"));
 }
}

#[cfg(test)]
mod mixed_array_comparison_regression {
 use super::*;
 #[test]
 fn mixed_array_scalar_comparison_compiles() {
  for expr in ["$a == false", "$a != true", "$a === false", "$a !== 0", "false == $a", "0 !== $a", "'x' == $a"] {
   let src=format!("<?php $a = []; if ({expr}) {{ echo 'yes'; }}");
   let generated=compile(&crate::lexer::lex(&src).unwrap()).expect(&format!("failed: {expr}"));
   assert!(generated.contains("phix_checked_equality"));
  }
 }
}

#[cfg(test)]
mod array_union_assignment_tests {
 use super::*;
 use crate::lexer::lex;
 #[test]
 fn union_assignment_codegen() {
  let generated=compile(&lex("<?php $a = [1]; $a += [2 => 3]; echo $a[2];").unwrap()).unwrap();
  assert!(generated.contains("phix_array_union_assignment_checked"));
  assert!(generated.contains("v_a.replace"));
 }
 #[test]
 fn union_assignment_scalar_left_compiles_with_runtime_type_check() {
  let generated=compile(&lex("<?php $a = 1; $a += [2];").unwrap()).unwrap();
  assert!(generated.contains("PhpSlot::new(Json::Int(v_a))"));
  assert!(generated.contains("phix_array_union_assignment_checked"));
 }
 #[test]
 fn integer_plus_equals_preserves_arithmetic() {
  let generated=compile(&lex("<?php $a = 1; $a += 2; echo $a;").unwrap()).unwrap();
  assert!(generated.contains("v_a += phix_rhs"));
 }
}

#[cfg(test)]
mod fatal_array_short_circuit_tests {
 use super::*;
 use crate::lexer::lex;
 #[test] fn top_level_stops_after_invalid_union() {
  let code=compile(&lex("<?php $a = [1] + 5; echo 'unreachable';").unwrap()).unwrap();
  assert!(code.contains("if phix_has_runtime_error() { return; }"));
  assert!(code.find("phix_has_runtime_error").unwrap() < code.find("unreachable").unwrap());
 }
 #[test] fn nested_block_stops_after_invalid_union() {
  let code=compile(&lex("<?php if (true) { $a = [1] + 5; echo 'unreachable'; }").unwrap()).unwrap();
  assert!(code.find("phix_has_runtime_error").unwrap() < code.find("unreachable").unwrap());
 }
}

#[cfg(test)]
mod fatal_echo_argument_tests {
 use super::*;
 use crate::lexer::lex;
 #[test]
 fn echo_guard_precedes_first_output_after_failing_expression() {
  let code=compile(&lex("<?php echo [1] + 2, 'AFTER';").unwrap()).unwrap();
  assert!(code.contains("if phix_has_runtime_error() { return; } ctx.echo"));
  assert!(code.find("if phix_has_runtime_error() { return; } ctx.echo").unwrap() < code.find("AFTER").unwrap());
 }
 #[test]
 fn echo_guard_is_present_for_each_argument() {
  let code=compile(&lex("<?php echo 'before', [1] + 2, 'after';").unwrap()).unwrap();
  assert_eq!(code.matches("if phix_has_runtime_error() { return; } ctx.echo").count(),3);
 }
}

#[cfg(test)]
mod array_condition_error_guards {
 use super::*;
 use crate::lexer::lex;
 #[test]
 fn if_condition_stops_before_entering_branch_on_recursive_array_error() {
  let generated=compile(&lex("<?php $a = [1]; if ($a == $a) { echo 'entered'; }").unwrap()).unwrap();
  assert!(generated.contains("if phix_has_runtime_error() { return; } if phix_condition"));
 }
 #[test]
 fn while_condition_stops_before_entering_body_on_array_error() {
  let generated=compile(&lex("<?php $a = [1]; while ($a > $a) { echo 'entered'; }").unwrap()).unwrap();
  assert!(generated.contains("if phix_has_runtime_error() { return; } phix_condition"));
 }
}

#[cfg(test)]
mod exception_foundation_tests {
 use super::*;
 use crate::lexer::lex;
 #[test] fn typed_catch_compiles(){let s=compile(&lex("<?php try { $a = [1] + 5; } catch (TypeError $e) { echo 'caught'; }").unwrap()).unwrap();assert!(s.contains("phix_error_matches"));assert!(s.contains("phix_take_comparison_error"));}
 #[test] fn finally_compiles(){let s=compile(&lex("<?php try { echo 'before'; } finally { echo 'after'; }").unwrap()).unwrap();assert!(s.contains("phix_deferred_error"));assert!(s.contains("after"));}
 #[test] fn missing_handler_rejected(){assert!(compile(&lex("<?php try { echo 'x'; }").unwrap()).is_err());}
}

#[cfg(test)]
mod exception_finally_pending_error_tests {
 use super::*;
 use crate::lexer::lex;
 #[test]
 fn unmatched_error_is_suspended_for_finally() {
  let code=compile(&lex("<?php try { $a = [1] + 5; } catch (ValueError $e) { echo 'wrong'; } finally { echo 'cleanup'; }").unwrap()).unwrap();
  assert!(code.contains("let phix_deferred_error = phix_take_comparison_error()"));
  assert!(code.contains("phix_restore_runtime_error(phix_original_error)"));
 }
 #[test]
 fn finally_only_suspends_pending_error() {
  let code=compile(&lex("<?php try { $a = [1] + 5; } finally { echo 'cleanup'; }").unwrap()).unwrap();
  assert!(code.contains("let phix_deferred_error = phix_take_comparison_error()"));
 }
}

#[cfg(test)]
mod exception_method_compiler_tests {
 use super::*;
 #[test]
 fn exception_method_calls_compile() {
  let src="<?php try { throw new Exception(\"test\"); } catch (Exception $e) { echo $e->getMessage(), $e->getCode(), $e->getPrevious(); }";
  let result=compile(&crate::lexer::lex(src).unwrap());
  assert!(result.is_ok(),"{result:?}");
 }
}

#[cfg(test)]
mod typed_throw_propagation_tests {
 use super::*;
 use crate::lexer::lex;
 #[test]
 fn typed_function_throw_is_compiled_with_error_unwind() {
  for (ty, value) in [("int", "1"), ("bool", "true"), ("string", "'ok'")] {
   let source=format!("<?php function fail(): {ty} {{ throw new Exception('failed'); return {value}; }} try {{ echo fail(); }} catch (Exception $e) {{ echo $e->getMessage(); }}");
   let generated=compile(&lex(&source).unwrap()).unwrap();
   assert!(generated.contains("return Err(phix_take_comparison_error().unwrap());"));
   assert!(generated.contains("phix_throw_builtin_with_previous"));
  }
 }
}

#[cfg(test)]
mod typed_try_catch_guards {
 use super::*;
 use crate::lexer::lex;
 #[test]
 fn typed_function_accepts_exception_handling_without_return_in_handler() {
  let src="<?php function f(): int { try { throw new Exception('x'); } catch (Exception $e) { echo $e->getMessage(); } finally { echo 'done'; } return 7; } echo f();";
  let code=compile(&lex(src).unwrap()).unwrap();
  assert!(code.contains("phix_deferred_error"));
  assert!(code.contains("phix_error_matches"));
 }
 #[test]
 fn typed_function_rejects_return_inside_try_until_unwinding_is_implemented() {
  let src="<?php function f(): int { try { return 4; } finally { echo 'done'; } } echo f();";
  assert!(compile(&lex(src).unwrap()).is_err());
 }
}

#[cfg(test)]
mod exception_expression_short_circuit_tests {
 use super::*;
 use crate::lexer::lex;
 #[test]
 fn typed_return_expression_checks_pending_exception() {
  let src="<?php function value(): int { return 3; } function fail(): int { return value(); } echo fail();";
  let code=compile(&lex(src).unwrap()).unwrap();
  assert!(code.contains("let phix_return_value ="));
  assert!(code.contains("if phix_has_runtime_error() { return Err(phix_take_comparison_error().unwrap()); } return Ok(phix_return_value);"));
 }
 #[test]
 fn nested_function_call_does_not_run_after_throwing_argument() {
  let src="<?php function consume(int $v): int { echo 'BAD'; return $v; } function fail(): int { return consume(throw new Exception('stop')); } echo fail();";
  // Throw expressions currently produce a dynamic Json value, so a typed
  // argument fails static checking. Preserve that limitation explicitly.
  assert!(compile(&lex(src).unwrap()).is_err());
 }
 #[test]
 fn typed_call_is_guarded_against_pending_runtime_errors() {
  let src="<?php function consume(int $v): int { echo 'BAD'; return $v; } $a = [1]; echo consume($a + 5);";
  assert!(compile(&lex(src).unwrap()).is_err());
  let src="<?php function consume(int $v): int { return $v; } echo consume(2);";
  let code=compile(&lex(src).unwrap()).unwrap();
  assert!(code.contains("if phix_has_runtime_error() { None } else { Some(phix_unwrap_function_outcome(fn_consume(ctx, phix_arg_0))) }"));
 }
}

#[cfg(test)]
mod terminal_return_control_flow_regression {
    use super::compile;
    use crate::lexer::lex;
    #[test]
    fn typed_function_cannot_claim_return_from_conditional_only() {
        let src = "<?php function f(): int { if (true) { return 2; } }";
        assert!(compile(&lex(src).unwrap()).is_err());
    }
    #[test]
    fn typed_function_accepts_unconditional_return_after_conditional() {
        let src = "<?php function f(): int { if (true) { echo 'x'; } return 2; }";
        assert!(compile(&lex(src).unwrap()).is_ok(), "{:?}", compile(&lex(src).unwrap()));
    }
}

#[cfg(test)]
mod unsupported_nonlocal_return_regressions {
    use super::compile;
    use crate::lexer::lex;
    fn rejects(src: &str) {
        let result = compile(&lex(src).unwrap());
        assert!(result.unwrap_err().contains("requires structured unwinding"));
    }
    #[test] fn void_function_return_in_try_is_not_silently_swallowed() {
        rejects("<?php function f(): void { try { return; } finally { echo 'cleanup'; } } f();");
    }
    #[test] fn void_function_return_in_catch_is_not_silently_swallowed() {
        rejects("<?php function f(): void { try { throw new Exception('x'); } catch (Exception $e) { return; } } f();");
    }
    #[test] fn void_function_return_in_finally_is_not_silently_swallowed() {
        rejects("<?php function f(): void { try { echo 'body'; } finally { return; } } f();");
    }
}

#[cfg(test)]
mod structured_loop_control_guards {
 use super::*;
 use crate::lexer::lex;
 #[test] fn break_reports_missing_structured_unwinding() {
  let err=compile(&lex("<?php while (true) { break; }").unwrap()).unwrap_err();
  assert!(err.contains("structured loop-control unwinding"),"{err}");
 }
 #[test] fn continue_reports_missing_structured_unwinding() {
  let err=compile(&lex("<?php while (true) { continue; }").unwrap()).unwrap_err();
  assert!(err.contains("structured loop-control unwinding"),"{err}");
 }
}

#[cfg(test)]
mod conditional_return_ir_integration_tests {
    use super::*;
    use crate::lexer::lex;
    #[test]
    fn typed_function_with_both_branches_returning_passes_ir_validation() {
        let src = "<?php function choice(): int { if (true) { return 1; } else { return 2; } } echo choice();";
        assert!(compile(&lex(src).unwrap()).is_ok(), "{:?}", compile(&lex(src).unwrap()));
    }
    #[test]
    fn typed_function_with_missing_branch_return_is_rejected() {
        let src = "<?php function choice(): int { if (true) { return 1; } else { echo 2; } } echo choice();";
        assert!(compile(&lex(src).unwrap()).unwrap_err().contains("may fall through"), "{:?}", compile(&lex(src).unwrap()));
    }
}

#[cfg(test)]
mod native_expression_ir_integration_tests {
    use super::*;
    use crate::lexer::lex;
    #[test]
    fn typed_arithmetic_is_lowered_through_expression_ir() {
        let source = "<?php function total(int $a, int $b): int { return $a + $b * 2; } echo total(3, 4);";
        let generated = compile(&lex(source).unwrap()).unwrap();
        assert!(generated.contains("let phix_ir_left"));
        assert!(generated.contains("phix_ir_left * phix_ir_right"));
        assert!(generated.contains("phix_ir_left + phix_ir_right"));
    }
}

#[cfg(test)]
mod php_logical_truthiness_ir_tests {
    use super::compile;
    use crate::lexer::lex;
    fn generate(src: &str) -> String { compile(&lex(src).unwrap()).unwrap() }
    #[test]
    fn integer_operands_convert_to_php_boolean() {
        let code = generate("<?php echo 0 || 7; echo 2 && 3;");
        assert!(code.contains("!= 0"));
        assert!(code.contains("phix_ir_left"));
    }
    #[test]
    fn string_zero_is_false_and_nonempty_strings_are_true() {
        let code = generate("<?php echo '0' || 'hello';");
        assert!(code.contains("phix_truthy_string != \"0\""));
        assert!(code.contains("!phix_truthy_string.is_empty()"));
    }
    #[test]
    fn dynamic_array_values_use_runtime_truthiness() {
        let code = generate("<?php $a = []; echo $a || true;");
        assert!(code.contains(".truthy()"));
    }
    #[test]
    fn mixed_scalar_types_keep_lazy_rhs() {
        let code = generate("<?php echo false && 'side effect'; echo 1 || 'unused';");
        assert!(code.contains("phix_ir_left &&"));
        assert!(code.contains("phix_ir_left ||"));
    }
}

#[cfg(test)]
mod php_truthiness_condition_tests {
    use super::*;
    use crate::lexer::lex;
    fn generated(src: &str) -> String { compile(&lex(src).unwrap()).unwrap() }
    #[test]
    fn integer_condition_uses_php_truthiness() {
        let code = generated("<?php if (0) { echo 1; } else { echo 2; }");
        assert!(code.contains("!= 0"));
    }
    #[test]
    fn string_condition_handles_php_zero_string() {
        let code = generated("<?php if ('0') { echo 1; } else { echo 2; }");
        assert!(code.contains("!= \"0\""));
    }
    #[test]
    fn unary_not_converts_integer_and_string() {
        let code = generated("<?php echo !0; echo !'0'; echo !'hello';");
        assert!(code.contains("!= 0"));
        assert!(code.contains("!= \"0\""));
    }
    #[test]
    fn boolean_condition_keeps_native_boolean() {
        let code = generated("<?php if (true) { echo 1; }");
        assert!(code.contains("if phix_condition"));
    }
}

#[cfg(test)]
mod typed_function_body_boundary_regression {
    use super::compile;
    use crate::lexer::lex;

    #[test]
    fn generated_typed_function_body_is_a_statement_not_a_tail_expression() {
        let source = "<?php function result(): int { return 7; } echo result();";
        let generated = compile(&lex(source).unwrap()).unwrap();
        assert!(generated.contains("fn fn_result(ctx: &mut RequestContext) -> Result<i64, std::sync::Arc<PhixRuntimeError>>"));
        assert!(generated.contains("}
; unreachable!("), "generated: {generated}");
    }
}

#[cfg(test)]
mod typed_guard_codegen_tests {
    use super::compile;
    use crate::lexer::lex;

    #[test]
    fn echo_inside_typed_function_uses_typed_error_exit() {
        let source = "<?php function emit(): int { echo 'before'; return 42; } echo emit();";
        let generated = compile(&lex(source).unwrap()).unwrap();
        assert!(generated.contains("let phix_echo_value ="));
        assert!(generated.contains("if phix_has_runtime_error() { return Err(phix_take_comparison_error().unwrap()); } ctx.echo"));
        assert!(generated.contains("fn fn_emit("));
    }

    #[test]
    fn while_condition_inside_typed_function_uses_typed_error_exit() {
        let source = "<?php function count(): int { while (false) { echo 'unused'; } return 7; } echo count();";
        let generated = compile(&lex(source).unwrap()).unwrap();
        assert!(generated.contains("if phix_has_runtime_error() { return Err(phix_take_comparison_error().unwrap()); } phix_condition"));
        assert!(!generated.contains("if phix_has_runtime_error() { return; } phix_condition"));
    }

    #[test]
    fn void_function_keeps_unit_error_exit() {
        let source = "<?php function emit(): void { echo 'hello'; while (false) { echo 'never'; } } emit();";
        let generated = compile(&lex(source).unwrap()).unwrap();
        assert!(generated.contains("if phix_has_runtime_error() { return; } ctx.echo"));
        assert!(generated.contains("if phix_has_runtime_error() { return; } phix_condition"));
    }
}

#[cfg(test)]
mod ordered_argument_exception_tests {
 use super::*;
 use crate::lexer::lex;
 #[test]
 fn multi_argument_call_guards_each_argument_before_the_next() {
  let src="<?php function sum(int $a, int $b, int $c): int { return $a + $b + $c; } echo sum(1, 2, 3);";
  let generated=compile(&lex(src).unwrap()).unwrap();
  let a=generated.find("let phix_arg_0 = 1i64").unwrap();
  let b=generated.find("let phix_arg_1 = 2i64").unwrap();
  let c=generated.find("let phix_arg_2 = 3i64").unwrap();
  assert!(a<b && b<c);
  assert_eq!(generated.matches("if phix_has_runtime_error() { None }").count(),3);
  assert!(generated.contains("Some(phix_unwrap_function_outcome(fn_sum(ctx, phix_arg_0, phix_arg_1, phix_arg_2)))"));
 }
 #[test]
 fn void_function_arguments_are_guarded_without_unwrapping_a_value() {
  let src="<?php function print_two(int $a, int $b): void { echo $a; echo $b; } print_two(1, 2);";
  let generated=compile(&lex(src).unwrap()).unwrap();
  assert!(generated.contains("let _ = { let phix_arg_0"));
  assert!(generated.contains("Some(fn_print_two(ctx, phix_arg_0, phix_arg_1))"));
 }
}

#[cfg(test)]
mod call_argument_guard_regressions {
 use super::*;
 use crate::lexer::lex;
 fn generated(src:&str)->String { compile(&lex(src).unwrap()).unwrap() }
 #[test]
 fn single_argument_call_guards_argument_exception_before_invocation() {
  let src="<?php function inner(): int { return 1; } function outer(int $x): int { return $x; } echo outer(inner());";
  let code=generated(src);
  assert!(code.contains("let phix_arg_0 ="));
  assert!(code.contains("if phix_has_runtime_error() { None } else { Some(phix_unwrap_function_outcome(fn_outer(ctx, phix_arg_0))) }"));
 }
 #[test]
 fn void_single_argument_call_is_guarded() {
  let src="<?php function inner(): int { return 1; } function consume(int $x): void { echo $x; } consume(inner());";
  let code=generated(src);
  assert!(code.contains("Some(fn_consume(ctx, phix_arg_0))"));
  assert!(code.contains("if !phix_has_runtime_error() { let _ ="));
 }
 #[test]
 fn zero_argument_void_call_checks_pending_error() {
  let src="<?php function ping(): void { echo 1; } ping();";
  let code=generated(src);
  assert!(code.contains("if !phix_has_runtime_error() { fn_ping(ctx); }"));
 }
}

#[cfg(test)]
mod assignment_exception_order_tests {
 use super::*;
 use crate::lexer::lex;
 fn generated(src:&str)->String { compile(&lex(src).unwrap()).unwrap() }
 #[test]
 fn existing_variable_assignment_checks_rhs_before_commit() {
  let code=generated("<?php function value(): int { return 2; } $x = 1; $x = value();");
  assert!(code.contains("let phix_rhs = { if phix_has_runtime_error()"));
  assert!(code.contains("v_x = phix_rhs;"));
  assert!(code.contains("if phix_has_runtime_error() { return; } v_x = phix_rhs;"));
 }
 #[test]
 fn new_variable_assignment_checks_rhs_before_binding() {
  let code=generated("<?php function value(): int { return 2; } $x = value();");
  assert!(code.contains("if phix_has_runtime_error() { return; } let mut v_x: i64 = phix_rhs;"));
 }
 #[test]
 fn integer_add_assign_checks_rhs_before_mutation() {
  let code=generated("<?php function value(): int { return 2; } $x = 1; $x += value();");
  assert!(code.contains("if phix_has_runtime_error() { return; } v_x += phix_rhs;"));
 }
}

#[cfg(test)]
mod logical_exception_guard_tests {
    use super::*;
    use crate::lexer::lex;
    #[test]
    fn or_skips_rhs_when_lhs_throws_even_if_false() {
        let code = compile(&lex("<?php function fail(): bool { throw new Exception('fail'); } function side(): bool { return true; } $value = fail() || side();").unwrap()).unwrap();
        assert!(code.contains("if phix_has_runtime_error() { false } else { phix_ir_left ||"));
        assert_eq!(code.matches("fn_side(ctx)").count(), 1);
    }
    #[test]
    fn and_skips_rhs_when_lhs_throws_even_if_true() {
        let code = compile(&lex("<?php function fail(): bool { throw new Exception('fail'); } function side(): bool { return true; } $value = fail() && side();").unwrap()).unwrap();
        assert!(code.contains("if phix_has_runtime_error() { false } else { phix_ir_left &&"));
        assert_eq!(code.matches("fn_side(ctx)").count(), 1);
    }
}

#[cfg(test)]
mod array_exception_order_tests {
 use super::*;
 use crate::lexer::lex;
 fn generated(source:&str)->String {compile(&lex(source).unwrap()).unwrap()}
 #[test]
 fn key_exception_prevents_value_and_later_entries() {
  let rust=generated("<?php function key(): int { return 1; } function value(): int { return 2; } $a = [key() => value(), 9];");
  let key=rust.find("let phix_array_key_0:").unwrap();
  let guard=rust[key..].find("if phix_has_runtime_error()").unwrap()+key;
  let cell=rust.find("let phix_array_cell_0").unwrap();
  let next=rust.find("let phix_array_key_1:").unwrap();
  assert!(key<guard && guard<cell && cell<next);
 }
 #[test]
 fn array_elements_are_evaluated_once_in_order() {
  let rust=generated("<?php function first(): int { return 1; } function second(): int { return 2; } $a = [first(), second()];");
  assert_eq!(rust.matches("let phix_array_cell_0").count(),1);
  assert_eq!(rust.matches("let phix_array_cell_1").count(),1);
  assert!(rust.find("let phix_array_cell_0").unwrap()<rust.find("let phix_array_cell_1").unwrap());
 }
}

#[cfg(test)]
mod index_exception_order_tests {
 use super::*;
 use crate::lexer::lex;
 fn generated(src:&str)->String {compile(&lex(src).unwrap()).unwrap()}
 #[test]
 fn index_key_is_not_evaluated_after_base_throws() {
  let rust=generated("<?php function key(): int { return 1; } $a = [1]; $b = $a[key()];");
  let start=rust.find("let phix_index_base =").unwrap();
  let guard=start+rust[start..].find("if phix_has_runtime_error()").unwrap();
  let key=rust.find("let phix_index_key =").unwrap();
  assert!(start<guard && guard<key);
  assert_eq!(rust.matches("fn_key(ctx)").count(),1);
 }
 #[test]
 fn nested_index_keys_are_guarded_in_order() {
  let rust=generated("<?php function first(): int { return 0; } function second(): int { return 1; } $a = [[1,2]]; $b = $a[first()][second()];");
  assert_eq!(rust.matches("let phix_index_key =").count(),2);
  assert_eq!(rust.matches("fn_first(ctx)").count(),1);
  assert_eq!(rust.matches("fn_second(ctx)").count(),1);
  assert!(rust.find("fn_first(ctx)").unwrap()<rust.find("fn_second(ctx)").unwrap());
 }
}

#[cfg(test)]
mod array_write_exception_order_tests {
 use super::compile;
 use crate::lexer::lex;
 #[test]
 fn array_write_guards_key_before_rhs() {
  let code=compile(&lex("<?php function fail(): int { return 1; } function later(): int { return 2; } $a = []; $a[fail()] = later();").unwrap()).unwrap();
  let write=code.find("phix_write_key_0").unwrap();
  let tail=&code[write..];
  assert!(tail.find("phix_has_runtime_error()").unwrap()<tail.find("let phix_write_value").unwrap());
  assert!(tail.find("let phix_write_value").unwrap()<tail.find("set_path(").unwrap());
 }
 #[test]
 fn append_write_guards_rhs_before_mutation() {
  let code=compile(&lex("<?php function later(): int { return 2; } $a = []; $a[] = later();").unwrap()).unwrap();
  let tail=&code[code.find("let phix_write_value").unwrap()..];
  assert!(tail.find("phix_has_runtime_error()").unwrap()<tail.find("append_path(").unwrap());
 }
}

#[cfg(test)]
mod unset_exception_order_tests {
 use super::compile;
 use crate::lexer::lex;
 #[test]
 fn nested_unset_guards_every_key_before_mutation() {
  let src="<?php function first(): int { return 0; } function second(): int { return 1; } $a = [[1,2]]; unset($a[first()][second()]);";
  let code=compile(&lex(src).unwrap()).unwrap();
  let tail=&code[code.find("let phix_unset_key_0").unwrap()..];
  assert!(tail.find("phix_has_runtime_error()").unwrap()<tail.find("let phix_unset_key_1").unwrap());
  assert!(tail.find("let phix_unset_key_1").unwrap()<tail.find("unset_path(").unwrap());
  assert_eq!(tail.matches("unset_path(").count(),1);
 }
 #[test]
 fn multiple_unsets_are_guarded_in_source_order() {
  let src="<?php function first(): int { return 0; } function second(): int { return 1; } $a = [1,2]; unset($a[first()], $a[second()]);";
  let code=compile(&lex(src).unwrap()).unwrap();
  let start=code.find("let phix_unset_key_0").unwrap();
  let tail=&code[start..];
  let first_mutation=tail.find("unset_path(").unwrap();
  let second_key=tail[first_mutation+1..].find("let phix_unset_key_0").unwrap()+first_mutation+1;
  assert!(first_mutation<second_key);
  assert!(tail[second_key..].contains("phix_has_runtime_error()"));
 }
}

#[cfg(test)]
mod exception_constructor_order_tests {
    use super::*;
    use crate::lexer::lex;
    #[test]
    fn statement_constructor_arguments_guarded_before_later_arguments() {
        let source = "<?php function fail(): string { throw new Exception('inner'); } function code(): int { return 3; } throw new Exception(fail(), code());";
        let generated = compile(&lex(source).unwrap()).unwrap();
        let pos = generated.rfind("let phix_throw_message =").unwrap();
        let fragment = &generated[pos..];
        assert!(fragment.find("if phix_has_runtime_error()").unwrap() < fragment.find("let phix_throw_code =").unwrap());
        assert!(fragment.find("let phix_throw_code =").unwrap() < fragment.find("let phix_throw_previous =").unwrap());
        assert!(fragment.find("let phix_throw_previous =").unwrap() < fragment.find("phix_throw_builtin_with_previous").unwrap());
    }
    #[test]
    fn expression_constructor_arguments_guarded_before_later_arguments() {
        let source = "<?php function fail(): string { throw new Exception('inner'); } function code(): int { return 3; } echo throw new Exception(fail(), code());";
        let generated = compile(&lex(source).unwrap()).unwrap();
        let pos = generated.rfind("let phix_throw_message =").unwrap();
        let fragment = &generated[pos..];
        assert!(fragment.contains("if phix_has_runtime_error() { Json::Null } else { let phix_throw_code ="));
    }
}

#[cfg(test)]
mod nested_throw_operand_regression {
 use super::*;
 use crate::lexer::lex;
 #[test]
 fn throw_statement_preserves_exception_from_operand() {
  // A throwing expression of Throwable type is needed to exercise the guard.
  let src="<?php $e = null; throw $e;";
  let generated=compile(&lex(src).unwrap()).unwrap();
  assert!(generated.contains("let phix_throw_operand ="));
  assert!(generated.contains("if !phix_has_runtime_error() { phix_throw_value(phix_throw_operand); }"));
 }
 #[test]
 fn throw_expression_preserves_exception_from_operand() {
  let src="<?php $e = null; $value = true || throw $e;";
  let generated=compile(&lex(src).unwrap()).unwrap();
  assert!(generated.contains("let phix_throw_operand ="));
  assert!(generated.contains("if !phix_has_runtime_error() { phix_throw_value(phix_throw_operand); }"));
 }
}

#[cfg(test)]
mod terminal_typed_function_codegen_tests {
    use super::*;
    use crate::lexer::lex;

    #[test]
    fn throw_only_typed_function_has_explicit_terminal_boundary() {
        for ty in ["int", "bool", "string"] {
            let src = format!("<?php function fail(): {ty} {{ throw new Exception('failed'); }}");
            let generated = compile(&lex(&src).unwrap()).unwrap();
            assert!(generated.contains("unreachable!(\"Phix verified terminal function fail fell through\")"));
        }
    }

    #[test]
    fn conditional_throw_and_return_are_terminal() {
        let src = "<?php function choose(bool $flag): int { if ($flag) { throw new Exception('no'); } else { return 5; } }";
        let generated = compile(&lex(src).unwrap()).unwrap();
        assert!(generated.contains("Phix verified terminal function choose fell through"));
    }

    #[test]
    fn void_functions_do_not_need_typed_terminal() {
        let generated = compile(&lex("<?php function done(): void { echo 'ok'; }").unwrap()).unwrap();
        assert!(!generated.contains("Phix verified terminal function done fell through"));
    }
}

#[cfg(test)]
mod compat098_scalar_union_exception_order {
 use super::*;
 use crate::lexer::lex;
 #[test]
 fn scalar_array_union_does_not_read_or_mutate_after_rhs_throws() {
  let output=compile(&lex("<?php function value(): int { return 2; } $a = [1]; $a += value();").unwrap()).unwrap();
  let rhs=output.rfind("let phix_rhs =").unwrap();
  let check=output[rhs..].find("if phix_has_runtime_error()").unwrap();
  let read=output[rhs..].find("let phix_left =").unwrap();
  assert!(check < read);
 }
}

#[cfg(test)]
mod spl_exception_hierarchy_codegen_tests {
 use super::*;
 use crate::lexer::lex;
 #[test]
 fn spl_exception_subclass_throw_and_parent_catch_compile() {
  let source = "<?php try { throw new InvalidArgumentException('bad'); } catch (LogicException $e) { echo 'logic'; }";
  let generated = compile(&lex(source).unwrap()).unwrap();
  assert!(generated.contains("phix_throw_builtin_with_previous(\"InvalidArgumentException\""));
  assert!(generated.contains("phix_error_matches(&phix_error, \"LogicException\")"));
 }
 #[test]
 fn spl_exception_multicatch_compiles() {
  let source = "<?php try { throw new OutOfBoundsException('bad'); } catch (RuntimeException|TypeError $e) { echo 'caught'; }";
  let generated = compile(&lex(source).unwrap()).unwrap();
  assert!(generated.contains("phix_error_matches(&phix_error, \"RuntimeException\")"));
  assert!(generated.contains("phix_error_matches(&phix_error, \"TypeError\")"));
 }
}

#[cfg(test)]
mod exception_constructor_without_parentheses_tests {
    use super::*;
    use crate::lexer::lex;
    #[test]
    fn statement_throw_new_exception_without_parentheses() {
        let rust = compile(&lex("<?php try { throw new InvalidArgumentException; } catch (LogicException $e) { echo 'caught'; }").unwrap()).unwrap();
        assert!(rust.contains("phix_throw_builtin_with_previous(\"InvalidArgumentException\""));
    }
    #[test]
    fn expression_throw_new_exception_without_parentheses() {
        let rust = compile(&lex("<?php $v = false || throw new RuntimeException;").unwrap()).unwrap();
        assert!(rust.contains("phix_throw_builtin_with_previous(\"RuntimeException\""));
    }
    #[test]
    fn parenthesized_constructor_still_accepts_arguments() {
        let rust = compile(&lex("<?php throw new Exception('message', 7);").unwrap()).unwrap();
        assert!(rust.contains("phix_throw_builtin_with_previous(\"Exception\""));
    }
}

#[cfg(test)]
mod coalesce_tests {
 use super::*;
 use crate::lexer::lex;
 fn generate(source:&str)->Result<String,String>{compile(&lex(source)?)}
 #[test] fn null_coalescing_is_lazy_and_preserves_non_null_values(){
  let rust=generate("<?php $a = null; $b = [1]; echo $a ?? $b;").unwrap();
  assert!(rust.contains("phix_coalesce_left"));
  assert!(rust.contains("!matches!(phix_coalesce_left, Json::Null)"));
  assert!(rust.contains("else { "));
 }
 #[test] fn null_coalescing_is_right_associative(){
  let rust=generate("<?php $a = null; $b = null; $c = [3]; echo $a ?? $b ?? $c;").unwrap();
  assert_eq!(rust.matches("let phix_coalesce_left").count(),2);
  let outer=rust.find("let phix_coalesce_left").unwrap();
  assert!(rust[outer..].contains("else { { let phix_coalesce_left"));
 }
 #[test] fn null_coalescing_does_not_swallow_left_exception(){
  let rust=generate("<?php function boom(): int { throw new Exception('bad'); } $a = [boom()]; echo $a[0] ?? [2];").unwrap();
  assert!(rust.contains("if phix_has_runtime_error() { Json::Null } else if !matches!"));
 }
}

#[cfg(test)]
mod ternary_compat_tests {
 use super::compile;
 use crate::lexer::lex;
 fn gen(src:&str)->Result<String,String>{compile(&lex(src)?)}
 #[test] fn standard_ternary_is_lazy(){
  let rust=gen("<?php $a = 1; $b = 2; $c = 3; echo $a ? $b : $c;").unwrap();
  assert!(rust.contains("let phix_ternary_value"));
  assert!(rust.contains("else if"));
 }
 #[test] fn shorthand_ternary_evaluates_condition_once(){
  let rust=gen("<?php function next(): int { return 2; } echo next() ?: 5;").unwrap();
  assert_eq!(rust.matches("let phix_ternary_value =").count(),1);
 }
 #[test] fn nested_ternary_requires_parentheses(){
  assert!(gen("<?php echo 1 ? 2 : 3 ? 4 : 5;").is_err());
  assert!(gen("<?php echo 1 ? (2 ? 3 : 4) : 5;").is_ok());
 }
}

#[cfg(test)]
mod typed_result_boundary_tests {
 use super::compile;
 use crate::lexer::lex;
 #[test]
 fn typed_functions_return_explicit_outcomes() {
  let rust = compile(&lex("<?php function value(): int { return 7; } echo value();").unwrap()).unwrap();
  assert!(rust.contains("-> Result<i64, std::sync::Arc<PhixRuntimeError>>"));
  assert!(rust.contains("return Ok(phix_return_value)"));
  assert!(rust.contains("phix_unwrap_function_outcome(fn_value(ctx))"));
 }
 #[test]
 fn typed_call_arguments_preserve_result_boundary() {
  let rust = compile(&lex("<?php function f(int $x): int { return $x; } echo f(3);").unwrap()).unwrap();
  assert!(rust.contains("phix_unwrap_function_outcome(fn_f(ctx, phix_arg_0))"));
 }
}

#[cfg(test)]
mod direct_result_codegen_tests {
 use super::*;
 use crate::lexer::lex;
 #[test]
 fn typed_return_uses_direct_ok_without_value_closure() {
  let generated=compile(&lex("<?php function value(): int { return 7; } echo value();").unwrap()).unwrap();
  assert!(generated.contains("return Ok(phix_return_value)"));
  assert!(!generated.contains("let phix_value = (||"));
 }
 #[test]
 fn typed_exception_guard_returns_err_not_default() {
  let generated=compile(&lex("<?php function value(): int { echo 'x'; return 7; } echo value();").unwrap()).unwrap();
  assert!(generated.contains("return Err(phix_take_comparison_error().unwrap())"));
  assert!(!generated.contains("return Default::default();"));
 }
}

#[cfg(test)]
mod direct_result_return_tests {
 use super::*;
 use crate::lexer::lex;
 #[test]
 fn zero_arg_function_return_propagates_result_without_placeholder() {
  let code=compile(&lex("function inner(): int { return 4; } function outer(): int { return inner(); }").unwrap()).unwrap();
  assert!(code.contains("return fn_inner(ctx);"));
  assert!(!code.contains("phix_unwrap_function_outcome(fn_inner(ctx))"));
 }
 #[test]
 fn matching_argument_call_forwards_result_and_checks_each_argument() {
  let code=compile(&lex("function inner(int $x, int $y): int { return $x + $y; } function outer(int $v): int { return inner($v, 2); }").unwrap()).unwrap();
  assert!(code.contains("return fn_inner(ctx, phix_forward_arg_0, phix_forward_arg_1);"));
  assert!(code.contains("let phix_forward_arg_0 = v_v;"));
  assert!(code.contains("let phix_forward_arg_1 = "));
  assert!(!code.contains("phix_unwrap_function_outcome(fn_inner(ctx"));
 }
 #[test]
 fn nested_argument_calls_guard_exception_before_forwarding() {
  let code=compile(&lex("function get(): int { return 2; } function inner(int $x): int { return $x; } function outer(): int { return inner(get()); }").unwrap()).unwrap();
  assert!(code.contains("return fn_inner(ctx, phix_forward_arg_0);"));
  assert!(code.contains("if phix_has_runtime_error() { return Err(phix_take_comparison_error().unwrap()); }"));
 }
 #[test]
 fn ordinary_call_expression_remains_on_existing_path() {
  let code=compile(&lex("function inner(): int { return 4; } function outer(): int { return inner() + 1; }").unwrap()).unwrap();
  assert!(!code.contains("return fn_inner(ctx);"));
 }
}

#[cfg(test)]
mod direct_nested_result_tests {
 use super::*;
 use crate::lexer::lex;
 #[test]
 fn nested_zero_arg_call_forwards_result_without_placeholder() {
  let source="<?php function inner(): int { return 7; } function outer(int $x): int { return $x; } function wrapper(): int { return outer(inner()); }";
  let generated=compile(&lex(source).unwrap()).unwrap();
  assert!(generated.contains("let phix_forward_arg_0 = fn_inner(ctx)?;"));
  assert!(generated.contains("return fn_outer(ctx, phix_forward_arg_0);"));
 }
 #[test]
 fn nested_call_argument_type_is_validated() {
  let source="<?php function inner(): bool { return true; } function outer(int $x): int { return $x; } function wrapper(): int { return outer(inner()); }";
  assert!(compile(&lex(source).unwrap()).is_err());
 }
}
