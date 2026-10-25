use super::*;

fn target_writer_file(e: &mut Engine, path: &str, category: &str, text: &str, target: bool) {
    let mut nodes = e.parser.parse(path, text, "").unwrap().ast;
    for node in &mut nodes {
        node.locate(path, &mut e.program.next_id);
    }
    e.program.index(File {
        path: path.into(),
        category: category.into(),
        ast: nodes.into_iter().map(Arc::new).collect(),
        target,
    });
}
fn target_writer_fixture(producers: &str) -> Engine {
    let mut e = make();
    target_writer_file(
        &mut e,
        "producers.txt",
        "scripted_effects",
        producers,
        false,
    );
    target_writer_file(
        &mut e,
        "consumer.txt",
        "events",
        "country_event={id=consumer is_triggered_only=yes immediate={global_event_target:shared={set_country_flag=ok}}}",
        true,
    );
    e
}
fn discover_writers(e: &mut Engine) {
    e.prepare_reads();
    e.prepare_saved_target_types();
}

#[test]
fn writer_type_constructor_is_independent_of_an_unknown_caller() {
    let mut e = target_writer_fixture(
        "producer={create_country={effect={save_global_event_target_as=shared}}}",
    );
    discover_writers(&mut e);
    let types = e
        .saved_target_type_candidates("global_event_target:shared")
        .unwrap();
    assert_eq!(types, Types::new("country"));
    assert!(
        e.saved_target_type_evidence()
            .iter()
            .any(|v| v["kind"] == "saved-target-type-candidates"
                && v["closed_static_writer_set"] == true)
    );
}

#[test]
fn writer_type_discovery_does_not_create_availability() {
    let mut e = target_writer_fixture(
        "producer={create_country={effect={save_global_event_target_as=shared}}}",
    );
    discover_writers(&mut e);
    let state = State::default();
    let n = ast(&mut e, "global_event_target:shared={set_country_flag=ok}").remove(0);
    let frame = e.resolve(&n.key, &Context::new("country"), &state, &n, false);
    assert_eq!(frame.this, Types::new("country"));
    assert_eq!(
        state.get("global_event_target:shared").availability,
        Availability::Absent
    );
    assert!(has(&e, "target-binding-unproven"));
    assert!(
        e.saved_target_type_evidence()
            .iter()
            .all(|v| v["availability_proven"] == false)
    );
}
#[test]
fn writer_type_catalog_does_not_erase_explicit_unknown_bindings() {
    for initial in [Types::new("?"), Types::new("country|?")] {
        let mut e = target_writer_fixture(
            "producer={create_country={effect={save_global_event_target_as=shared}}}",
        );
        discover_writers(&mut e);
        let mut state = State::default();
        state.set(
            "global_event_target:shared".into(),
            Binding {
                types: initial,
                availability: Availability::Maybe,
            },
        );
        let n = ast(
            &mut e,
            "global_event_target:shared={set_fleet_flag=unknown}",
        )
        .remove(0);
        let frame = e.resolve(&n.key, &Context::new("fleet"), &state, &n, false);
        assert!(frame.this.unknown);
        assert!(frame.this.known.contains("country"));
        e.effects(std::slice::from_ref(&n), &Context::new("fleet"), &mut state);
        assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
        assert!(has(&e, "scope-type-unknown"));
    }
}
#[test]
fn writer_type_exists_does_not_erase_a_preexisting_open_binding() {
    for initial in [Types::new("?"), Types::new("country|?")] {
        let mut e = target_writer_fixture(
            "producer={create_country={effect={save_global_event_target_as=shared}}}",
        );
        discover_writers(&mut e);
        let mut state = State::default();
        state.set(
            "global_event_target:shared".into(),
            Binding {
                types: initial,
                availability: Availability::Maybe,
            },
        );
        let body = ast(
            &mut e,
            "if={limit={exists=global_event_target:shared} global_event_target:shared={set_fleet_flag=unknown}}",
        );
        e.effects(&body, &Context::new("fleet"), &mut state);
        assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
        assert!(has(&e, "scope-type-unknown"));
    }
}
#[test]
fn writer_type_delayed_arrival_does_not_reuse_a_closed_past_type() {
    let mut e = target_writer_fixture(
        "producer={create_country={effect={save_global_event_target_as=shared}}} arrival={global_event_target:shared={set_fleet_flag=unknown}}",
    );
    discover_writers(&mut e);
    assert_eq!(
        e.saved_target_type_candidates("global_event_target:shared")
            .unwrap(),
        Types::new("country")
    );
    let mut state = State::default();
    state.set(
        "global_event_target:shared".into(),
        Binding::assigned(Types::new("country")),
    );
    state.set(
        "event_target:destination".into(),
        Binding::assigned(Types::new("planet")),
    );
    let before = state.clone();
    let body = ast(
        &mut e,
        "auto_move_to_planet={target=event_target:destination arrival_effect=arrival}",
    );
    e.effects(&body, &Context::new("fleet"), &mut state);
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert_eq!(state, before);
    assert!(has(&e, "scope-type-unknown"));
    assert!(has(&e, "target-binding-unproven"));
}

