use super::*;

// Current local 4.5.2 exports: triggers.log:7-8,231-234,451-454;
// scopes.log:70-80. THIS is the current object; ROOT and PREV have independent
// meanings. API SHA256s: triggers bc5d0c71e8534a664fcbf2501cd8ba3f1c0ecd24c3a83754af9ce126133dc568;
// scopes 4cbca39019a53de4ab299b4dd66271e3bb70d72ef2a2a48017775fd22142a077.
// Prior binary 1a197701...f626eb4 minimal CLI controls are retained under
// verification/guard-detection-round24/20261024-163459-917.

fn guard_edge_defs(e: &mut Engine, text: &str) {
    let nodes = ast(e, text);
    e.program.index(File {
        path: "guard-definitions.txt".into(),
        category: "scripted_triggers".into(),
        ast: nodes.into_iter().map(Arc::new).collect(),
        target: false,
    });
}
fn guard_edge_effect(defs: &str, text: &str) -> Engine {
    let mut e = make();
    guard_edge_defs(&mut e, defs);
    let nodes = ast(&mut e, text);
    e.effects(&nodes, &Context::unknown_caller(), &mut State::default());
    e
}
fn guard_edge_scope_error(e: &Engine, token: &str) -> bool {
    e.diagnostics
        .rows
        .iter()
        .any(|d| d.severity == "error" && d.key == "scope" && d.message.contains(token))
}

