use super::*;

fn timeline_engine() -> Engine {
    let mut e = make();
    let definitions = ast(
        &mut e,
        "timeline_crisis={} timeline_two={types={country planet}} timeline_fleet={types={fleet}}",
    );
    e.program.index(File {
        path: "fixture.txt".into(),
        category: "timeline_events".into(),
        ast: definitions.into_iter().map(Arc::new).collect(),
        target: false,
    });
    e.program.entities.insert(
        "gfx_spritetype".into(),
        ["GFX_evt_dimensional_horror".into(), "custom_sprite".into()]
            .into_iter()
            .collect(),
    );
    e.program.loc.insert(
        "l_english".into(),
        [
            "TIMELINE_EVENT_CRISIS_UNBIDDEN_TOOLTIP".into(),
            "TIMELINE_EVENT_CRISIS_UNBIDDEN".into(),
            "GOOD".into(),
            "TIP".into(),
        ]
        .into_iter()
        .collect(),
    );
    e
}

fn timeline_case(body: &str) -> Engine {
    let mut e = timeline_engine();
    let nodes = ast(&mut e, body);
    e.effects(&nodes, &Context::new("country"), &mut State::default());
    e
}

#[test]
fn timeline_native_unbidden_override_is_valid() {
    // events/timeline_events.txt:3064-3072, unchanged effect.
    let e = timeline_case(
        "add_timeline_event={type=timeline_crisis override_id=timeline_unbidden override_tooltip=\"TIMELINE_EVENT_CRISIS_UNBIDDEN_TOOLTIP\" override_types={} override_text={\"text:TIMELINE_EVENT_CRISIS_UNBIDDEN\"} override_texture={\"background:GFX_evt_dimensional_horror\"} targets={}}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "timeline-gui-binding-unverified"));
}

#[test]
fn timeline_explicit_country_planet_targets_match_positionally() {
    let e = timeline_case(
        "save_event_target_as=c home_planet={save_event_target_as=p} add_timeline_event={type=timeline_crisis override_id=x override_types={country planet} targets={event_target:c event_target:p}}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn timeline_database_type_list_is_used_without_an_override() {
    let e = timeline_case(
        "save_event_target_as=c home_planet={save_event_target_as=p} add_timeline_event={type=timeline_two targets={event_target:c event_target:p}}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn timeline_explicit_override_types_replace_database_types() {
    let e = timeline_case(
        "add_timeline_event={type=timeline_two override_id=x override_types={country} targets={this}}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn timeline_type_and_target_counts_must_match() {
    let e = timeline_case(
        "add_timeline_event={type=timeline_crisis override_id=x override_types={country planet} targets={this}}",
    );
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "timeline-target-count")
    );
}

#[test]
fn timeline_declared_planet_does_not_accept_country() {
    let e = timeline_case(
        "add_timeline_event={type=timeline_crisis override_id=x override_types={planet} targets={this}}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "scope"));
}

#[test]
fn timeline_effect_requires_country_receiver() {
    let mut e = timeline_engine();
    let nodes = ast(&mut e, "add_timeline_event={type=timeline_crisis}");
    e.effects(&nodes, &Context::new("ship"), &mut State::default());
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "scope"));
}

#[test]
fn timeline_requires_database_type() {
    let e = timeline_case("add_timeline_event={targets={}}");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-field"));
}

#[test]
fn timeline_any_override_requires_override_id() {
    let e =
        timeline_case("add_timeline_event={type=timeline_crisis override_text={\"text:GOOD\"}}");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-field"));
}

#[test]
fn timeline_missing_database_entry_is_detected() {
    let e = timeline_case("add_timeline_event={type=missing_timeline}");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
}

#[test]
fn timeline_missing_texture_sprite_is_detected() {
    let e = timeline_case(
        "add_timeline_event={type=timeline_crisis override_id=x override_texture={\"background:GFX_missing\"}}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
}

#[test]
fn timeline_missing_text_localisation_preserves_warning_severity() {
    let e = timeline_case(
        "add_timeline_event={type=timeline_crisis override_id=x override_text={\"text:MISSING\"}}",
    );
    assert_eq!(errors(&e), 0);
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "localization" && d.severity == "warning")
    );
}

