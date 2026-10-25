use super::*;

#[test]
fn spawn_planet_unknown_query_objects_do_not_borrow_caller_zero_constants() {
    for expression in ["x", "root.x"] {
        let mut e = make();
        e.program
            .entities
            .entry("planet_classes".into())
            .or_default()
            .insert("pc_test".into());
        let values = ast(&mut e, &format!("v={{base=1 divide={expression}}}"));
        e.program.index(File {
            path: "values.txt".into(),
            category: "script_values".into(),
            ast: values.into_iter().map(Arc::new).collect(),
            target: false,
        });
        let n = ast(
            &mut e,
            "spawn_planet={class=pc_test orbit_angle_offset=value:v}",
        )
        .remove(0);
        let mut inputs = State::default();
        inputs.set_number(
            State::number_key("root", "x"),
            crate::domain::NumberFact::assigned(Some(0.0)),
        );
        let mut state = inputs.clone();
        e.spawn_planet_parameters(&n, &Context::new("galactic_object"), &mut state, &inputs);
        assert_eq!(errors(&e), 0, "{expression}: {:?}", e.diagnostics.rows);
        assert!(has(&e, "spawn-planet-expression-context-unverified"));
        assert!(has(&e, "numeric-divisor-unproven"));
    }
}

#[test]
fn spawn_planet_direct_variable_range_keeps_caller_numeric_facts() {
    let mut initial = State::default();
    initial.set_number(
        State::number_key("root", "minimum"),
        crate::domain::NumberFact::assigned(Some(10.0)),
    );
    let (e, _) = spawn_case(
        "spawn_planet={class=pc_test orbit_distance={min=minimum max=5}}",
        Context::new("galactic_object"),
        initial,
    );
    assert!(
        e.diagnostics.rows.iter().any(|d| d.key == "range-order"),
        "{:?}",
        e.diagnostics.rows
    );
    assert!(!has(&e, "spawn-planet-expression-context-unverified"));
}