#[test]
fn writer_type_closed_set_can_detect_a_wrong_scope_without_presence_proof() {
    let mut e = target_writer_fixture(
        "producer={create_country={effect={save_global_event_target_as=shared}}}",
    );
    discover_writers(&mut e);
    let body = ast(&mut e, "global_event_target:shared={set_planet_flag=wrong}");
    e.effects(&body, &Context::new("country"), &mut State::default());
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "scope"));
    assert!(has(&e, "target-binding-unproven"));
}

#[test]
fn writer_type_exists_guard_cannot_erase_closed_receiver_type() {
    let mut e = target_writer_fixture(
        "producer={create_country={effect={save_global_event_target_as=shared}}}",
    );
    discover_writers(&mut e);
    let body = ast(
        &mut e,
        "if={limit={exists=global_event_target:shared} global_event_target:shared={set_planet_flag=wrong}}",
    );
    e.effects(&body, &Context::new("country"), &mut State::default());
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "scope"));
}

#[test]
fn writer_type_catalog_does_not_resurrect_a_cleared_target() {
    let mut e = target_writer_fixture(
        "producer={create_country={effect={save_global_event_target_as=shared}}}",
    );
    discover_writers(&mut e);
    let body = ast(
        &mut e,
        "clear_global_event_target=shared global_event_target:shared={set_country_flag=ok}",
    );
    let mut state = State::default();
    e.effects(&body, &Context::new("country"), &mut state);
    assert_eq!(
        state.get("global_event_target:shared").availability,
        Availability::Absent
    );
    assert!(has(&e, "target-binding-unproven"));
}

#[test]
fn writer_type_multiple_writers_are_a_union_not_a_preferred_kind() {
    let mut e = target_writer_fixture(
        "one={create_country={effect={save_global_event_target_as=shared}}} two={spawn_planet={class=pc_test init_effect={save_global_event_target_as=shared}}}",
    );
    discover_writers(&mut e);
    assert_eq!(
        e.saved_target_type_candidates("global_event_target:shared")
            .unwrap(),
        Types::new("country|planet")
    );
    let body = ast(&mut e, "global_event_target:shared={is_ai=yes}");
    e.triggers(&body, &Context::new("country"), &State::default());
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "scope-type-ambiguous"));
}

#[test]
fn writer_type_unknown_this_is_not_borrowed_from_the_consumer() {
    let mut e = target_writer_fixture("producer={save_global_event_target_as=shared}");
    discover_writers(&mut e);
    let types = e
        .saved_target_type_candidates("global_event_target:shared")
        .unwrap();
    assert!(types.unknown);
    assert!(types.known.is_empty());
}