#[test]
fn common_type_guard_reaches_or_and_nor_alternatives() {
    for text in [
        "is_scope_type=country OR={is_star=yes always=yes}",
        "is_scope_type=country NOR={is_star=yes always=no}",
        "OR={is_star=yes always=yes} is_scope_type=country",
        "AND={is_scope_type=country OR={is_star=yes always=yes}}",
    ] {
        let mut e = make();
        let nodes = ast(&mut e, text);
        e.triggers(&nodes, &Context::unknown_caller(), &State::default());
        assert!(
            guard_edge_scope_error(&e, "is_star"),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn common_country_guard_keeps_legal_or_receivers_known() {
    let mut e = make();
    let nodes = ast(&mut e, "is_scope_type=country OR={is_ai=yes always=yes}");
    e.triggers(&nodes, &Context::unknown_caller(), &State::default());
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn or_without_common_guard_does_not_borrow_another_arms_type() {
    for text in [
        "OR={is_scope_type=country is_star=yes}",
        "OR={is_star=yes always=yes}",
    ] {
        let mut e = make();
        let nodes = ast(&mut e, text);
        e.triggers(&nodes, &Context::unknown_caller(), &State::default());
        assert_eq!(errors(&e), 0, "{text}: {:?}", e.diagnostics.rows);
        assert!(
            has(&e, "scope-type-unknown"),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn or_common_existence_fact_reaches_reads_without_self_proving_presence() {
    let mut e = make();
    let nodes = ast(
        &mut e,
        "exists=event_target:t OR={event_target:t={is_scope_type=country} always=yes}",
    );
    e.triggers(&nodes, &Context::new("country"), &State::default());
    assert!(
        !has(&e, "target-binding-unproven"),
        "{:?}",
        e.diagnostics.rows
    );
    let mut e = make();
    let nodes = ast(
        &mut e,
        "OR={event_target:t={is_scope_type=country} event_target:t={is_scope_type=planet}}",
    );
    e.triggers(&nodes, &Context::new("country"), &State::default());
    assert!(
        has(&e, "target-binding-unproven"),
        "{:?}",
        e.diagnostics.rows
    );
}

#[test]
fn same_object_this_guards_reject_wrong_effects_and_accept_legal_effects() {
    for guard in [
        "this={is_scope_type=country}",
        "THIS={is_scope_type=country}",
        "ThIs.THIS={is_scope_type=country}",
        "this={THIS={is_scope_type=country}}",
    ] {
        let bad = guard_edge_effect(
            "",
            &format!("if={{limit={{{guard}}} set_planet_flag=wrong}}"),
        );
        assert!(
            guard_edge_scope_error(&bad, "set_planet_flag"),
            "{guard}: {:?}",
            bad.diagnostics.rows
        );
        let good = guard_edge_effect("", &format!("if={{limit={{{guard}}} set_country_flag=ok}}"));
        assert!(
            good.diagnostics.rows.is_empty(),
            "{guard}: {:?}",
            good.diagnostics.rows
        );
    }
}

#[test]
fn named_target_this_guard_projects_type_and_does_not_hide_its_entry_read() {
    let mut e = make();
    let nodes = ast(
        &mut e,
        "if={limit={event_target:t={THIS={is_scope_type=country}}} event_target:t={set_planet_flag=wrong}}",
    );
    e.effects(&nodes, &Context::new("leader"), &mut State::default());
    assert!(guard_edge_scope_error(&e, "set_planet_flag"));
    assert!(has(&e, "target-binding-unproven"));
}

#[test]
fn this_projection_restores_outer_root_from_prev_and_object_identity() {
    let mut e = make();
    let nodes = ast(&mut e, "THIS.this={is_scope_type=country}");
    let mut ctx = Context::unknown_caller();
    ctx.root = Types::new("fleet");
    ctx.root_object = "independent-root".into();
    ctx.prevs = vec![Types::new("leader")];
    ctx.prev_objects = vec!["independent-prev".into()];
    ctx.froms = vec![Types::new("species")];
    ctx.from_objects = vec!["independent-from".into()];
    let result = e.refine(&nodes, &ctx, &mut State::default(), true).unwrap();
    assert_eq!(result.this, Types::new("country"));
    assert_eq!(result.root, ctx.root);
    assert_eq!(result.froms, ctx.froms);
    assert_eq!(result.prevs, ctx.prevs);
    assert_eq!(result.object, ctx.object);
    assert_eq!(result.prev_objects, ctx.prev_objects);
    assert_eq!(result.from_objects, ctx.from_objects);
    assert_eq!(result.root_object, ctx.root_object);
}

#[test]
fn root_from_and_prev_guards_do_not_lend_their_type_to_this() {
    for selector in ["ROOT", "FROM", "PREV", "owner"] {
        let e = guard_edge_effect(
            "",
            &format!("if={{limit={{{selector}={{is_scope_type=country}}}} set_planet_flag=maybe}}"),
        );
        assert!(
            !guard_edge_scope_error(&e, "set_planet_flag"),
            "{selector}: {:?}",
            e.diagnostics.rows
        );
        assert!(
            has(&e, "scope-type-unknown"),
            "{selector}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn optional_this_and_negative_unknown_do_not_create_a_concrete_type() {
    for text in [
        "if={limit={THIS?={is_scope_type=country}} set_planet_flag=maybe}",
        "if={limit={NOT={THIS={is_scope_type=country}}} set_country_flag=maybe}",
    ] {
        let e = guard_edge_effect("", text);
        assert_eq!(errors(&e), 0, "{text}: {:?}", e.diagnostics.rows);
        assert!(has(&e, "scope-type-unknown"));
    }
}

#[test]
fn negative_this_guard_removes_only_a_known_candidate() {
    let mut e = make();
    let nodes = ast(&mut e, "NOT={THIS={is_scope_type=country}}");
    let ctx = Context::new("country|planet");
    let result = e.refine(&nodes, &ctx, &mut State::default(), true).unwrap();
    assert_eq!(result.this, Types::new("planet"));
    assert_eq!(result.root, ctx.root);
}

#[test]
fn rendered_macro_guards_check_both_country_and_planet_receivers() {
    for (kind, good, bad) in [
        ("country", "set_country_flag", "set_planet_flag"),
        ("planet", "set_planet_flag", "set_country_flag"),
    ] {
        let defs = "is_kind={is_scope_type=$TYPE$}";
        let good = guard_edge_effect(
            defs,
            &format!("if={{limit={{is_kind={{TYPE={kind}}}}} {good}=ok}}"),
        );
        assert!(
            good.diagnostics.rows.is_empty(),
            "{:?}",
            good.diagnostics.rows
        );
        let bad_case = guard_edge_effect(
            defs,
            &format!("if={{limit={{is_kind={{TYPE={kind}}}}} {bad}=wrong}}"),
        );
        assert!(
            guard_edge_scope_error(&bad_case, bad),
            "{:?}",
            bad_case.diagnostics.rows
        );
    }
}

#[test]
fn macro_aliases_and_conditional_parameters_use_the_rendered_boolean_body() {
    let defs = "is_kind={[[TYPE] is_scope_type=$TYPE$ ] [[!TYPE] always=yes ]} wrapper={is_kind={TYPE=country}}";
    let e = guard_edge_effect(defs, "if={limit={wrapper=yes} set_planet_flag=wrong}");
    assert!(
        guard_edge_scope_error(&e, "set_planet_flag"),
        "{:?}",
        e.diagnostics.rows
    );
    let e = guard_edge_effect(defs, "if={limit={is_kind={}} set_planet_flag=maybe}");
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "scope-type-unknown"));
}

#[test]
fn macro_guard_polarity_flows_through_not_or_and_else_if() {
    let defs = "is_kind={is_scope_type=$TYPE$}";
    let e = guard_edge_effect(
        defs,
        "if={limit={OR={is_kind={TYPE=country} is_kind={TYPE=country}}} set_planet_flag=wrong}",
    );
    assert!(guard_edge_scope_error(&e, "set_planet_flag"));
    let mut e = make();
    guard_edge_defs(&mut e, defs);
    let nodes = ast(
        &mut e,
        "if={limit={is_kind={TYPE=country}} set_country_flag=ok} else_if={limit={is_kind={TYPE=planet}} set_country_flag=wrong}",
    );
    e.effects(
        &nodes,
        &Context::new("country|planet"),
        &mut State::default(),
    );
    assert!(
        guard_edge_scope_error(&e, "set_country_flag"),
        "{:?}",
        e.diagnostics.rows
    );
    let mut e = make();
    guard_edge_defs(&mut e, defs);
    let nodes = ast(
        &mut e,
        "if={limit={NOT={is_kind={TYPE=country}}} set_country_flag=wrong}",
    );
    e.effects(
        &nodes,
        &Context::new("country|planet"),
        &mut State::default(),
    );
    assert!(guard_edge_scope_error(&e, "set_country_flag"));
}

#[test]
fn macro_or_alternatives_do_not_borrow_a_different_target_or_kind() {
    let e = guard_edge_effect(
        "is_kind={is_scope_type=$TYPE$}",
        "if={limit={OR={is_kind={TYPE=country} always=yes}} set_planet_flag=maybe}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "scope-type-unknown"));
}

#[test]
fn macro_guard_ast_cache_rechecks_each_callers_context() {
    let mut e = make();
    guard_edge_defs(&mut e, "is_kind={is_scope_type=$TYPE$}");
    let nodes = ast(&mut e, "is_kind={TYPE=country}");
    assert!(
        e.refine(&nodes, &Context::new("planet"), &mut State::default(), true)
            .is_none()
    );
    let result = e
        .refine(
            &nodes,
            &Context::unknown_caller(),
            &mut State::default(),
            true,
        )
        .unwrap();
    assert_eq!(result.this, Types::new("country"));
    assert_eq!(e.script_bodies.len(), 1);
    assert!(e.metrics.script_body_cache_hits > 0);
}

#[test]
fn macro_guard_cache_keeps_rendered_parameter_values_separate() {
    let mut e = make();
    guard_edge_defs(&mut e, "is_kind={is_scope_type=$TYPE$}");
    for kind in ["country", "planet", "country"] {
        let nodes = ast(&mut e, &format!("is_kind={{TYPE={kind}}}"));
        let result = e
            .refine(
                &nodes,
                &Context::unknown_caller(),
                &mut State::default(),
                true,
            )
            .unwrap();
        assert_eq!(result.this, Types::new(kind));
    }
    assert_eq!(e.script_bodies.len(), 2);
}

#[test]
fn macro_guard_cache_key_includes_the_complete_definition_reader_header() {
    let mut e = make();
    let source = "is_kind={is_scope_type=@KIND always=$BOOL$}";
    let mut parsed = e
        .parser
        .parse("reader-guard.txt", source, "@KIND=country")
        .unwrap();
    assert!(parsed.diagnostics.is_empty());
    for n in &mut parsed.ast {
        n.locate("reader-guard.txt", &mut e.program.next_id);
    }
    e.program.index(File {
        path: "reader-guard.txt".into(),
        category: "scripted_triggers".into(),
        ast: parsed.ast.into_iter().map(Arc::new).collect(),
        target: false,
    });
    let nodes = ast(&mut e, "is_kind={BOOL=yes}");
    for kind in ["country", "planet"] {
        e.program.globals = format!("@KIND={kind}");
        let result = e
            .refine(
                &nodes,
                &Context::unknown_caller(),
                &mut State::default(),
                true,
            )
            .unwrap();
        assert_eq!(result.this, Types::new(kind), "{:?}", e.diagnostics.rows);
    }
    assert_eq!(e.script_bodies.len(), 2);
}

#[test]
fn macro_guard_cache_does_not_reuse_a_colliding_id_from_another_source_or_line() {
    let mut e = make();
    for (name, path, line) in [
        ("one", "one.txt", 10),
        ("two", "two.txt", 10),
        ("three", "one.txt", 100),
    ] {
        let mut n = e
            .parser
            .parse(
                path,
                &format!("{name}={{always=$BOOL$ has_ethic=missing_ethic}}"),
                "",
            )
            .unwrap()
            .ast
            .remove(0);
        n.locate(path, &mut e.program.next_id);
        n.id = 0;
        n.line = line;
        e.program.scripts.insert((false, name.into()), Arc::new(n));
        let call = ast(&mut e, &format!("{name}={{BOOL=yes}}"));
        e.call(
            &call[0],
            false,
            &Context::new("country"),
            &mut State::default(),
        );
    }
    assert_eq!(e.script_bodies.len(), 3);
    let missing = e
        .diagnostics
        .rows
        .iter()
        .filter(|d| d.key == "missing-item")
        .collect::<Vec<_>>();
    assert!(missing.iter().any(|d| d.path == "one.txt" && d.line < 50));
    assert!(missing.iter().any(|d| d.path == "two.txt"));
    assert!(missing.iter().any(|d| d.path == "one.txt" && d.line >= 100));
}

#[test]
fn missing_macro_parameters_keep_the_original_error_without_lending_a_type() {
    let e = guard_edge_effect(
        "is_kind={is_scope_type=$TYPE$}",
        "if={limit={is_kind={}} set_planet_flag=maybe}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "macro-argument"));
    assert!(!guard_edge_scope_error(&e, "set_planet_flag"));
    assert!(has(&e, "scope-type-unknown"));
}

#[test]
fn duplicate_and_nonassignment_macro_arguments_do_not_create_guard_facts() {
    for call in [
        "is_kind={TYPE=planet TYPE=country}",
        "is_kind={TYPE>=country}",
        "is_kind={loose TYPE=country}",
        "is_kind={TYPE={country}}",
    ] {
        let e = guard_edge_effect(
            "is_kind={is_scope_type=$TYPE$}",
            &format!("if={{limit={{{call}}} set_planet_flag=maybe}}"),
        );
        assert_eq!(errors(&e), 0, "{call}: {:?}", e.diagnostics.rows);
        assert!(has(&e, "script-guard-contract-unverified"));
        assert!(has(&e, "scope-type-unknown"));
    }
}

#[test]
fn dynamic_macro_scope_values_remain_unknown_instead_of_fake_concrete_types() {
    let e = guard_edge_effect(
        "is_kind={is_scope_type=$TYPE$}",
        "if={limit={is_kind={TYPE=$RUNTIME$}} set_planet_flag=maybe}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "scope-type-expression-unverified"));
    assert!(has(&e, "scope-type-unknown"));
}

#[test]
fn fixed_invalid_scope_values_keep_their_static_error() {
    let e = guard_edge_effect(
        "is_kind={is_scope_type=$TYPE$}",
        "if={limit={is_kind={TYPE=not_a_scope}} set_planet_flag=maybe}",
    );
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "value" && d.message.contains("not_a_scope"))
    );
    assert!(!guard_edge_scope_error(&e, "set_planet_flag"));
}

#[test]
fn unresolved_scope_kind_is_not_a_stable_guard_identity() {
    let mut e = make();
    let nodes = ast(&mut e, "is_scope_type=$KIND$");
    let ctx = Context::unknown_caller();
    assert!(e.guard_key(&nodes, &ctx, true).is_none());
    let frame = e.refine(&nodes, &ctx, &mut State::default(), true).unwrap();
    assert!(frame.this.unknown);
}

#[test]
fn dirty_macro_guard_is_not_cached_and_ordinary_call_still_checks_its_body() {
    let mut e = make();
    guard_edge_defs(&mut e, "is_kind={is_scope_type=$TYPE$ ignored=@UNDEFINED}");
    let nodes = ast(&mut e, "is_kind={TYPE=country}");
    let result = e
        .refine(
            &nodes,
            &Context::unknown_caller(),
            &mut State::default(),
            true,
        )
        .unwrap();
    assert!(result.this.unknown);
    assert!(e.script_bodies.is_empty());
    assert!(has(&e, "script-guard-contract-unverified"));
    e.call(
        &nodes[0],
        false,
        &Context::unknown_caller(),
        &mut State::default(),
    );
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "unknown-trigger" && d.message.contains("ignored"))
    );
    assert!(e.script_bodies.is_empty());
}

#[test]
fn macro_parameter_cannot_escape_the_wrapper_and_lend_a_false_guard_fact() {
    let mut e = make();
    guard_edge_defs(&mut e, "is_kind={is_scope_type=$TYPE$}");
    let nodes = ast(&mut e, "is_kind={TYPE=\"country } stray={ always=no\"}");
    let frame = e
        .refine(
            &nodes,
            &Context::unknown_caller(),
            &mut State::default(),
            true,
        )
        .unwrap();
    assert!(frame.this.unknown);
    assert!(e.script_bodies.is_empty());
    assert!(has(&e, "script-expansion-boundary-unverified"));
    e.call(
        &nodes[0],
        false,
        &Context::unknown_caller(),
        &mut State::default(),
    );
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "unknown-trigger" && d.message.contains("stray"))
    );
    assert!(e.script_bodies.is_empty());
}

#[test]
fn macro_wrapper_boundary_preserves_fixed_references_in_unexpected_roots() {
    let mut e = make();
    guard_edge_defs(&mut e, "is_kind={is_scope_type=$TYPE$}");
    let nodes = ast(
        &mut e,
        "is_kind={TYPE=\"country } has_ethic=missing_ethic AND={ always=no\"}",
    );
    e.call(
        &nodes[0],
        false,
        &Context::unknown_caller(),
        &mut State::default(),
    );
    assert!(has(&e, "script-expansion-boundary-unverified"));
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "missing-item" && d.message.contains("missing_ethic"))
    );
    assert!(e.script_bodies.is_empty());
}

#[test]
fn intact_macro_wrapper_remains_a_clean_boolean_guard() {
    let mut e = make();
    guard_edge_defs(&mut e, "is_kind={is_scope_type=$TYPE$}");
    let nodes = ast(&mut e, "is_kind={TYPE=country}");
    let frame = e
        .refine(
            &nodes,
            &Context::unknown_caller(),
            &mut State::default(),
            true,
        )
        .unwrap();
    assert_eq!(frame.this, Types::new("country"));
    assert_eq!(e.script_bodies.len(), 1);
    assert!(!has(&e, "script-expansion-boundary-unverified"));
}

#[test]
fn escaped_script_value_roots_are_unknown_and_cannot_certify_zero() {
    let (e, _) = value_case(
        "calc={base=$BASE$}",
        "set_variable={which=x value=10} divide_variable={which=x value=\"value:calc|BASE|0 } add=1 modifier={ factor=1|\"}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "script-value-expansion-boundary-unverified"));
    assert!(has(&e, "numeric-divisor-unproven"));
    assert!(
        !e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "value" && d.message.contains("zero"))
    );
}

#[test]
fn escaped_script_value_roots_keep_fixed_reference_errors_without_numeric_evaluation() {
    let (e, _) = value_case(
        &format!("{}calc={{base=$BASE$}}", "\n".repeat(49)),
        "set_variable={which=x value=10} divide_variable={which=x value=\"value:calc|BASE|0 } add=value:missing_value modifier={ factor=1|\"}",
    );
    assert!(has(&e, "script-value-expansion-boundary-unverified"));
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"
        && d.message.contains("missing_value")
        && d.path == "fixture.txt"
        && d.line == 50));
    assert!(
        !e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "value" && d.message.contains("zero"))
    );
}

