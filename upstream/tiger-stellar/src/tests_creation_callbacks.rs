use super::*;

fn parent_fields_case(text: &str, scope: &str, inputs: State) -> Engine {
    let mut e = make();
    for category in ["situations", "megastructures"] {
        e.program
            .entities
            .entry(category.into())
            .or_default()
            .insert("known".into());
    }
    let parent = ast(&mut e, text).remove(0);
    e.creation_callback_structure(&parent);
    let mut state = inputs.clone();
    state.forget_numbers();
    for child in parent.body() {
        e.creation_parent_field(&parent, child, &Context::new(scope), &mut state, &inputs);
    }
    e
}

#[test]
fn creation_parents_require_block_assignment_and_evidenced_required_fields() {
    for (text, expected) in [
        ("start_situation=yes", "structure"),
        ("spawn_megastructure=yes", "structure"),
        ("auto_move_to_planet=yes", "structure"),
        ("start_situation>={type=known}", "operator"),
        ("start_situation={}", "missing-field"),
        ("spawn_megastructure={}", "missing-field"),
        ("auto_move_to_planet={}", "missing-field"),
    ] {
        let e = parent_fields_case(text, "country", State::default());
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == expected),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn creation_parent_reference_and_target_contracts_are_checked() {
    for (text, scope, expected) in [
        ("start_situation={type=missing}", "country", "missing-item"),
        (
            "spawn_megastructure={type=missing}",
            "galactic_object",
            "missing-item",
        ),
        ("auto_move_to_planet={target=this}", "fleet", "scope"),
        (
            "spawn_megastructure={type=known owner=this}",
            "galactic_object",
            "scope",
        ),
        (
            "spawn_megastructure={type=known planet=this}",
            "galactic_object",
            "scope",
        ),
        ("start_situation={type={bad=known}}", "country", "structure"),
        (
            "auto_move_to_planet={target={bad=yes}}",
            "fleet",
            "structure",
        ),
    ] {
        let e = parent_fields_case(text, scope, State::default());
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == expected),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn creation_parent_boolean_numeric_and_field_operators_are_checked() {
    for (text, expected) in [
        (
            "auto_move_to_planet={target=this clear_auto_move_on_arrival=maybe}",
            "value",
        ),
        ("spawn_megastructure={type=known orbit_angle=NaN}", "value"),
        (
            "spawn_megastructure={type=known orbit_distance=inf}",
            "value",
        ),
        (
            "spawn_megastructure={type=known orbit_angle>=2}",
            "operator",
        ),
    ] {
        let e = parent_fields_case(text, "planet", State::default());
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == expected),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn creation_parent_numeric_reads_use_pre_effect_inputs_and_allow_large_values() {
    let mut inputs = State::default();
    inputs.set_number(
        State::number_key("root", "angle"),
        crate::domain::NumberFact::assigned(Some(2.0)),
    );
    let e = parent_fields_case(
        "spawn_megastructure={type=known coords_from=this name=NAME graphical_culture=generic_01 orbit_angle=angle orbit_distance=1000000000}",
        "galactic_object",
        inputs,
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    let e = parent_fields_case(
        "auto_move_to_planet={target=this clear_auto_move_on_arrival=yes}",
        "planet",
        State::default(),
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    let e = parent_fields_case(
        "start_situation={type=known target=this}",
        "country",
        State::default(),
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn creation_parent_unknown_fields_are_not_silently_consumed() {
    let mut e = make();
    let parent = ast(&mut e, "start_situation={future_engine_parameter=yes}").remove(0);
    assert!(!e.creation_parent_field(
        &parent,
        &parent.body()[0],
        &Context::new("country"),
        &mut State::default(),
        &State::default(),
    ));
}

#[test]
fn unreachable_creation_parent_literal_type_reference_is_still_checked() {
    let mut e = make();
    let body = ast(
        &mut e,
        "if={limit={always=no} start_situation={type=missing_situation} spawn_megastructure={type=missing_megastructure}}",
    );
    e.effects(&body, &Context::new("country"), &mut State::default());
    for name in ["missing_situation", "missing_megastructure"] {
        assert!(
            e.diagnostics
                .rows
                .iter()
                .any(|d| d.key == "missing-item" && d.message.contains(name)),
            "{:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn unreachable_creation_callbacks_still_check_effect_grammar() {
    let mut e = make();
    let body = ast(
        &mut e,
        "if={limit={always=no} start_situation={type=unused effect={unknown_creation_effect=yes}} auto_move_to_planet={arrival_effect={unknown_arrival_effect=yes}}}",
    );
    e.effects(&body, &Context::new("country"), &mut State::default());
    for name in ["unknown_creation_effect", "unknown_arrival_effect"] {
        assert!(
            e.diagnostics
                .rows
                .iter()
                .any(|d| d.key == "unknown-effect" && d.message.contains(name)),
            "{:?}",
            e.diagnostics.rows,
        );
    }
}

#[test]
fn callback_structure_rejects_scalar_creation_and_missing_arrival_helper() {
    let mut e = make();
    for text in [
        "start_situation={effect=yes}",
        "spawn_megastructure={init_effect=helper}",
        "auto_move_to_planet={arrival_effect=missing_helper}",
        "create_random_fleet={effect>={set_fleet_flag=ok}}",
    ] {
        let parent = ast(&mut e, text).remove(0);
        e.creation_callback_structure(&parent);
    }
    for key in ["structure", "missing-item", "operator"] {
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{:?}",
            e.diagnostics.rows
        );
    }
}

fn callback_case(text: &str, context: &Context, initial: State, helpers: &str) -> (Engine, State) {
    let mut e = make();
    for def in ast(&mut e, helpers) {
        e.program
            .scripts
            .insert((true, def.key.clone()), Arc::new(def));
    }
    let parent = ast(&mut e, text).remove(0);
    let child = parent
        .body()
        .iter()
        .find(|n| matches!(n.key.as_str(), "effect" | "init_effect" | "arrival_effect"))
        .unwrap()
        .clone();
    let mut state = initial;
    assert!(e.creation_callback(&parent, &child, context, &mut state));
    (e, state)
}

#[test]
fn creation_callback_targets_keep_created_type_without_proving_execution() {
    for (text, input, output) in [
        (
            "start_situation={effect={save_event_target_as=t}}",
            "country",
            "situation",
        ),
        (
            "spawn_megastructure={init_effect={save_event_target_as=t}}",
            "galactic_object",
            "megastructure",
        ),
        (
            "create_random_fleet={effect={save_event_target_as=t}}",
            "country",
            "fleet",
        ),
    ] {
        let (e, state) = callback_case(text, &Context::new(input), State::default(), "");
        assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
        assert_eq!(state.get("event_target:t").types.label(), output);
        assert_eq!(
            state.get("event_target:t").availability,
            Availability::Maybe
        );
    }
}

#[test]
fn creation_callback_root_from_prev_are_distinct_from_created_this() {
    let mut ctx = Context::new("country");
    ctx.froms = vec![Types::new("planet")];
    ctx.from_objects = vec!["source-planet".into()];
    let (e, _) = callback_case(
        "start_situation={effect={root={set_country_flag=ok} prev={set_country_flag=ok} from={set_planet_flag=ok} save_event_target_as=t}}",
        &ctx,
        State::default(),
        "",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    let (e, _) = callback_case(
        "start_situation={effect={set_country_flag=bad}}",
        &ctx,
        State::default(),
        "",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "scope"));
    assert!(e.diagnostics.rows.iter().any(|d| {
        d.call_chain
            .iter()
            .any(|f| f["kind"] == "creation-callback")
    }));
}

#[test]
fn creation_callback_shape_and_operator_are_checked() {
    for text in [
        "start_situation={effect=yes}",
        "spawn_megastructure={init_effect=helper}",
        "create_random_fleet={effect=no}",
    ] {
        let (e, _) = callback_case(text, &Context::new("country"), State::default(), "");
        assert!(e.diagnostics.rows.iter().any(|d| d.key == "structure"));
    }
    let (e, _) = callback_case(
        "start_situation={effect>={save_event_target_as=t}}",
        &Context::new("country"),
        State::default(),
        "",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "operator"));
}

#[test]
fn creation_callback_does_not_loan_root_number_or_guarded_target() {
    let mut initial = State::default();
    initial.set_number(
        State::number_key("root", "x"),
        crate::domain::NumberFact::assigned(Some(2.0)),
    );
    let (e, state) = callback_case(
        "start_situation={effect={root={set_variable={which=x value=0}} if={limit={is_scope_type=situation} save_event_target_as=t}}}",
        &Context::new("country"),
        initial,
        "",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert_eq!(state.number(&State::number_key("root", "x")).value(), None);
    assert_eq!(
        state.get("event_target:t").availability,
        Availability::Maybe
    );
    assert!(state.guarded.is_empty());
}

#[test]
fn creation_callback_fresh_receiver_does_not_borrow_old_number() {
    let mut e = make();
    let parent = ast(&mut e, "create_random_fleet={effect={set_variable={which=n value=1} divide_variable={which=n value=x}}}").remove(0);
    let child = parent.field("effect").unwrap();
    let ctx = Context::new("country");
    let frame = ctx.push(
        Types::new("fleet"),
        &format!("created-callback:{}:{}", parent.id, child.key),
    );
    let mut state = State::default();
    state.set_number(
        State::number_key(&frame.object, "x"),
        crate::domain::NumberFact::assigned(Some(0.0)),
    );
    e.creation_callback(&parent, child, &ctx, &mut state);
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "numeric-binding-unproven"));
}

#[test]
fn arrival_scalar_helper_uses_fleet_this_and_retains_call_chain() {
    let (e, _) = callback_case(
        "auto_move_to_planet={arrival_effect=helper}",
        &Context::new("fleet"),
        State::default(),
        "helper={set_fleet_flag=ok}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "arrival-callback-context-unverified"));
    assert!(e.invoked.contains(&(true, "helper".into())));
    let (e, _) = callback_case(
        "auto_move_to_planet={arrival_effect=helper}",
        &Context::new("fleet"),
        State::default(),
        "helper={set_country_flag=bad}",
    );
    let error = e
        .diagnostics
        .rows
        .iter()
        .find(|d| d.key == "scope")
        .unwrap();
    assert!(
        error
            .call_chain
            .iter()
            .any(|f| f["kind"] == "arrival-callback")
    );
    assert!(
        error
            .call_chain
            .iter()
            .any(|f| f["kind"] == "scripted-call")
    );
}

#[test]
fn arrival_missing_helper_and_operator_are_errors() {
    let (e, _) = callback_case(
        "auto_move_to_planet={arrival_effect=missing_helper}",
        &Context::new("fleet"),
        State::default(),
        "",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
    let (e, _) = callback_case(
        "auto_move_to_planet={arrival_effect>=helper}",
        &Context::new("fleet"),
        State::default(),
        "helper={set_fleet_flag=ok}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "operator"));
}

#[test]
fn arrival_future_has_no_known_root_from_prev_or_old_numeric_value() {
    let mut ctx = Context::new("fleet");
    ctx.root = Types::new("country");
    ctx.froms = vec![Types::new("country")];
    let mut initial = State::default();
    initial.set_number(
        State::number_key("root", "x"),
        crate::domain::NumberFact::assigned(Some(0.0)),
    );
    initial.set(
        "event_target:old".into(),
        Binding::assigned(Types::new("planet")),
    );
    let (e, _) = callback_case(
        "auto_move_to_planet={arrival_effect=helper}",
        &ctx,
        initial,
        "helper={root={set_country_flag=unknown} from={set_country_flag=unknown} prev={set_country_flag=unknown} event_target:old={set_country_flag=unknown} set_variable={which=n value=1} divide_variable={which=n value=x}}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "numeric-binding-unproven"));
    assert!(has(&e, "target-binding-unproven"));
    assert!(has(&e, "caller-context-unknown"));
}

#[test]
fn arrival_callback_does_not_modify_registration_state() {
    let mut initial = State::default();
    initial.set_number(
        State::number_key("root", "x"),
        crate::domain::NumberFact::assigned(Some(2.0)),
    );
    let before = initial.clone();
    let (e, state) = callback_case(
        "auto_move_to_planet={arrival_effect=helper}",
        &Context::new("fleet"),
        initial,
        "helper={save_event_target_as=future set_variable={which=x value=0}}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert_eq!(state, before);
    assert_eq!(
        state.get("event_target:future").availability,
        Availability::Absent
    );
}

#[test]
fn arrival_ship_and_block_forms_do_not_guess_callback_contracts() {
    let (e, _) = callback_case(
        "auto_move_to_planet={arrival_effect=helper}",
        &Context::new("ship"),
        State::default(),
        "helper={set_fleet_flag=unknown}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "arrival-callback-input-unverified"));
    let (e, _) = callback_case(
        "auto_move_to_planet={arrival_effect={set_country_flag=unknown}}",
        &Context::new("fleet"),
        State::default(),
        "",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "arrival-callback-form-unverified"));
}