#[test]
fn writer_type_root_and_from_are_not_invented_from_constructor_this() {
    for selector in ["root", "from", "prevprev"] {
        let mut e = target_writer_fixture(&format!(
            "producer={{create_country={{effect={{{selector}={{save_global_event_target_as=shared}}}}}}}}"
        ));
        discover_writers(&mut e);
        assert!(
            e.saved_target_type_candidates("global_event_target:shared")
                .unwrap()
                .unknown,
            "{selector}"
        );
    }
}

#[test]
fn writer_type_fixed_selector_output_can_be_proved_without_caller_identity() {
    let mut e = target_writer_fixture("producer={owner={save_global_event_target_as=shared}}");
    discover_writers(&mut e);
    assert_eq!(
        e.saved_target_type_candidates("global_event_target:shared")
            .unwrap(),
        Types::new("country")
    );
}

#[test]
fn writer_type_dynamic_uninstantiated_macro_keeps_the_set_open() {
    let mut e = target_writer_fixture(
        "known={create_country={effect={save_global_event_target_as=shared}}} unknown={save_global_event_target_as=$TARGET$}",
    );
    discover_writers(&mut e);
    let types = e
        .saved_target_type_candidates("global_event_target:shared")
        .unwrap();
    assert!(types.unknown);
    assert!(types.known.contains("country"));
}

#[test]
fn writer_type_actual_macro_call_is_rendered_before_name_and_type_collection() {
    let mut e = target_writer_fixture("write_target={save_global_event_target_as=$TARGET$}");
    target_writer_file(
        &mut e,
        "caller.txt",
        "events",
        "country_event={id=producer immediate={write_target={TARGET=shared}}}",
        false,
    );
    discover_writers(&mut e);
    assert_eq!(
        e.saved_target_type_candidates("global_event_target:shared")
            .unwrap(),
        Types::new("country")
    );
    assert!(
        e.saved_target_type_evidence()
            .iter()
            .any(|v| v["kind"] == "saved-target-writer" && v["source_location_exact"] == false)
    );
}

#[test]
fn writer_type_missing_macro_argument_keeps_other_literals_open() {
    let mut e = target_writer_fixture(
        "write_target={save_global_event_target_as=$TARGET$} known={create_country={effect={save_global_event_target_as=shared}}}",
    );
    target_writer_file(
        &mut e,
        "caller.txt",
        "events",
        "country_event={id=producer immediate={write_target=yes}}",
        false,
    );
    discover_writers(&mut e);
    assert!(
        e.saved_target_type_candidates("global_event_target:shared")
            .unwrap()
            .unknown
    );
}

#[test]
fn writer_type_hidden_inline_helper_is_part_of_the_writer_closure() {
    let mut e = target_writer_fixture(
        "known={create_country={effect={save_global_event_target_as=shared}}} helper={inline_script=writer_inline}",
    );
    e.program.inline.insert(
        "writer_inline".into(),
        (
            "writer_inline.txt".into(),
            "every_owned_planet={save_global_event_target_as=shared}".into(),
        ),
    );
    discover_writers(&mut e);
    assert_eq!(
        e.saved_target_type_candidates("global_event_target:shared")
            .unwrap(),
        Types::new("country|planet")
    );
}

#[test]
fn writer_type_hidden_script_callee_is_walked_through_the_reverse_graph() {
    let mut e = target_writer_fixture(
        "callee={save_global_event_target_as=shared} caller={every_country={callee=yes}}",
    );
    discover_writers(&mut e);
    assert_eq!(
        e.saved_target_type_candidates("global_event_target:shared")
            .unwrap(),
        Types::new("country")
    );
}

#[test]
fn writer_type_scalar_callback_does_not_borrow_an_ordinary_caller_frame() {
    let mut e = target_writer_fixture("callee={save_global_event_target_as=shared}");
    target_writer_file(
        &mut e,
        "caller.txt",
        "events",
        "country_event={id=producer immediate={callee=yes auto_move_to_planet={arrival_effect=callee}}}",
        false,
    );
    discover_writers(&mut e);
    assert!(
        e.saved_target_type_candidates("global_event_target:shared")
            .unwrap()
            .unknown
    );
}