#[test]
fn spawn_planet_registered_class_identity_wins_over_scope_or_selector_spelling() {
    let mut e = make();
    e.program.entities.insert(
        "planet_classes".into(),
        ["root", "random_custom"]
            .into_iter()
            .map(String::from)
            .collect(),
    );
    for text in [
        "spawn_planet={class=root}",
        "spawn_planet={class=random_custom}",
    ] {
        let n = ast(&mut e, text).remove(0);
        e.spawn_planet_parameters(
            &n,
            &Context::new("galactic_object"),
            &mut State::default(),
            &State::default(),
        );
    }
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn spawn_planet_native_planet_location_is_supported() {
    let mut initial = State::default();
    initial.set(
        "event_target:target_planet".into(),
        Binding::assigned(Types::new("planet")),
    );
    let (e, _) = spawn_case(
        "spawn_planet={class=pc_test location=event_target:target_planet orbit_location=yes orbit_angle_offset=135 orbit_distance_offset=9.899 size=10 has_ring=no}",
        Context::new("galactic_object"),
        initial,
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn spawn_planet_unproved_numeric_query_frame_does_not_create_a_hard_scope_error() {
    let (e, _) = spawn_case(
        "spawn_planet={class=pc_test orbit_angle_offset=trigger:fleet_power}",
        Context::new("galactic_object"),
        State::default(),
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "spawn-planet-expression-context-unverified"));
    assert!(has(&e, "scope-type-unknown"));
}

fn spawn_case(text: &str, ctx: Context, initial: State) -> (Engine, State) {
    let mut e = make();
    e.program.entities.insert(
        "planet_classes".into(),
        ["pc_test", "pc_ringworld_tech_damaged"]
            .into_iter()
            .map(String::from)
            .collect(),
    );
    let n = ast(&mut e, text).remove(0);
    let mut state = initial.clone();
    e.spawn_planet_parameters(&n, &ctx, &mut state, &initial);
    (e, state)
}

#[test]
fn spawn_planet_shipped_ringworld_call_is_valid() {
    let ctx = Context::new("megastructure").push(Types::new("galactic_object"), "solar_system");
    let (e, _) = spawn_case(
        "spawn_planet={class=pc_ringworld_tech_damaged location=prev orbit_angle_offset=30}",
        ctx,
        State::default(),
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn spawn_planet_random_class_none_location_and_native_ranges_are_valid() {
    let (e, _) = spawn_case(
        "spawn_planet={class=random_colonizable generate_random_name=yes location=none orbit_distance={min=60 max=260} orbit_angle={min=0 max=360} init_effect={set_planet_flag=ok}}",
        Context::new("galactic_object"),
        State::default(),
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn spawn_planet_required_class_structure_and_assignment_are_checked() {
    for (text, key) in [
        ("spawn_planet=yes", "structure"),
        ("spawn_planet={}", "missing-field"),
        ("spawn_planet>={class=pc_test}", "operator"),
        ("spawn_planet={class=missing_class}", "missing-item"),
        ("spawn_planet={class={bad=yes}}", "structure"),
        (
            "spawn_planet={class=pc_test orbit_angle_offset>=2}",
            "operator",
        ),
        ("spawn_planet={class=pc_test init_effect=yes}", "structure"),
    ] {
        let (e, _) = spawn_case(text, Context::new("galactic_object"), State::default());
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn spawn_planet_class_scope_source_is_unknown_not_missing_literal() {
    let (e, _) = spawn_case(
        "spawn_planet={class=root}",
        Context::new("galactic_object"),
        State::default(),
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "spawn-planet-class-source-unverified"));
    let (e, _) = spawn_case(
        "spawn_planet={class=random_unverified}",
        Context::new("galactic_object"),
        State::default(),
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "spawn-planet-class-selector-unverified"));
}

#[test]
fn spawn_planet_boolean_and_numeric_fields_reject_invalid_values() {
    for text in [
        "spawn_planet={class=pc_test orbit_angle_offset=NaN}",
        "spawn_planet={class=pc_test orbit_distance=inf}",
        "spawn_planet={class=pc_test has_ring=maybe}",
        "spawn_planet={class=pc_test orbit_location=maybe}",
    ] {
        let (e, _) = spawn_case(text, Context::new("galactic_object"), State::default());
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "value"),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn spawn_planet_range_shapes_order_and_unknown_defaults_are_distinct() {
    for (text, key) in [
        (
            "spawn_planet={class=pc_test orbit_angle={min=4 max=2}}",
            "range-order",
        ),
        (
            "spawn_planet={class=pc_test orbit_distance={min=1 typo=2}}",
            "unknown-field",
        ),
        (
            "spawn_planet={class=pc_test orbit_distance={min={bad=yes} max=2}}",
            "structure",
        ),
        (
            "spawn_planet={class=pc_test orbit_angle={min=1 min=2 max=3}}",
            "duplicate-field",
        ),
    ] {
        let (e, _) = spawn_case(text, Context::new("galactic_object"), State::default());
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
    let (e, _) = spawn_case(
        "spawn_planet={class=pc_test orbit_distance={min=1}}",
        Context::new("galactic_object"),
        State::default(),
    );
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "numeric-range-unproven"));
}

#[test]
fn spawn_planet_parameters_read_pre_callback_numbers() {
    let mut initial = State::default();
    initial.set_number(
        State::number_key("root", "angle"),
        crate::domain::NumberFact::assigned(Some(30.0)),
    );
    let (e, _) = spawn_case(
        "spawn_planet={class=pc_test init_effect={root={set_variable={which=angle value=0}}} orbit_angle_offset=angle}",
        Context::new("galactic_object"),
        initial,
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn spawn_planet_callback_preserves_this_root_from_prev() {
    let mut ctx = Context::new("country").push(Types::new("galactic_object"), "solar_system");
    ctx.froms = vec![Types::new("country")];
    ctx.from_objects = vec!["from-country".into()];
    let (e, _) = spawn_case(
        "spawn_planet={class=pc_test init_effect={set_planet_flag=ok root={set_country_flag=ok} from={set_country_flag=ok} prev={set_star_flag=ok}}}",
        ctx,
        State::default(),
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    let (e, _) = spawn_case(
        "spawn_planet={class=pc_test init_effect={set_country_flag=bad}}",
        Context::new("galactic_object"),
        State::default(),
    );
    let d = e
        .diagnostics
        .rows
        .iter()
        .find(|d| d.key == "scope")
        .unwrap();
    assert!(
        d.call_chain
            .iter()
            .any(|f| f["kind"] == "spawn-planet-callback")
    );
}

#[test]
fn spawn_planet_creation_does_not_loan_guaranteed_target_or_number() {
    let mut initial = State::default();
    initial.set_number(
        State::number_key("root", "x"),
        crate::domain::NumberFact::assigned(Some(2.0)),
    );
    let (e, state) = spawn_case(
        "spawn_planet={class=pc_test init_effect={save_event_target_as=created root={set_variable={which=x value=0}}}}",
        Context::new("galactic_object"),
        initial,
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert_eq!(state.get("event_target:created").types.label(), "planet");
    assert_eq!(
        state.get("event_target:created").availability,
        Availability::Maybe
    );
    assert_eq!(state.number(&State::number_key("root", "x")).value(), None);
}

#[test]
fn spawn_planet_fresh_callback_does_not_borrow_old_receiver_number() {
    let mut e = make();
    e.program
        .entities
        .entry("planet_classes".into())
        .or_default()
        .insert("pc_test".into());
    let n = ast(&mut e, "spawn_planet={class=pc_test init_effect={set_variable={which=n value=1} divide_variable={which=n value=x}}}").remove(0);
    let ctx = Context::new("galactic_object");
    let frame = ctx.push(Types::new("planet"), &format!("spawned-planet:{}", n.id));
    let mut initial = State::default();
    initial.set_number(
        State::number_key(&frame.object, "x"),
        crate::domain::NumberFact::assigned(Some(0.0)),
    );
    let mut state = initial.clone();
    e.spawn_planet_parameters(&n, &ctx, &mut state, &initial);
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "numeric-binding-unproven"));
}

#[test]
fn spawn_planet_native_flags_and_none_lists_are_valid() {
    let (e, _) = spawn_case(
        "spawn_planet={class=pc_test orbit_angle=-30 orbit_distance=45 deposit_blockers=none modifiers=none flags={ignore_startup_effect megastructure}}",
        Context::new("galactic_object"),
        State::default(),
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    let (e, _) = spawn_case(
        "spawn_planet={class=pc_test flags={bad=yes}}",
        Context::new("galactic_object"),
        State::default(),
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "structure"));
}

#[test]
fn spawn_planet_unverified_location_and_subfields_remain_unknown() {
    let (e, _) = spawn_case(
        "spawn_planet={class=pc_test location=root plane_offset={value=1} modifiers={unknown_modifier=yes}}",
        Context::new("galactic_object"),
        State::default(),
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "spawn-planet-location-unverified"));
    assert!(has(&e, "spawn-planet-field-unverified"));
}

#[test]
fn spawn_planet_unreachable_body_and_literal_reference_are_checked() {
    let mut e = make();
    let n = ast(
        &mut e,
        "spawn_planet={class=missing_class init_effect={unknown_spawn_effect=yes}}",
    )
    .remove(0);
    e.spawn_planet_structure(&n);
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "unknown-effect"));
}

#[test]
fn spawn_planet_plural_callback_preserves_legacy_with_current_boundary() {
    let (e, _) = spawn_case(
        "spawn_planet={class=pc_test init_effects={set_planet_flag=ok}}",
        Context::new("galactic_object"),
        State::default(),
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "spawn-planet-callback-form-unverified"));
}