#[test]
fn intact_script_value_wrapper_still_detects_a_real_zero_and_accepts_nonzero() {
    let (e, _) = value_case(
        "calc={base=$BASE$}",
        "set_variable={which=x value=10} divide_variable={which=x value=value:calc|BASE|0|}",
    );
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "value" && d.message.contains("zero"))
    );
    assert!(!has(&e, "script-value-expansion-boundary-unverified"));
    let (e, _) = value_case(
        "calc={base=$BASE$}",
        "set_variable={which=x value=10} divide_variable={which=x value=value:calc|BASE|2|}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(!has(&e, "script-value-expansion-boundary-unverified"));
}

#[test]
fn count_and_unverified_statement_results_do_not_become_boolean_type_guards() {
    for defs in [
        "is_kind={count_owned_pop_amount={limit={is_scope_type=$TYPE$}}}",
        "is_kind={if={limit={always=yes} is_scope_type=$TYPE$}}",
    ] {
        let mut e = make();
        guard_edge_defs(&mut e, defs);
        let nodes = ast(&mut e, "is_kind={TYPE=country}");
        let result = e
            .refine(
                &nodes,
                &Context::unknown_caller(),
                &mut State::default(),
                true,
            )
            .unwrap();
        assert!(result.this.unknown, "{defs}");
        assert!(has(&e, "script-guard-return-unverified"));
    }
}