#[test]
fn writer_type_dynamic_script_key_must_expand_or_remain_open() {
    let mut e = target_writer_fixture(
        "known={create_country={effect={save_global_event_target_as=shared}}} delegated={$WRITER$=yes}",
    );
    discover_writers(&mut e);
    assert!(
        e.saved_target_type_candidates("global_event_target:shared")
            .unwrap()
            .unknown
    );
}

#[test]
fn writer_type_same_name_action_fragments_do_not_drop_writers() {
    let mut e = target_writer_fixture("");
    target_writer_file(
        &mut e,
        "action_a.txt",
        "on_actions",
        "on_test={effect={every_owned_planet={save_global_event_target_as=shared}}}",
        false,
    );
    target_writer_file(
        &mut e,
        "action_b.txt",
        "on_actions",
        "on_test={effect={every_owned_fleet={save_global_event_target_as=shared}}}",
        false,
    );
    discover_writers(&mut e);
    assert_eq!(
        e.saved_target_type_candidates("global_event_target:shared")
            .unwrap(),
        Types::new("fleet|planet")
    );
}

#[test]
fn writer_type_effective_definition_override_excludes_old_provider() {
    let mut e = target_writer_fixture(
        "producer={create_country={effect={save_global_event_target_as=shared}}}",
    );
    target_writer_file(
        &mut e,
        "newer.txt",
        "scripted_effects",
        "producer={create_fleet={effect={save_global_event_target_as=shared}}}",
        false,
    );
    discover_writers(&mut e);
    assert_eq!(
        e.saved_target_type_candidates("global_event_target:shared")
            .unwrap(),
        Types::new("fleet")
    );
}

#[test]
fn writer_type_global_alias_is_supported_without_a_suffix_convention() {
    let mut e = target_writer_fixture(
        "producer={create_country={effect={save_global_event_target_as=shared}}}",
    );
    discover_writers(&mut e);
    assert_eq!(
        e.saved_target_type_candidates("event_target:shared")
            .unwrap(),
        Types::new("country")
    );
    assert!(
        e.saved_target_type_candidates("event_target:shared_global")
            .is_none()
    );
}

#[test]
fn writer_type_local_and_global_same_name_candidates_are_both_retained() {
    let mut e = target_writer_fixture(
        "one={create_country={effect={save_global_event_target_as=shared}}} two={create_fleet={effect={save_event_target_as=shared}}}",
    );
    discover_writers(&mut e);
    assert_eq!(
        e.saved_target_type_candidates("event_target:shared")
            .unwrap(),
        Types::new("country|fleet")
    );
    assert_eq!(
        e.saved_target_type_candidates("global_event_target:shared")
            .unwrap(),
        Types::new("country")
    );
}

#[test]
fn writer_type_unknown_inline_source_keeps_the_set_open() {
    let mut e = target_writer_fixture(
        "known={create_country={effect={save_global_event_target_as=shared}}} helper={inline_script=missing_writer_inline}",
    );
    discover_writers(&mut e);
    assert!(
        e.saved_target_type_candidates("global_event_target:shared")
            .unwrap()
            .unknown
    );
}

#[test]
fn writer_type_unrelated_upstream_reader_does_not_become_a_demand() {
    let mut e = target_writer_fixture(
        "known={create_country={effect={save_global_event_target_as=shared}}} upstream_reader={global_event_target:unrelated={set_country_flag=ok}}",
    );
    discover_writers(&mut e);
    let summary = e
        .saved_target_type_evidence()
        .into_iter()
        .find(|v| v["kind"] == "writer-type-discovery-summary")
        .unwrap();
    assert!(
        !summary["demanded_names"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "unrelated")
    );
}

