use super::*;

// Stellaris Cygnus 4.5.2 local primary sources, read on 2026-10-24:
// script_documentation/effects.log:175-178(code flags),606-612(war name),2798-2802(Vivarium).
// SHA256 bd7bfa135c89a01a423833acab77fd78e19a5311bf4532d6f4a55584b0d4747b
// events/fallen_empire_awakening_events.txt:111-113, colonizer=yes.
// SHA256 bbe2df476005a3e01ced6cd254acdc30acec309beca6477cda7a739d30120721
// common/scripted_effects/grand_archive_effects.txt:1697-1706, five owners.
// SHA256 0369d4f864e409ab4136274673fee86ff69b36ecf1ef26af3dc9513bd70c3a1a
// common/ship_categories/ship_categories.txt:16,23,31,40,47, matching category keys.
// SHA256 30609069c027f9d40e3ce631561f1cab19c1879af82838866abeb949ebeeff23
// events/planet_destruction_events.txt:1847-1855, localized name+variable_string.
// SHA256 b0e7521d7ef7cc925e033c12c7fdb8e6d1c09aa624be1d45c8cad998e3c13e87
// These fixtures check static field contracts, not actual fauna release, war
// validity, text rendering, or a runtime engine callback/evaluation frame.
fn effect_map_case(text: &str, scope: &str) -> Engine {
    let mut e = make();
    e.program.entities.insert(
        "ship_categories".into(),
        [
            "space_amoeba",
            "tiyanki",
            "voidworm",
            "cutholoid",
            "crystalline_entity",
            "custom_fauna",
            "military",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
    );
    e.program.entities.insert(
        "war_goals".into(),
        ["wg_subjugation"].into_iter().map(String::from).collect(),
    );
    e.program
        .loc
        .entry("english".into())
        .or_default()
        .insert("NAME_Holy_World_War".into());
    let nodes = ast(&mut e, text);
    let mut state = State::default();
    for name in [
        "amoeba_country",
        "tiyanki_country",
        "voidworms_country",
        "cutholoids_country",
        "crystal_country",
        "country",
    ] {
        state.set(
            format!("event_target:{name}"),
            Binding::assigned(Types::new("country")),
        );
    }
    state.set(
        "event_target:planet".into(),
        Binding::assigned(Types::new("planet")),
    );
    state.set(
        "event_target:system".into(),
        Binding::assigned(Types::new("galactic_object")),
    );
    e.effects(&nodes, &Context::new(scope), &mut state);
    e
}

#[test]
fn code_flags_native_colonizer_and_all_exported_boolean_flags_are_valid() {
    for flag in [
        "colonizer",
        "rebel",
        "track_all_situations",
        "custom_name",
        "is_in_breach_of_any",
        "has_advisor",
        "autopick_leader_traits",
        "crisis_fighter",
        "is_under_crisis_attack",
        "initialized",
        "is_affected_by_war_exhaustion",
        "is_nomadic",
        "killed",
    ] {
        for value in ["yes", "no"] {
            let e = effect_map_case(
                &format!("set_country_code_flags={{{flag}={value}}}"),
                "country",
            );
            assert!(
                e.diagnostics.rows.is_empty(),
                "{flag}={value}: {:?}",
                e.diagnostics.rows
            );
        }
    }
}

#[test]
fn code_flags_multiple_assignments_keep_the_exported_map_contract() {
    let e = effect_map_case(
        "set_country_code_flags={colonizer=yes rebel=no initialized=yes}",
        "country",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn code_flags_typos_invalid_booleans_and_wrong_shapes_are_errors() {
    for (text, key) in [
        ("set_country_code_flags={coloniser=yes}", "unknown-field"),
        ("set_country_code_flags={colonizer=2}", "value"),
        ("set_country_code_flags={colonizer=maybe}", "value"),
        ("set_country_code_flags={colonizer={yes}}", "structure"),
        ("set_country_code_flags=yes", "structure"),
        ("set_country_code_flags={colonizer}", "structure"),
    ] {
        let e = effect_map_case(text, "country");
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn code_flags_parent_and_child_assignment_operators_are_checked() {
    for text in [
        "set_country_code_flags>={colonizer=yes}",
        "set_country_code_flags={colonizer!=yes}",
    ] {
        let e = effect_map_case(text, "country");
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "operator"),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn code_flags_dynamic_identity_and_boolean_keep_unknown_boundaries() {
    for (text, reason) in [
        ("set_country_code_flags={$FLAG$=yes}", "dynamic-reference"),
        (
            "set_country_code_flags={colonizer=$VALUE$}",
            "code-flag-value-unverified",
        ),
    ] {
        let e = effect_map_case(text, "country");
        assert_eq!(errors(&e), 0, "{text}: {:?}", e.diagnostics.rows);
        assert!(has(&e, reason));
    }
}

#[test]
fn code_flags_unknown_receiver_does_not_hide_static_typos() {
    let e = effect_map_case("set_country_code_flags={coloniser=yes}", "?");
    assert!(has(&e, "scope-type-unknown"));
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "unknown-field"));
    let e = effect_map_case("set_country_code_flags={colonizer=yes}", "planet");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "scope"));
}

#[test]
fn code_flags_unreachable_typos_and_bad_booleans_are_still_checked() {
    for (body, key) in [("coloniser=yes", "unknown-field"), ("colonizer=2", "value")] {
        let e = effect_map_case(
            &format!("if={{limit={{always=no}} set_country_code_flags={{{body}}}}}"),
            "country",
        );
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{body}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn code_flags_empty_and_repeated_maps_keep_unproved_composition_visible() {
    for (text, reason) in [
        (
            "set_country_code_flags={}",
            "code-flag-empty-map-unverified",
        ),
        (
            "set_country_code_flags={colonizer=yes colonizer=no}",
            "code-flag-composition-unverified",
        ),
    ] {
        let e = effect_map_case(text, "country");
        assert_eq!(errors(&e), 0, "{text}: {:?}", e.diagnostics.rows);
        assert!(has(&e, reason));
    }
}

#[test]
fn vivarium_native_five_owner_categories_and_country_targets_are_valid() {
    let e = effect_map_case(
        "release_vivarium_fauna_count={count=1 location=event_target:system owners={space_amoeba=event_target:amoeba_country tiyanki=event_target:tiyanki_country voidworm=event_target:voidworms_country cutholoid=event_target:cutholoids_country crystalline_entity=event_target:crystal_country}}",
        "country",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn vivarium_owner_category_typos_and_map_shapes_are_checked_statically() {
    for (body, key) in [
        ("owners={space_amobea=root}", "missing-item"),
        ("owners=yes", "structure"),
        ("owners={space_amoeba={owner=root}}", "structure"),
        ("owners={space_amoeba}", "structure"),
    ] {
        let e = effect_map_case(
            &format!("release_vivarium_fauna_count={{count=1 {body}}}"),
            "country",
        );
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{body}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn vivarium_owner_and_parent_assignment_operators_are_checked() {
    for text in [
        "release_vivarium_fauna_count>={count=1 owners={space_amoeba=root}}",
        "release_vivarium_fauna_count={owners>={space_amoeba=root}}",
        "release_vivarium_fauna_count={owners={space_amoeba!=root}}",
    ] {
        let e = effect_map_case(text, "country");
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "operator"),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn vivarium_owner_country_requirement_and_parent_input_scope_remain_active() {
    let e = effect_map_case(
        "release_vivarium_fauna_count={owners={space_amoeba=event_target:planet}}",
        "country",
    );
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "scope" && d.message.contains("Vivarium fauna owner")),
        "{:?}",
        e.diagnostics.rows
    );
    let e = effect_map_case(
        "release_vivarium_fauna_count={owners={space_amoeba=event_target:country}}",
        "planet",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "scope"));
}

#[test]
fn vivarium_unknown_owner_binding_and_receiver_keep_uncertainty() {
    let e = effect_map_case(
        "release_vivarium_fauna_count={owners={space_amoeba=event_target:unknown_country}}",
        "?",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "target-binding-unproven"));
    assert!(has(&e, "scope-type-unknown"));
}

#[test]
fn vivarium_unreachable_and_unknown_contexts_still_check_category_references() {
    for (text, scope) in [
        (
            "if={limit={always=no} release_vivarium_fauna_count={owners={space_amobea=root}}}",
            "country",
        ),
        (
            "release_vivarium_fauna_count={owners={space_amobea=root}}",
            "?",
        ),
    ] {
        let e = effect_map_case(text, scope);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "missing-item"),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn vivarium_dynamic_and_other_registered_categories_are_not_assumed_supported() {
    for (category, reason) in [
        ("$CATEGORY$", "dynamic-reference"),
        ("custom_fauna", "vivarium-owner-category-unverified"),
        ("military", "vivarium-owner-category-unverified"),
    ] {
        let e = effect_map_case(
            &format!("release_vivarium_fauna_count={{owners={{{category}=event_target:country}}}}"),
            "country",
        );
        assert_eq!(errors(&e), 0, "{category}: {:?}", e.diagnostics.rows);
        assert!(has(&e, reason));
    }
}

#[test]
fn vivarium_empty_and_repeated_owner_maps_keep_unproved_defaults_visible() {
    for (text, reason) in [
        (
            "release_vivarium_fauna_count={owners={}}",
            "vivarium-owner-default-unverified",
        ),
        (
            "release_vivarium_fauna_count={owners={space_amoeba=root space_amoeba=root}}",
            "vivarium-owner-composition-unverified",
        ),
    ] {
        let e = effect_map_case(text, "country");
        assert_eq!(errors(&e), 0, "{text}: {:?}", e.diagnostics.rows);
        assert!(has(&e, reason));
    }
}

#[test]
fn war_name_scalar_is_literal_and_does_not_require_localization() {
    for value in ["UnregisteredWarName", "\"A literal war name\""] {
        let e = effect_map_case(
            &format!(
                "declare_war={{target=event_target:country name={value} attacker_war_goal=wg_subjugation}}"
            ),
            "country",
        );
        assert!(
            e.diagnostics.rows.is_empty(),
            "{value}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn war_name_native_localized_form_preserves_runtime_substitution_boundary() {
    let e = effect_map_case(
        "declare_war={target=event_target:country name={key=NAME_Holy_World_War variable_string=\"[This.MainAttacker.GetAdj]\"} attacker_war_goal=wg_subjugation}",
        "country",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "war-name-substitution-context-unverified"));
    assert!(!has(&e, "scope-type-unknown"));
}

#[test]
fn war_name_fixed_key_and_literal_substitutions_are_structurally_valid() {
    let e = effect_map_case(
        "declare_war={target=event_target:country name={key=NAME_Holy_World_War variable_string=Attacker variable_string=Defender} attacker_war_goal=wg_subjugation}",
        "country",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn war_name_fixed_localization_key_keeps_its_warning_severity() {
    let e = effect_map_case(
        "declare_war={target=event_target:country name={key=NAME_Missing_War}}",
        "country",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "localization" && d.severity == "warning")
    );
}

#[test]
fn war_name_known_field_shapes_and_operators_are_checked() {
    for (body, key) in [
        ("name={key={NAME_Holy_World_War}}", "structure"),
        (
            "name={key=NAME_Holy_World_War variable_string={value=Attacker}}",
            "structure",
        ),
        ("name>={key=NAME_Holy_World_War}", "operator"),
        ("name={key!=NAME_Holy_World_War}", "operator"),
    ] {
        let e = effect_map_case(
            &format!("declare_war={{target=event_target:country {body}}}"),
            "country",
        );
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{body}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn war_name_unknown_extensions_forms_dynamic_keys_and_duplicates_stay_unknown() {
    for (body, reason) in [
        (
            "name={variable_string=Attacker}",
            "war-name-block-form-unverified",
        ),
        (
            "name={key=NAME_Holy_World_War future_field=yes}",
            "war-name-parameter-unverified",
        ),
        ("name={key=$NAME$}", "dynamic-reference"),
        (
            "name={key=NAME_Holy_World_War key=NAME_Holy_World_War}",
            "war-name-composition-unverified",
        ),
    ] {
        let e = effect_map_case(
            &format!("declare_war={{target=event_target:country {body}}}"),
            "country",
        );
        assert_eq!(errors(&e), 0, "{body}: {:?}", e.diagnostics.rows);
        assert!(has(&e, reason));
    }
}

#[test]
fn war_name_does_not_hide_parent_receiver_or_target_scope_errors() {
    for (text, scope) in [
        (
            "declare_war={target=event_target:country name=Literal}",
            "planet",
        ),
        (
            "declare_war={target=event_target:planet name=Literal}",
            "country",
        ),
    ] {
        let e = effect_map_case(text, scope);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "scope"),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn war_name_unreachable_and_unknown_receiver_paths_still_check_fixed_keys() {
    for (text, scope) in [
        (
            "if={limit={always=no} declare_war={target=event_target:country name={key=NAME_Missing_War}}}",
            "country",
        ),
        (
            "declare_war={target=event_target:country name={key=NAME_Missing_War}}",
            "?",
        ),
    ] {
        let e = effect_map_case(text, scope);
        assert!(
            e.diagnostics
                .rows
                .iter()
                .any(|d| d.key == "localization" && d.severity == "warning"),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn effect_map_partial_hooks_do_not_claim_unrelated_or_other_parent_fields() {
    let mut e = make();
    let n = ast(
        &mut e,
        "declare_war={target=root name=Literal attacker_war_goal=wg_test}",
    )
    .remove(0);
    assert!(!e.effect_map_structure(&n));
    assert!(!e.effect_map_parameters(&n));
    assert!(!e.effect_map_parameter_child(
        &n,
        n.field("attacker_war_goal").unwrap(),
        &Context::new("country"),
        &State::default()
    ));
    assert!(e.effect_map_parameter_child(
        &n,
        n.field("name").unwrap(),
        &Context::new("country"),
        &State::default()
    ));
    assert!(e.diagnostics.rows.is_empty());
}

#[test]
fn war_target_shape_and_operator_checks_survive_unreachable_branches() {
    for (text, key) in [
        (
            "declare_war={target={owner=root} name=Literal}",
            "structure",
        ),
        ("declare_war={target>=root name=Literal}", "operator"),
        (
            "if={limit={always=no} declare_war={target={owner=root}}}",
            "structure",
        ),
    ] {
        let e = effect_map_case(text, "country");
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn war_target_unexpanded_values_and_unavailable_bindings_remain_unknown() {
    for value in ["$TARGET$", "\"[Runtime.GetCountry]\"", "\"@reader\""] {
        let e = effect_map_case(
            &format!("declare_war={{target={value} name=Literal}}"),
            "country",
        );
        assert_eq!(errors(&e), 0, "{value}: {:?}", e.diagnostics.rows);
        assert!(has(&e, "war-target-value-unresolved"));
    }
    let e = effect_map_case(
        "declare_war={target=event_target:unknown_country name=Literal}",
        "country",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "target-binding-unproven"));
    assert!(has(&e, "scope-type-unknown"));
}

#[test]
fn war_target_unknown_caller_does_not_hide_a_known_wrong_target_type() {
    let e = effect_map_case("declare_war={target=event_target:planet name=Literal}", "?");
    assert!(has(&e, "scope-type-unknown"));
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "scope" && d.message.contains("declare_war target")),
        "{:?}",
        e.diagnostics.rows
    );
    for value in ["not_a_scope", "event_target:country.not_a_link"] {
        let e = effect_map_case(
            &format!("declare_war={{target={value} name=Literal}}"),
            "country",
        );
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "unknown-scope"),
            "{value}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn vivarium_unexpanded_owner_parameters_and_runtime_text_do_not_become_scope_typos() {
    for value in ["$OWNER$", "\"[Runtime.GetCountry]\"", "\"@reader\""] {
        let e = effect_map_case(
            &format!("release_vivarium_fauna_count={{owners={{space_amoeba={value}}}}}"),
            "country",
        );
        assert_eq!(errors(&e), 0, "{value}: {:?}", e.diagnostics.rows);
        assert!(has(&e, "vivarium-owner-value-unresolved"));
    }
}

#[test]
fn vivarium_fixed_country_targets_are_resolved_and_fixed_scope_typos_still_fail() {
    let e = effect_map_case(
        "release_vivarium_fauna_count={owners={space_amoeba=event_target:country}}",
        "country",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    for value in ["not_a_scope", "event_target:country.not_a_link"] {
        let e = effect_map_case(
            &format!("release_vivarium_fauna_count={{owners={{space_amoeba={value}}}}}"),
            "country",
        );
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "unknown-scope"),
            "{value}: {:?}",
            e.diagnostics.rows
        );
        assert!(!has(&e, "vivarium-owner-value-unresolved"));
    }
}

#[test]
fn vivarium_unreachable_owner_parameters_keep_the_static_unknown_boundary() {
    let e = effect_map_case(
        "if={limit={always=no} release_vivarium_fauna_count={owners={space_amoeba=$OWNER$}}}",
        "country",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "vivarium-owner-value-unresolved"));
}