#[test]
fn recursive_macro_return_and_guard_budget_never_fabricate_success_or_bottom() {
    let mut e = make();
    guard_edge_defs(&mut e, "loop_guard={loop_guard={TYPE=$TYPE$}}");
    let nodes = ast(&mut e, "loop_guard={TYPE=country}");
    let result = e
        .refine(
            &nodes,
            &Context::unknown_caller(),
            &mut State::default(),
            true,
        )
        .unwrap();
    assert!(result.this.unknown);
    assert!(has(&e, "script-guard-return-unverified"));
    let mut e = make();
    guard_edge_defs(
        &mut e,
        "is_kind={exists=event_target:t is_scope_type=$TYPE$}",
    );
    let text = format!("OR={{{}}}", "is_kind={TYPE=country} ".repeat(600));
    let nodes = ast(&mut e, &text);
    let mut state = State::default();
    let result = e
        .refine(&nodes, &Context::unknown_caller(), &mut state, true)
        .unwrap();
    assert!(result.this.unknown);
    assert_ne!(
        state.get("event_target:t").availability,
        Availability::Definite
    );
}

#[test]
fn macro_boolean_classification_budget_keeps_the_callers_type_unknown() {
    let mut e = make();
    guard_edge_defs(
        &mut e,
        &format!(
            "is_kind={{AND={{{}}} is_scope_type=country}}",
            "always=$BOOL$ ".repeat(130)
        ),
    );
    let nodes = ast(&mut e, "is_kind={BOOL=yes}");
    let result = e
        .refine(
            &nodes,
            &Context::unknown_caller(),
            &mut State::default(),
            true,
        )
        .unwrap();
    assert!(result.this.unknown);
    assert!(has(&e, "script-guard-return-unverified"));
}