#[test]
fn writer_type_summary_is_deterministic_and_never_claims_runtime_execution() {
    let mut e = target_writer_fixture(
        "producer={start_situation={effect={save_global_event_target_as=shared}}}",
    );
    discover_writers(&mut e);
    let first = e.saved_target_type_evidence();
    discover_writers(&mut e);
    assert_eq!(first, e.saved_target_type_evidence());
    assert_eq!(
        e.saved_target_type_candidates("global_event_target:shared")
            .unwrap(),
        Types::new("situation")
    );
    assert!(first.iter().all(|v| v["availability_proven"] == false));
}

fn writer_cache_callers(e: &mut Engine, country_call: &str, planet_call: &str) {
    target_writer_file(
        e,
        "country_caller.txt",
        "events",
        &format!("country_event={{id=cache_country immediate={{{country_call}}}}}"),
        false,
    );
    target_writer_file(
        e,
        "planet_caller.txt",
        "events",
        &format!("planet_event={{id=cache_planet immediate={{{planet_call}}}}}"),
        false,
    );
}
fn writer_cache_demand_other(e: &mut Engine) {
    target_writer_file(
        e,
        "other_consumer.txt",
        "events",
        "country_event={id=other_consumer immediate={global_event_target:other={set_country_flag=ok}}}",
        true,
    );
}

#[test]
fn writer_macro_ast_cache_rewalks_country_and_planet_and_is_discovery_local() {
    let mut e = target_writer_fixture("write_target={save_global_event_target_as=$TARGET$}");
    writer_cache_callers(
        &mut e,
        "write_target={TARGET=shared}",
        "write_target={TARGET=shared}",
    );
    let before = e.parser.parses;
    discover_writers(&mut e);
    // Two distinct contexts still produce two writer witnesses, but identical
    // rendered source needs only one parse. No semantic result is cached.
    assert_eq!(e.parser.parses - before, 1);
    assert_eq!(
        e.saved_target_type_candidates("global_event_target:shared")
            .unwrap(),
        Types::new("country|planet")
    );
    let first = e.saved_target_type_evidence();
    for path in ["country_caller.txt", "planet_caller.txt"] {
        assert!(first.iter().any(|v| v["kind"] == "saved-target-writer"
            && v["expansion_call_chain"][0]["call_path"] == path));
    }
    let before = e.parser.parses;
    discover_writers(&mut e);
    assert_eq!(e.parser.parses - before, 1, "no cross-discovery cache");
    assert_eq!(first, e.saved_target_type_evidence());
}

#[test]
fn writer_macro_single_artificial_wrapper_still_proves_a_known_candidate() {
    let mut e = target_writer_fixture(
        "write_target={save_global_event_target_as=$TARGET$} producer={create_country={effect={write_target={TARGET=shared}}}}",
    );
    discover_writers(&mut e);
    assert_eq!(
        e.saved_target_type_candidates("global_event_target:shared")
            .unwrap(),
        Types::new("country")
    );
    assert!(e.saved_target_type_evidence().iter().any(|v| {
        v["kind"] == "saved-target-type-candidates"
            && v["target"] == "global_event_target:shared"
            && v["closed_static_writer_set"] == true
    }));
}

