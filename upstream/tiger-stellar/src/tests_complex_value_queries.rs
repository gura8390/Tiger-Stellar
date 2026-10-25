use super::*;

fn query_case(definitions: &str, effects: &str) -> (Engine, State) {
    let mut e = make();
    for (category, names) in [
        (
            "buildings",
            vec!["building_capital", "building_identity_repository"],
        ),
        ("districts", vec!["district_farming"]),
        ("strategic_resources", vec!["unity"]),
        (
            "pop_categories",
            vec!["cruise_passenger", "cruise_passenger_unemployment"],
        ),
    ] {
        e.program.entities.insert(
            category.into(),
            names.into_iter().map(String::from).collect(),
        );
    }
    let nodes = ast(&mut e, definitions);
    e.program.index(File {
        path: "fixture.txt".into(),
        category: "script_values".into(),
        ast: nodes.into_iter().map(Arc::new).collect(),
        target: false,
    });
    let nodes = ast(&mut e, effects);
    let mut state = State::default();
    e.effects(&nodes, &Context::new("country"), &mut state);
    (e, state)
}

#[test]
fn complex_query_native_active_buildings_macro_is_valid() {
    // common/script_values/00_script_values.txt:572-581, unchanged definition.
    let (e, _) = query_case(
        "num_active_buildings={complex_trigger_modifier={trigger=num_buildings parameters={type=$BUILDING$ disabled=no} mode=add}}",
        "set_variable={which=x value=value:num_active_buildings|BUILDING|building_capital|}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn complex_query_native_resource_income_is_numeric() {
    // common/script_values/00_script_values.txt:559-570.
    let (e, s) = query_case(
        "sacrifice_cost={base=0 complex_trigger_modifier={trigger=resource_income_compare parameters={resource=unity} mode=add} abs=yes mult=3}",
        "set_variable={which=x value=value:sacrifice_cost}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    assert_eq!(s.number(&State::number_key("root", "x")).value(), None);
}

#[test]
fn complex_query_native_passenger_filter_runs_in_pop_group() {
    // common/script_values/08_script_values_nomads.txt:95-109.
    let (e, _) = query_case(
        "passenger_satisfaction_passenger_contract={complex_trigger_modifier={trigger=count_owned_pop_amount parameters={limit={OR={is_pop_category=cruise_passenger is_pop_category=cruise_passenger_unemployment}}} mode=add} divide=100}",
        "home_planet={set_variable={which=x value=value:passenger_satisfaction_passenger_contract}}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn complex_query_missing_building_is_an_error() {
    let (e, _) = query_case(
        "v={complex_trigger_modifier={trigger=num_buildings parameters={type=building_missing} mode=add}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
}

#[test]
fn complex_query_missing_district_is_an_error() {
    let (e, _) = query_case(
        "v={complex_trigger_modifier={trigger=num_districts parameters={type=district_missing} mode=add}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
}

#[test]
fn complex_query_missing_resource_is_an_error() {
    let (e, _) = query_case(
        "v={complex_trigger_modifier={trigger=resource_income_compare parameters={resource=missing} mode=add}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
}

#[test]
fn complex_query_resource_requires_a_resource_parameter() {
    let (e, _) = query_case(
        "v={complex_trigger_modifier={trigger=resource_income_compare mode=add}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-field"));
}

#[test]
fn complex_query_galaxy_setting_is_required() {
    let (e, _) = query_case(
        "v={complex_trigger_modifier={trigger=check_galaxy_setup_value parameters={} mode=add}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-field"));
}

#[test]
fn complex_query_building_enums_are_closed() {
    for parameter in [
        "disabled=maybe",
        "in_construction=perhaps",
        "owner_type=alien",
    ] {
        let (e, _) = query_case(
            &format!(
                "v={{complex_trigger_modifier={{trigger=num_buildings parameters={{type=any {parameter}}} mode=add}}}}"
            ),
            "set_variable={which=x value=value:v}",
        );
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "value"),
            "{parameter}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn complex_query_unknown_parameter_is_an_error() {
    let (e, _) = query_case(
        "v={complex_trigger_modifier={trigger=num_buildings parameters={type=any disabeld=no} mode=add}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "unknown-field"));
}

#[test]
fn complex_query_parameter_operator_is_not_a_comparison() {
    let (e, _) = query_case(
        "v={complex_trigger_modifier={trigger=num_buildings parameters={type>building_capital} mode=add}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "operator"));
}

#[test]
fn complex_query_scalar_parameters_are_rejected() {
    let (e, _) = query_case(
        "v={complex_trigger_modifier={trigger=num_districts parameters=1 mode=add}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "structure"));
}

#[test]
fn complex_query_limit_preserves_root_and_prev_after_trigger_scope() {
    let (e, _) = query_case(
        "v={complex_trigger_modifier={trigger=count_owned_pop_amount trigger_scope=home_planet parameters={limit={is_pop_category=cruise_passenger root={is_ai=yes} prev={is_star=no}}} mode=add}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn complex_query_pop_group_limit_rejects_country_predicate() {
    let (e, _) = query_case(
        "v={complex_trigger_modifier={trigger=count_owned_pop_amount parameters={limit={is_ai=yes}} mode=add}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "scope"));
}

#[test]
fn complex_query_limit_root_does_not_become_the_pop_group() {
    let (e, _) = query_case(
        "v={complex_trigger_modifier={trigger=count_owned_pop_amount trigger_scope=home_planet parameters={limit={root={is_star=yes}}} mode=add}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "scope"));
}

#[test]
fn complex_query_limit_prev_is_the_query_receiver() {
    let (e, _) = query_case(
        "v={complex_trigger_modifier={trigger=count_owned_pop_amount trigger_scope=home_planet parameters={limit={prev={is_ai=yes}}} mode=add}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "scope"));
}

#[test]
fn complex_query_building_limit_is_colony_not_country_or_planet() {
    let (e, _) = query_case(
        "v={complex_trigger_modifier={trigger=num_buildings parameters={type=any limit={is_star=yes}} mode=add}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(
        e.diagnostics.rows.iter().any(|d| d.key == "scope"
            && d.context
                .as_ref()
                .is_some_and(|c| c.this.label() == "colony")),
        "{:?}",
        e.diagnostics.rows
    );
}

#[test]
fn complex_query_building_limit_keeps_country_prev_and_root() {
    let (e, _) = query_case(
        "v={complex_trigger_modifier={trigger=num_buildings parameters={type=any limit={has_building=building_capital prev={is_ai=yes} root={is_ai=yes}}} mode=add}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn complex_query_district_requires_evidence_for_an_omitted_type_default() {
    for parameters in ["", "parameters={}"] {
        let (e, _) = query_case(
            &format!(
                "v={{complex_trigger_modifier={{trigger=num_districts {parameters} mode=add}}}}"
            ),
            "set_variable={which=x value=value:v}",
        );
        assert_eq!(errors(&e), 0);
        assert!(has(&e, "numeric-trigger-contract-unverified"));
    }
    // 08_script_values_nomads.txt:856-858 explicitly supplies type=any.
    let (e, _) = query_case(
        "v={complex_trigger_modifier={trigger=num_districts parameters={type=any} mode=add}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn complex_query_duplicate_parameters_are_rejected() {
    let (e, _) = query_case(
        "v={complex_trigger_modifier={trigger=num_buildings parameters={type=any type=building_capital} mode=add}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "duplicate-field")
    );
}

#[test]
fn complex_query_filter_cannot_export_numeric_guard_facts() {
    let mut e = make();
    let nodes = ast(
        &mut e,
        "complex_trigger_modifier={trigger=count_owned_pop_amount parameters={limit={root={is_variable_set=unproven check_variable={which=unproven value>0}}}} mode=add}",
    );
    let state = State::default();
    let before = state.clone();
    assert!(e.complex_value_query(&nodes[0], &Context::new("country"), &state));
    assert_eq!(state, before);
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn complex_query_building_category_remains_unknown() {
    let (e, _) = query_case(
        "v={complex_trigger_modifier={trigger=num_buildings parameters={type=any category=unity} mode=add}}",
        "set_variable={which=x value=value:v}",
    );
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "numeric-trigger-contract-unverified"));
}

#[test]
fn complex_query_comparison_parameter_remains_unknown() {
    let (e, _) = query_case(
        "v={complex_trigger_modifier={trigger=count_owned_pop_amount parameters={count>1 limit={always=yes}} mode=add}}",
        "set_variable={which=x value=value:v}",
    );
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "numeric-trigger-contract-unverified"));
}

#[test]
fn complex_query_count_planet_export_conflict_remains_unknown() {
    let (e, _) = query_case(
        "v={complex_trigger_modifier={trigger=count_owned_planet parameters={limit={always=yes}} mode=add}}",
        "set_variable={which=x value=value:v}",
    );
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "numeric-trigger-contract-unverified"));
}

#[test]
fn complex_query_unimplemented_numeric_result_is_not_guessed() {
    let (e, _) = query_case(
        "v={complex_trigger_modifier={trigger=has_building parameters={type=building_capital} mode=add}}",
        "home_planet={set_variable={which=x value=value:v}}",
    );
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "numeric-trigger-contract-unverified"));
}

#[test]
fn complex_query_valid_enums_do_not_reject_high_runtime_values() {
    let (e, _) = query_case(
        "v={base=1000000 complex_trigger_modifier={trigger=num_buildings parameters={type=any disabled=any in_construction=yes owner_type=subject_holding category=any} mode=add mult=1000000}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}