#[test]
fn macro_guard_cache_full_still_parses_and_rechecks_the_new_context() {
    let mut e = make();
    guard_edge_defs(
        &mut e,
        "is_kind={is_scope_type=country has_country_flag=$FLAG$}",
    );
    for i in 0..=512 {
        let nodes = ast(&mut e, &format!("is_kind={{FLAG=flag_{i}}}"));
        assert!(
            e.refine(
                &nodes,
                &Context::unknown_caller(),
                &mut State::default(),
                true
            )
            .is_some()
        );
    }
    assert_eq!(e.script_bodies.len(), 512);
    let nodes = ast(&mut e, "is_kind={FLAG=flag_512}");
    let before = e.parser.parses;
    assert!(
        e.refine(&nodes, &Context::new("planet"), &mut State::default(), true)
            .is_none()
    );
    assert_eq!(e.parser.parses - before, 1);
}

fn guard_edge_effect_macro(e: &mut Engine, text: &str) {
    let nodes = ast(e, text);
    e.program.index(File {
        path: "effect-macro.txt".into(),
        category: "scripted_effects".into(),
        ast: nodes.into_iter().map(Arc::new).collect(),
        target: false,
    });
}

#[test]
fn escaped_effect_macro_cannot_export_old_or_new_target_and_numeric_certainty() {
    let mut e = make();
    guard_edge_effect_macro(&mut e, "writer={save_event_target_as=$TARGET$}");
    let nodes = ast(
        &mut e,
        "writer={TARGET=\"first } save_global_event_target_as=second set_variable={which=y value=0} hidden_effect={save_event_target_as=third\"}",
    );
    let mut state = State::default();
    state.set(
        "event_target:old".into(),
        Binding::assigned(Types::new("planet")),
    );
    let x = State::number_key("root", "x");
    state.set_number(x.clone(), crate::domain::NumberFact::assigned(Some(0.0)));
    state
        .guarded
        .insert("remembered".into(), state.bindings.clone());
    state
        .guarded_numbers
        .insert("remembered".into(), state.numbers.clone());
    e.call(&nodes[0], true, &Context::new("country"), &mut state);
    assert!(has(&e, "script-expansion-boundary-unverified"));
    for key in [
        "event_target:old",
        "event_target:first",
        "global_event_target:second",
        "event_target:third",
    ] {
        let binding = state.get(key);
        assert_eq!(binding.availability, Availability::Maybe, "{key}");
        assert!(binding.types.unknown, "{key}");
        assert!(binding.types.known.is_empty(), "{key}");
    }
    assert!(state.number(&x).value().is_none());
    assert_eq!(state.number(&x).availability, Availability::Maybe);
    assert!(!state.numbers.contains_key(&State::number_key("root", "y")));
    assert!(state.guarded.is_empty());
    assert!(state.guarded_numbers.is_empty());
}