#[test]
fn writer_macro_real_two_root_expansion_does_not_close_the_writer_set() {
    let mut e = target_writer_fixture(
        r#"write_target={save_global_event_target_as=$TARGET$} producer={create_country={effect={write_target={TARGET="shared } every_owned_planet={save_global_event_target_as=shared"}}}}"#,
    );
    // This is a syntax-clean, fully balanced two-root AST, not merely a parse
    // failure. The first root alone says Country; its sibling can save Planet.
    let expanded = e
        .parser
        .parse(
            "two-root-control.txt",
            "__writer_body__={\nsave_global_event_target_as=shared } every_owned_planet={save_global_event_target_as=shared\n}",
            "",
        )
        .unwrap();
    assert!(expanded.fatal.is_none() && expanded.diagnostics.is_empty());
    assert_eq!(expanded.ast.len(), 2);
    assert_eq!(expanded.ast[0].key, "__writer_body__");
    assert_eq!(expanded.ast[1].key, "every_owned_planet");

    discover_writers(&mut e);
    let types = e
        .saved_target_type_candidates("global_event_target:shared")
        .unwrap();
    assert!(types.unknown);
    assert!(types.known.is_empty());
    let evidence = e.saved_target_type_evidence();
    assert!(evidence.iter().any(|v| {
        v["kind"] == "open-writer-source"
            && v["reason"] == "scripted macro writer expansion boundary is unverified"
    }));
    assert!(evidence.iter().any(|v| {
        v["kind"] == "saved-target-type-candidates"
            && v["target"] == "global_event_target:shared"
            && v["closed_static_writer_set"] == false
    }));
    let body = ast(
        &mut e,
        "global_event_target:shared={set_planet_flag=unverified}",
    );
    e.effects(&body, &Context::new("country"), &mut State::default());
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "scope-type-unknown"));
    assert!(has(&e, "target-binding-unproven"));
}

#[test]
fn writer_macro_cached_two_root_expansion_keeps_the_boundary_open() {
    let mut e = target_writer_fixture("write_target={save_global_event_target_as=$TARGET$}");
    let call = r#"write_target={TARGET="shared } every_owned_planet={save_global_event_target_as=shared"}"#;
    writer_cache_callers(&mut e, call, call);
    let before = e.parser.parses;
    discover_writers(&mut e);
    assert_eq!(e.parser.parses - before, 1, "syntax AST can be shared");
    let types = e
        .saved_target_type_candidates("global_event_target:shared")
        .unwrap();
    assert!(types.unknown);
    assert!(types.known.is_empty());
    let first = e.saved_target_type_evidence();
    assert!(first.iter().any(|v| {
        v["kind"] == "open-writer-source"
            && v["reason"] == "scripted macro writer expansion boundary is unverified"
    }));
    assert!(!first.iter().any(|v| {
        v["kind"] == "saved-target-type-candidates" && v["closed_static_writer_set"] == true
    }));
    let before = e.parser.parses;
    discover_writers(&mut e);
    assert_eq!(e.parser.parses - before, 1, "cache is discovery-local");
    assert_eq!(first, e.saved_target_type_evidence());
}

#[test]
fn writer_macro_opaque_residual_parameter_does_not_silently_drop_writers() {
    let mut e = target_writer_fixture("write_target={save_global_event_target_as=$TARGET$}");
    writer_cache_callers(
        &mut e,
        r#"write_target={TARGET="$LATER$"}"#,
        r#"write_target={TARGET="$LATER$"}"#,
    );
    discover_writers(&mut e);
    let types = e
        .saved_target_type_candidates("global_event_target:shared")
        .unwrap();
    assert!(types.unknown);
    assert!(types.known.is_empty());
    assert!(e.saved_target_type_evidence().iter().any(|v| {
        v["kind"] == "open-writer-source"
            && v["reason"] == "scripted macro writer expansion boundary is unverified"
    }));
}

#[test]
fn writer_inline_ast_cache_rewalks_callers_and_preserves_each_provenance() {
    let mut e = target_writer_fixture("");
    e.program.inline.insert(
        "writer_inline".into(),
        (
            "writer_inline.txt".into(),
            "save_global_event_target_as=shared".into(),
        ),
    );
    writer_cache_callers(
        &mut e,
        "inline_script=writer_inline",
        "inline_script=writer_inline",
    );
    let before = e.parser.parses;
    discover_writers(&mut e);
    assert_eq!(e.parser.parses - before, 1);
    assert_eq!(
        e.saved_target_type_candidates("global_event_target:shared")
            .unwrap(),
        Types::new("country|planet")
    );
    let evidence = e.saved_target_type_evidence();
    for path in ["country_caller.txt", "planet_caller.txt"] {
        assert!(evidence.iter().any(|v| v["kind"] == "saved-target-writer"
            && v["path"] == "writer_inline.txt"
            && v["source_location_exact"] == false
            && v["expansion_call_chain"][0]["call_path"] == path));
    }
    assert!(evidence.iter().all(|v| v["availability_proven"] == false));
}

