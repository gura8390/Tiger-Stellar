use super::*;

fn action_case(action: &str, event: &str) -> Engine {
    let mut e = make();
    let actions = ast(&mut e, action);
    e.program.index(File {
        path: "actions.txt".into(),
        category: "on_actions".into(),
        ast: actions.into_iter().map(Arc::new).collect(),
        target: false,
    });
    let events = ast(&mut e, event);
    e.program.index(File {
        path: "events.txt".into(),
        category: "events".into(),
        ast: events.into_iter().map(Arc::new).collect(),
        target: false,
    });
    e
}
fn action_run(action: &str, event: &str, script: &str) -> Engine {
    let mut e = action_case(action, event);
    let body = ast(&mut e, script);
    e.effects(&body, &Context::new("country"), &mut State::default());
    e.analyze();
    e
}
#[test]
fn action_explicit_from_is_not_shifted_into_the_caller_root() {
    let e = action_run(
        "custom={events={b}}",
        "country_event={id=b is_triggered_only=yes immediate={from={set_planet_flag=ok} fromfrom={set_country_flag=ok}}}",
        "home_planet={fire_on_action={on_action=custom scopes={from=this fromfrom=root}}}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "on-action-default-context-unverified"));
    let input = e.event_inputs.values().next().unwrap();
    assert_eq!(input.ctx.froms[0].label(), "planet");
    assert_eq!(input.ctx.froms[1].label(), "country");
    assert!(input.ctx.root.unknown);
}
#[test]
fn action_explicit_wrong_from_is_detected_with_no_named_bindings() {
    let e = action_run(
        "custom={events={b}}",
        "country_event={id=b is_triggered_only=yes immediate={from={set_planet_flag=wrong}}}",
        "fire_on_action={on_action=custom scopes={from=root}}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "scope"));
}
#[test]
fn action_sparse_from_slots_do_not_fill_omitted_slots_with_this() {
    let e = action_run(
        "custom={events={b}}",
        "country_event={id=b is_triggered_only=yes immediate={from={set_planet_flag=unknown} fromfrom={set_country_flag=ok}}}",
        "fire_on_action={on_action=custom scopes={fromfrom=root}}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    let input = e.event_inputs.values().next().unwrap();
    assert!(input.ctx.froms[0].unknown);
    assert_eq!(input.ctx.froms[1].label(), "country");
}
#[test]
fn action_root_and_prev_and_default_from_are_not_borrowed() {
    let e = action_run(
        "custom={events={b}}",
        "country_event={id=b is_triggered_only=yes immediate={root={set_planet_flag=unknown} prev={set_country_flag=unknown} from={set_planet_flag=unknown}}}",
        "fire_on_action={on_action=custom}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "scope-type-unknown"));
    assert!(has(&e, "caller-context-unknown"));
}
#[test]
fn action_missing_fixed_identity_is_checked_in_unreachable_code() {
    let e = action_run(
        "custom={}",
        "",
        "if={limit={always=no} fire_on_action={on_action=missing}}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
}
#[test]
fn action_dynamic_identity_remains_unknown() {
    let e = action_run("custom={}", "", "fire_on_action={on_action=$ACTION$}");
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "on-action-expansion-unverified"));
}
#[test]
fn action_nested_unexpanded_parameter_is_not_misread_as_a_fixed_reference() {
    let e = events("country_event={id=a immediate={fire_on_action={on_action=$ACTION$}}}");
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "on-action-expansion-unverified"));
}
#[test]
fn action_parent_shape_and_fields_are_checked() {
    for script in [
        "fire_on_action=custom",
        "fire_on_action={}",
        "fire_on_action>={on_action=custom}",
        "fire_on_action={on_action={x=custom}}",
        "fire_on_action={on_action=custom typo=1}",
        "fire_on_action={on_action=custom on_action=custom}",
        "fire_on_action={on_action=custom scopes=root}",
    ] {
        let e = action_run("custom={}", "", script);
        assert!(errors(&e) > 0, "{script}: {:?}", e.diagnostics.rows);
    }
}
#[test]
fn action_override_shape_names_operators_and_duplicates_are_checked() {
    for field in [
        "root=root",
        "prev=root",
        "typo=root",
        "from={root=yes}",
        "from>=root",
        "from=root FROM=root",
    ] {
        let e = action_run(
            "custom={}",
            "",
            &format!("fire_on_action={{on_action=custom scopes={{{field}}}}}"),
        );
        assert!(errors(&e) > 0, "{field}: {:?}", e.diagnostics.rows);
    }
}
#[test]
fn action_deep_scope_slots_are_bounded_not_declared_invalid() {
    let e = action_run(
        "custom={}",
        "",
        "fire_on_action={on_action=custom scopes={fromfromfromfromfromfromfrom=root}}",
    );
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "analysis-limit"));
    assert!(!e.complete);
}
#[test]
fn action_null_scope_contract_is_explicitly_unknown() {
    let e = action_run(
        "custom={}",
        "",
        "fire_on_action={on_action=custom scopes={from=none}}",
    );
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "on-action-null-scope-unverified"));
}
#[test]
fn action_zero_weight_references_checked_but_not_dispatched() {
    let e = action_run(
        "custom={random_events={0=b 0=missing 5=0}}",
        "country_event={id=b is_triggered_only=yes immediate={set_planet_flag=wrong}}",
        "fire_on_action={on_action=custom}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-event"));
    assert!(e.event_inputs.is_empty());
    assert!(!e.diagnostics.rows.iter().any(|d| d.key == "scope"));
}
#[test]
fn action_event_lists_reject_bad_weights_and_keyed_entries() {
    for list in [
        "events={bad=b}",
        "random_events={-1=b}",
        "random_events={1e999=b}",
        "random_events={1>=b}",
        "events={b={}}",
    ] {
        let e = action_run(
            &format!("custom={{{list}}}"),
            "country_event={id=b is_triggered_only=yes}",
            "fire_on_action={on_action=custom}",
        );
        assert!(errors(&e) > 0, "{list}: {:?}", e.diagnostics.rows);
    }
}
#[test]
fn action_callback_writes_are_possible_not_guaranteed() {
    let mut e = action_case("custom={effect={from={save_event_target_as=made}}}", "");
    let nodes = ast(
        &mut e,
        "fire_on_action={on_action=custom scopes={from=root}}",
    );
    let mut state = State::default();
    e.effects(&nodes, &Context::new("country"), &mut state);
    assert_eq!(
        state.get("event_target:made").availability,
        Availability::Maybe
    );
    assert!(has(&e, "on-action-effect-engine-unverified"));
}
#[test]
fn action_effect_rebinding_does_not_keep_the_old_type_as_certain() {
    let mut e = action_case("custom={effect={from={save_event_target_as=t}}}", "");
    let nodes = ast(
        &mut e,
        "home_planet={save_event_target_as=t} fire_on_action={on_action=custom scopes={from=root}}",
    );
    let mut state = State::default();
    e.effects(&nodes, &Context::new("country"), &mut state);
    let value = state.get("event_target:t");
    assert!(value.types.known.contains("planet"));
    assert!(value.types.known.contains("country"));
}
#[test]
fn action_event_writes_do_not_flow_back_to_the_caller() {
    let mut e = action_case(
        "custom={events={b}}",
        "country_event={id=b is_triggered_only=yes immediate={save_event_target_as=made}}",
    );
    let body = ast(&mut e, "fire_on_action={on_action=custom}");
    let mut state = State::default();
    e.effects(&body, &Context::new("country"), &mut state);
    e.analyze();
    assert_ne!(
        state.get("event_target:made").availability,
        Availability::Definite
    );
}
#[test]
fn action_recursive_effect_execution_is_bounded() {
    let e = action_run(
        "custom={effect={fire_on_action={on_action=custom}}}",
        "",
        "fire_on_action={on_action=custom}",
    );
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "analysis-limit"));
    assert!(!e.complete);
}
#[test]
fn action_target_declaration_fallback_does_not_inject_an_unknown_fired_context() {
    let mut e = make();
    let nodes = ast(&mut e, "custom={events={b}}");
    e.program.index(File {
        path: "actions.txt".into(),
        category: "on_actions".into(),
        ast: nodes.into_iter().map(Arc::new).collect(),
        target: true,
    });
    let nodes = ast(
        &mut e,
        "country_event={id=a immediate={fire_on_action={on_action=custom scopes={from=root}}}} country_event={id=b is_triggered_only=yes immediate={from={set_country_flag=ok}}}",
    );
    e.program.index(File {
        path: "events.txt".into(),
        category: "events".into(),
        ast: nodes.into_iter().map(Arc::new).collect(),
        target: true,
    });
    e.analyze();
    let b = e
        .event_inputs
        .iter()
        .filter(|(k, _)| k.starts_with("b\n"))
        .collect::<Vec<_>>();
    assert_eq!(b.len(), 1, "{:?}", e.diagnostics.rows);
    assert_eq!(b[0].1.ctx.froms[0].label(), "country");
    assert!(!has(&e, "on-action-caller-context-unknown"));
}
#[test]
fn action_target_declaration_without_a_caller_still_checks_its_event() {
    let mut e = action_case(
        "custom={events={b}}",
        "country_event={id=b is_triggered_only=yes immediate={set_planet_flag=wrong}}",
    );
    e.program.files[0].target = true;
    e.analyze();
    assert!(has(&e, "on-action-caller-context-unknown"));
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "scope"));
}
#[test]
fn action_transitive_reads_retain_caller_saved_targets() {
    let mut e = make();
    let actions = ast(&mut e, "custom={events={b}}");
    e.program.index(File {
        path: "actions.txt".into(),
        category: "on_actions".into(),
        ast: actions.into_iter().map(Arc::new).collect(),
        target: false,
    });
    let body = ast(
        &mut e,
        "country_event={id=start immediate={save_event_target_as=shared country_event={id=a}}} country_event={id=a is_triggered_only=yes immediate={fire_on_action={on_action=custom scopes={from=event_target:shared}}}} country_event={id=b is_triggered_only=yes immediate={from={set_planet_flag=wrong}}}",
    );
    e.program.index(File {
        path: "events.txt".into(),
        category: "events".into(),
        ast: body.into_iter().map(Arc::new).collect(),
        target: true,
    });
    e.analyze();
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "scope" && d.message.contains("set_planet_flag")),
        "{:?}",
        e.diagnostics.rows
    );
}
#[test]
fn action_unknown_scalar_effect_form_is_not_a_confirmed_shape_error() {
    let e = action_run(
        "custom={effect=some_helper}",
        "",
        "fire_on_action={on_action=custom}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "on-action-effect-form-unverified"));
}
#[test]
fn action_block_effect_requires_assignment() {
    let e = action_run(
        "custom={effect>={from={set_country_flag=ok}}}",
        "",
        "fire_on_action={on_action=custom scopes={from=root}}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "operator"));
}
#[test]
fn action_fallback_precedes_unknown_event_root_fallback() {
    let mut e = action_case(
        "custom={events={b}}",
        "country_event={id=b is_triggered_only=yes immediate={root={set_planet_flag=unknown}}}",
    );
    for file in &mut e.program.files {
        file.target = true;
    }
    e.analyze();
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "scope-type-unknown"));
    let inputs = e.event_inputs.values().collect::<Vec<_>>();
    assert_eq!(inputs.len(), 1);
    assert!(inputs[0].ctx.root.unknown);
}
#[test]
fn action_macro_call_expands_before_fixed_reference_and_scope_checks() {
    for (name, receiver, expected) in [
        ("custom", "set_country_flag", 0),
        ("missing", "set_country_flag", 1),
        ("custom", "set_planet_flag", 1),
    ] {
        let mut e = action_case(
            "custom={events={b}}",
            &format!(
                "country_event={{id=b is_triggered_only=yes immediate={{from={{{receiver}=value}}}}}}"
            ),
        );
        let nodes = ast(
            &mut e,
            "helper={fire_on_action={on_action=$ACTION$ scopes={from=$TARGET$}}}",
        );
        e.program.index(File {
            path: "helper.txt".into(),
            category: "scripted_effects".into(),
            ast: nodes.into_iter().map(Arc::new).collect(),
            target: false,
        });
        let call = ast(&mut e, &format!("helper={{ACTION={name} TARGET=root}}"));
        e.effects(&call, &Context::new("country"), &mut State::default());
        e.analyze();
        assert_eq!(
            errors(&e),
            expected,
            "{name}/{receiver}: {:?}",
            e.diagnostics.rows
        );
    }
}
#[test]
fn action_memory_directives_are_not_action_names_or_event_references() {
    let mut e = action_case(
        "optimize_memory custom={events={optimize_memory b}}",
        "country_event={id=b is_triggered_only=yes}",
    );
    e.program.files[0].target = true;
    e.analyze();
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(
        !e.diagnostics
            .rows
            .iter()
            .any(|d| d.message.contains("unknown event optimize_memory"))
    );
    let mut empty = action_case("optimize_memory custom={}", "");
    empty.program.files[0].target = true;
    empty.analyze();
    assert!(
        empty.diagnostics.rows.is_empty(),
        "{:?}",
        empty.diagnostics.rows
    );
}