#[test]
fn escaped_effect_macro_still_checks_fixed_references_without_running_the_roots() {
    let mut e = make();
    guard_edge_effect_macro(&mut e, "writer={save_event_target_as=$TARGET$}");
    let nodes = ast(
        &mut e,
        "writer={TARGET=\"first } add_building=missing_building hidden_effect={save_event_target_as=second\"}",
    );
    let mut state = State::default();
    e.call(&nodes[0], true, &Context::new("country"), &mut state);
    assert!(has(&e, "script-expansion-boundary-unverified"));
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "missing-item" && d.message.contains("missing_building"))
    );
    assert!(!e.diagnostics.rows.iter().any(|d| d.key == "scope"));
    assert_eq!(
        state.get("event_target:first").availability,
        Availability::Maybe
    );
    assert!(e.script_bodies.is_empty());
}

#[test]
fn single_wrapper_dirty_parse_keeps_the_existing_ordinary_effect_transfer() {
    let mut e = make();
    guard_edge_effect_macro(
        &mut e,
        "writer={save_event_target_as=$TARGET$ ignored=@UNDEFINED}",
    );
    let nodes = ast(&mut e, "writer={TARGET=first}");
    let mut state = State::default();
    e.call(&nodes[0], true, &Context::new("country"), &mut state);
    assert_eq!(
        state.get("event_target:first").availability,
        Availability::Definite
    );
    assert_eq!(state.get("event_target:first").types, Types::new("country"));
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "unknown-effect" && d.message.contains("ignored"))
    );
    assert!(!has(&e, "script-expansion-boundary-unverified"));
    assert!(e.script_bodies.is_empty());
}