#[test]
fn writer_macro_ast_cache_key_includes_real_rendered_parameters() {
    let mut e = target_writer_fixture("write_target={save_global_event_target_as=$TARGET$}");
    writer_cache_demand_other(&mut e);
    writer_cache_callers(
        &mut e,
        "write_target={TARGET=shared}",
        "write_target={TARGET=other}",
    );
    let before = e.parser.parses;
    discover_writers(&mut e);
    assert_eq!(e.parser.parses - before, 2);
    assert_eq!(
        e.saved_target_type_candidates("global_event_target:shared")
            .unwrap(),
        Types::new("country")
    );
    assert_eq!(
        e.saved_target_type_candidates("global_event_target:other")
            .unwrap(),
        Types::new("planet")
    );
}

#[test]
fn writer_inline_ast_cache_key_includes_real_rendered_parameters() {
    let mut e = target_writer_fixture("");
    writer_cache_demand_other(&mut e);
    e.program.inline.insert(
        "writer_inline".into(),
        (
            "writer_inline.txt".into(),
            "save_global_event_target_as=$TARGET$".into(),
        ),
    );
    writer_cache_callers(
        &mut e,
        "inline_script={script=writer_inline TARGET=shared}",
        "inline_script={script=writer_inline TARGET=other}",
    );
    let before = e.parser.parses;
    discover_writers(&mut e);
    assert_eq!(e.parser.parses - before, 2);
    assert_eq!(
        e.saved_target_type_candidates("global_event_target:shared")
            .unwrap(),
        Types::new("country")
    );
    assert_eq!(
        e.saved_target_type_candidates("global_event_target:other")
            .unwrap(),
        Types::new("planet")
    );
}

#[test]
fn writer_inline_ast_cache_key_includes_the_complete_reader_header() {
    let mut e = target_writer_fixture("");
    writer_cache_demand_other(&mut e);
    e.program.inline.insert(
        "writer_inline".into(),
        (
            "writer_inline.txt".into(),
            "save_global_event_target_as=@TARGET ignored=@KEEP".into(),
        ),
    );
    writer_cache_callers(
        &mut e,
        "inline_script=writer_inline",
        "inline_script=writer_inline",
    );
    e.program.globals = "@TARGET=global_default\n@KEEP=7".into();
    e.program
        .readers
        .insert("country_caller.txt".into(), vec!["@TARGET=shared".into()]);
    e.program
        .readers
        .insert("planet_caller.txt".into(), vec!["@TARGET=other".into()]);
    let before = e.parser.parses;
    discover_writers(&mut e);
    // The inline path and rendered bytes match; only the complete header
    // changes. Reader values must not leak from the first caller's parse.
    assert_eq!(e.parser.parses - before, 2);
    assert_eq!(
        e.saved_target_type_candidates("global_event_target:shared")
            .unwrap(),
        Types::new("country")
    );
    assert_eq!(
        e.saved_target_type_candidates("global_event_target:other")
            .unwrap(),
        Types::new("planet")
    );
}

#[test]
fn writer_inline_ast_cache_key_keeps_equal_text_from_distinct_sources_separate() {
    let mut e = target_writer_fixture("");
    for name in ["one", "two"] {
        e.program.inline.insert(
            name.into(),
            (
                format!("{name}.txt"),
                "save_global_event_target_as=shared".into(),
            ),
        );
    }
    writer_cache_callers(&mut e, "inline_script=one", "inline_script=two");
    let before = e.parser.parses;
    discover_writers(&mut e);
    assert_eq!(e.parser.parses - before, 2);
    assert_eq!(
        e.saved_target_type_candidates("global_event_target:shared")
            .unwrap(),
        Types::new("country|planet")
    );
    let evidence = e.saved_target_type_evidence();
    for source in ["one.txt", "two.txt"] {
        assert!(
            evidence
                .iter()
                .any(|v| v["kind"] == "saved-target-writer" && v["path"] == source)
        );
    }
}