#[test]
fn timeline_missing_tooltip_localisation_is_checked() {
    let e = timeline_case(
        "add_timeline_event={type=timeline_crisis override_id=x override_tooltip=MISSING override_tooltip_delayed=TIP}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "localization"));
}

#[test]
fn timeline_override_entries_require_gui_reference_pair() {
    for entry in ["NO_COLON", "text:", ":GOOD"] {
        let e = timeline_case(&format!(
            "add_timeline_event={{type=timeline_crisis override_id=x override_text={{\"{entry}\"}}}}"
        ));
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "value"),
            "{entry}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn timeline_keyed_override_entry_is_not_an_array_element() {
    let e = timeline_case(
        "add_timeline_event={type=timeline_crisis override_id=x override_text={text=GOOD}}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "structure"));
}

#[test]
fn timeline_scalar_override_list_is_rejected() {
    let e = timeline_case(
        "add_timeline_event={type=timeline_crisis override_id=x override_texture=GFX_evt_dimensional_horror}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "structure"));
}

#[test]
fn timeline_parameter_requires_assignment() {
    let e = timeline_case(
        "add_timeline_event={type=timeline_crisis override_id=x override_text>{\"text:GOOD\"}}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "operator"));
}

#[test]
fn timeline_duplicate_parameter_is_detected() {
    let e = timeline_case("add_timeline_event={type=timeline_crisis type=timeline_crisis}");
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "duplicate-field")
    );
}

#[test]
fn timeline_unknown_scope_type_is_not_silently_accepted() {
    let e = timeline_case(
        "add_timeline_event={type=timeline_crisis override_id=x override_types={countri} targets={this}}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "value"));
}

#[test]
fn timeline_unreachable_structure_and_references_are_checked() {
    let e = timeline_case(
        "if={limit={always=no} add_timeline_event={type=timeline_crisis override_id=x override_text={\"NO_COLON\"} override_texture={\"background:GFX_missing\"}}}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "value"));
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
}

#[test]
fn timeline_other_scope_storage_is_not_assumed_from_scope_enum() {
    let mut e = timeline_engine();
    let nodes = ast(
        &mut e,
        "add_timeline_event={type=timeline_fleet targets={event_target:f}}",
    );
    let mut state = State::default();
    state.set(
        "event_target:f".into(),
        Binding::assigned(Types::new("fleet")),
    );
    e.effects(&nodes, &Context::new("country"), &mut state);
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "timeline-target-storage-unverified"));
}

#[test]
fn timeline_unresolved_external_target_remains_unknown() {
    let e = timeline_case(
        "add_timeline_event={type=timeline_crisis override_id=x override_types={planet} targets={event_target:external}}",
    );
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "target-binding-unproven"));
    assert!(has(&e, "scope-type-unknown"));
}

#[test]
fn timeline_missing_database_type_contract_is_not_assumed_empty() {
    let e = timeline_case("add_timeline_event={type=timeline_crisis targets={this}}");
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "timeline-target-types-unverified"));
}

#[test]
fn timeline_unknown_extension_parameter_retains_specific_boundary() {
    let e = timeline_case("add_timeline_event={type=timeline_crisis future_parameter=1}");
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "timeline-parameter-unverified"));
}

#[test]
fn timeline_date_runtime_expression_is_not_an_invented_calendar_error() {
    let e = timeline_case("add_timeline_event={type=timeline_crisis date=not_a_date}");
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "timeline-date-unverified"));
}

#[test]
fn timeline_documented_date_and_two_pairs_in_one_scalar_are_valid() {
    let e = timeline_case(
        "add_timeline_event={type=timeline_crisis date=2300.1.1 override_id=x override_text={\"text:GOOD button:TIP\"} override_texture={\"background:GFX_evt_dimensional_horror icon:custom_sprite\"}}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "timeline-gui-binding-unverified"));
}

#[test]
fn timeline_missing_gui_control_is_unknown_even_when_references_exist() {
    for field in [
        "override_text={\"definitely_missing_control:GOOD\"}",
        "override_texture={\"definitely_missing_control:GFX_evt_dimensional_horror\"}",
    ] {
        let e = timeline_case(&format!(
            "add_timeline_event={{type=timeline_crisis override_id=x {field}}}"
        ));
        assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
        assert!(has(&e, "timeline-gui-binding-unverified"));
    }
}

#[test]
fn timeline_direct_texture_path_is_not_mistaken_for_sprite() {
    let e = timeline_case(
        "add_timeline_event={type=timeline_crisis override_id=x override_texture={\"background:gfx/interface/a.dds\"}}",
    );
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "timeline-texture-reference-unverified"));
}

#[test]
fn timeline_target_reads_do_not_modify_input_state() {
    let mut e = timeline_engine();
    let nodes = ast(
        &mut e,
        "add_timeline_event={type=timeline_crisis override_id=x override_types={country} targets={event_target:c}}",
    );
    let mut state = State::default();
    state.set(
        "event_target:c".into(),
        Binding::assigned(Types::new("country")),
    );
    let before = state.clone();
    e.timeline_parameters(&nodes[0], &Context::new("country"), &state);
    assert_eq!(state, before);
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn timeline_target_array_limit_comes_from_shipped_guide() {
    let types = ["country"; 11].join(" ");
    let targets = ["this"; 11].join(" ");
    let e = timeline_case(&format!(
        "add_timeline_event={{type=timeline_crisis override_id=x override_types={{{types}}} targets={{{targets}}}}}"
    ));
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "value"));
}
