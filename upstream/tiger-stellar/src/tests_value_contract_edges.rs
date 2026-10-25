use super::*;
#[test]
fn value_pop_query_limit_checks_pop_category_reference() {
    let (e, _) = value_case(
        "v={complex_trigger_modifier={trigger=count_owned_pop_amount parameters={limit={is_pop_category=missing_category}} mode=add}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
}

#[test]
fn value_modifier_container_requires_assignment_operator() {
    let (e, _) = value_case(
        "v={base=1 modifier>={add=1}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "operator"));
}
#[test]
fn value_complex_container_and_fields_require_assignment_operators() {
    for value in [
        "v={complex_trigger_modifier!={trigger=check_galaxy_setup_value parameters={setting=fallen_empire_strength_scale} mode=add}}",
        "v={complex_trigger_modifier={trigger>=check_galaxy_setup_value parameters={setting=fallen_empire_strength_scale} mode=add}}",
        "v={complex_trigger_modifier={trigger=check_galaxy_setup_value parameters>{setting=fallen_empire_strength_scale} mode=add}}",
    ] {
        let (e, _) = value_case(value, "set_variable={which=x value=value:v}");
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "operator"),
            "{value}: {:?}",
            e.diagnostics.rows
        );
    }
}
#[test]
fn value_fixed_missing_reference_is_checked_before_dynamic_arguments() {
    let (e, _) = value_case("", "set_variable={which=x value=value:missing|N|@reader|}");
    assert!(
        e.diagnostics.rows.iter().any(|d| d.key == "missing-item"),
        "{:?}",
        e.diagnostics.rows
    );
}
#[test]
fn value_known_reference_with_dynamic_arguments_retains_boundary() {
    let (e, _) = value_case(
        "v={base=$N$}",
        "set_variable={which=x value=value:v|N|@reader|}",
    );
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "numeric-information-unresolved"));
}
#[test]
fn value_modifier_queries_are_runtime_numbers_without_variable_binding_loans() {
    let (e, _) = value_case(
        "v={base=modifier:refugee_attraction add=owner.modifier:leader_initial_traits_add}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}
#[test]
fn value_unknown_modifier_is_a_reference_error() {
    let (e, _) = value_case(
        "v={base=modifier:missing_stellar_modifier}",
        "set_variable={which=x value=value:v}",
    );
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "unknown-modifier")
    );
}
#[test]
fn value_modifier_unknown_receiver_is_not_guessed() {
    let (e, _) = value_case(
        "v={base=from.modifier:refugee_attraction}",
        "set_variable={which=x value=value:v}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "numeric-modifier-context-unverified"));
}
#[test]
fn value_alias_assignment_invalidates_previous_absence_in_both_directions() {
    let (e, s) = value_case(
        "v={base=x}",
        "clear_variable=x save_event_target_as=t event_target:t={set_variable={which=x value=2}} set_variable={which=y value=value:v}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert_eq!(
        s.number(&State::number_key("root", "x")).availability,
        Availability::Maybe
    );
    assert!(has(&e, "numeric-binding-unproven"));
}
#[test]
fn value_alias_assignment_does_not_prune_a_reachable_bad_branch() {
    let (e, _) = value_case(
        "v={base=2 subtract=2}",
        "clear_variable=x save_event_target_as=t event_target:t={set_variable={which=x value=2}} if={limit={is_variable_set=x} set_variable={which=n value=1} divide_variable={which=n value=value:v}}",
    );
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "value" && d.message.contains("zero")),
        "{:?}",
        e.diagnostics.rows
    );
}
#[test]
fn value_abs_preserves_proven_runtime_nonzero_without_inventing_constant() {
    let (e, _) = value_case(
        "v={base=x abs=yes}",
        "if={limit={is_variable_set=x check_variable={which=x value>0}} set_variable={which=n value=1} divide_variable={which=n value=value:v}}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}
#[test]
fn value_negative_power_of_zero_is_not_silently_accepted() {
    let (e, _) = value_case("v={base=0 pow=-1}", "set_variable={which=x value=value:v}");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "value"));
}
#[test]
fn value_small_integer_power_detects_derived_zero() {
    let (e, _) = value_case(
        "v={base=2 pow=3 subtract=8}",
        "set_variable={which=x value=1} divide_variable={which=x value=value:v}",
    );
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "value" && d.message.contains("zero"))
    );
}
#[test]
fn value_small_reciprocal_and_large_unproven_power_are_distinct() {
    let (e, _) = value_case("v={base=2 pow=-1}", "set_variable={which=x value=value:v}");
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    let (e, _) = value_case(
        "v={base=1e30 pow=32}",
        "set_variable={which=x value=value:v}",
    );
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "numeric-power-unproven"));
}
#[test]
fn value_native_min_value_field_retains_order_boundary() {
    let (e, _) = value_case(
        "v={complex_trigger_modifier={trigger=check_galaxy_setup_value parameters={setting=fallen_empire_strength_scale} mode=divide min_value=1}}",
        "set_variable={which=x value=value:v}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "numeric-clamp-contract-unverified"));
    assert!(has(&e, "numeric-divisor-unproven"));
}