#[test]
fn writer_inline_ast_cache_does_not_cache_parse_errors_or_reader_diagnostics() {
    for text in [
        "save_global_event_target_as={",
        "save_global_event_target_as=shared ignored=@UNDEFINED",
    ] {
        let mut e = target_writer_fixture("");
        e.program
            .inline
            .insert("broken".into(), ("broken_inline.txt".into(), text.into()));
        writer_cache_callers(&mut e, "inline_script=broken", "inline_script=broken");
        let before = e.parser.parses;
        discover_writers(&mut e);
        assert_eq!(e.parser.parses - before, 2, "{text}");
        let types = e
            .saved_target_type_candidates("global_event_target:shared")
            .unwrap();
        assert!(types.unknown, "{text}");
        assert!(types.known.is_empty(), "incomplete AST must not be walked");
        let evidence = e.saved_target_type_evidence();
        for path in ["country_caller.txt", "planet_caller.txt"] {
            assert!(evidence.iter().any(|v| v["kind"] == "open-writer-source"
                && v["path"] == path
                && v["reason"] == "inline writer expansion is incomplete"));
        }
    }
}

#[test]
fn writer_inline_ast_cache_does_not_memoize_recursive_walk_boundaries() {
    let mut e = target_writer_fixture("");
    e.program.inline.insert(
        "cycle".into(),
        (
            "cycle.txt".into(),
            "inline_script=cycle save_global_event_target_as=shared".into(),
        ),
    );
    writer_cache_callers(&mut e, "inline_script=cycle", "inline_script=cycle");
    let before = e.parser.parses;
    discover_writers(&mut e);
    assert_eq!(e.parser.parses - before, 1);
    let types = e
        .saved_target_type_candidates("global_event_target:shared")
        .unwrap();
    assert!(types.unknown);
    assert_eq!(types.known, Types::new("country|planet").known);
    assert!(
        e.saved_target_type_evidence()
            .iter()
            .any(|v| v["kind"] == "open-writer-source" && v["reason"] == "recursive inline writer")
    );
    let first = e.saved_target_type_evidence();
    discover_writers(&mut e);
    assert_eq!(first, e.saved_target_type_evidence());
}

#[test]
fn writer_ast_cache_capacity_falls_back_to_parsing_without_skipping_contexts() {
    let mut e = target_writer_fixture("");
    let limit = crate::saved_target_types::EXPANDED_AST_CACHE_LIMIT;
    let mut country_calls = String::new();
    for i in 0..=limit {
        let name = format!("writer_{i}");
        e.program.inline.insert(
            name.clone(),
            (
                format!("{name}.txt"),
                "save_global_event_target_as=shared".into(),
            ),
        );
        country_calls.push_str(&format!("inline_script={name}\n"));
    }
    writer_cache_callers(
        &mut e,
        &country_calls,
        &format!("inline_script=writer_{limit}"),
    );
    let before = e.parser.parses;
    discover_writers(&mut e);
    // The first 512 distinct source triples fit. The final source, including
    // its Planet invocation, must still be parsed and walked after saturation.
    assert_eq!(e.parser.parses - before, (limit + 2) as u64);
    assert_eq!(
        e.saved_target_type_candidates("global_event_target:shared")
            .unwrap(),
        Types::new("country|planet")
    );
    let evidence = e.saved_target_type_evidence();
    assert!(evidence.iter().any(|v| v["kind"] == "saved-target-writer"
        && v["path"] == format!("writer_{limit}.txt")
        && v["receiver_types"]["known"] == serde_json::json!(["planet"])));
}
