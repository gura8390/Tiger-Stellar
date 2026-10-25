use super::*;

fn random_fleet_value_case(definitions: &str, text: &str) -> Engine {
    let mut e = make();
    e.program.entities.insert(
        "global_ship_designs".into(),
        ["NAME_Alpha"].into_iter().map(String::from).collect(),
    );
    let values = ast(&mut e, definitions);
    e.program.index(File {
        path: "fixture-values.txt".into(),
        category: "script_values".into(),
        ast: values.into_iter().map(Arc::new).collect(),
        target: false,
    });
    let body = ast(&mut e, text);
    e.effects(&body, &Context::new("country"), &mut State::default());
    e
}

#[test]
fn random_fleet_derived_fractional_size_and_counts_are_rejected() {
    for body in [
        "create_random_fleet={size=value:fraction}",
        "create_random_fleet={size=2 ship_designs={{design=NAME_Alpha min=value:fraction}}}",
        "create_random_fleet={size=2 ship_designs={{design=NAME_Alpha max=value:fraction}}}",
        "set_variable={which=fraction value=1.5} create_random_fleet={size=fraction}",
        "set_variable={which=fraction value=1.5} create_random_fleet={size=2 ship_designs={{design=NAME_Alpha min=fraction}}}",
        "set_variable={which=fraction value=1.5} create_random_fleet={size=2 ship_designs={{design=NAME_Alpha max=fraction}}}",
    ] {
        let e = random_fleet_value_case("fraction={base=1.5}", body);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "value"
                && d.message == "fleet size and design counts require integers"),
            "{body}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn random_fleet_derived_integers_and_fractional_weights_are_valid() {
    for body in [
        "create_random_fleet={size=value:integer ship_designs={{design=NAME_Alpha min=value:integer max=value:integer weight=value:fraction}}}",
        "set_variable={which=count value=2} set_variable={which=weight value=1.5} create_random_fleet={size=count ship_designs={{design=NAME_Alpha min=count max=count weight=weight}}}",
    ] {
        let e = random_fleet_value_case("integer={base=1 add=1} fraction={base=1.5}", body);
        assert!(
            e.diagnostics.rows.is_empty(),
            "{body}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn random_fleet_runtime_integer_contract_remains_unproved_not_an_error() {
    let e = random_fleet_value_case(
        "quantity={base=trigger:is_difficulty}",
        "create_random_fleet={size=value:quantity ship_designs={{design=NAME_Alpha min=external_min max=external_max}}}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "numeric-integer-unproven"));
    assert!(has(&e, "numeric-range-unproven"));
}

fn random_fleet_case(text: &str) -> Engine {
    let mut e = make();
    e.program.entities.insert(
        "global_ship_designs".into(),
        ["NAME_Alpha", "NAME_Beta", "NAME_Gamma"]
            .into_iter()
            .map(String::from)
            .collect(),
    );
    let body = ast(&mut e, text);
    e.effects(&body, &Context::new("country"), &mut State::default());
    e
}
#[test]
fn random_fleet_native_design_entries_are_valid() {
    let e = random_fleet_case(
        "create_random_fleet={size=200 ship_designs={{design=NAME_Alpha weight=0.1 max=1} {design=NAME_Beta weight=1} {design=NAME_Gamma weight=4}}}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}
#[test]
fn random_fleet_bare_and_omitted_designs_are_valid() {
    for text in [
        "create_random_fleet={size=0}",
        "create_random_fleet={size=10 can_overflow=no ship_designs={NAME_Alpha NAME_Beta}}",
    ] {
        let e = random_fleet_case(text);
        assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    }
}
#[test]
fn random_fleet_missing_and_misspelled_designs_are_rejected() {
    for text in [
        "create_random_fleet={size=2 ship_designs={missing}}",
        "create_random_fleet={size=2 ship_designs={{weight=1}}}",
        "create_random_fleet={size=2 ship_designs={{design=NAME_Alpha wieght=1}}}",
    ] {
        let e = random_fleet_case(text);
        assert!(errors(&e) > 0, "{text}: {:?}", e.diagnostics.rows);
    }
}
#[test]
fn random_fleet_counts_and_finite_weights_checked() {
    for text in [
        "create_random_fleet={size=1.5}",
        "create_random_fleet={size=2 ship_designs={{design=NAME_Alpha weight=NaN}}}",
        "create_random_fleet={size=2 ship_designs={{design=NAME_Alpha min=4 max=2}}}",
    ] {
        let e = random_fleet_case(text);
        assert!(errors(&e) > 0, "{text}: {:?}", e.diagnostics.rows);
    }
}
#[test]
fn random_fleet_unreachable_entries_still_check_references_and_order() {
    let e = random_fleet_case(
        "if={limit={always=no} create_random_fleet={size=2 ship_designs={{design=missing min=4 max=2}}}}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "range-order"));
}
#[test]
fn random_fleet_wrong_keyed_list_and_wrong_fields_rejected() {
    for text in [
        "create_random_fleet>={size=2}",
        "create_random_fleet={size=2 ship_designs={NAME_Alpha=1}}",
        "create_random_fleet={size=2 conditions={always=yes}}",
        "create_random_fleet={size=2 ship_designs={{design=NAME_Alpha weight>=1}}}",
    ] {
        let e = random_fleet_case(text);
        assert!(errors(&e) > 0, "{text}: {:?}", e.diagnostics.rows);
    }
}
#[test]
fn random_fleet_wrong_callback_this_is_detected() {
    let e = random_fleet_case("create_random_fleet={size=10 effect={set_country_flag=bad}}");
    assert!(
        e.diagnostics.rows.iter().any(|d| d.key == "scope"),
        "{:?}",
        e.diagnostics.rows
    );
}
#[test]
fn random_fleet_callback_retains_caller_root_and_prev() {
    let e = random_fleet_case(
        "create_random_fleet={size=10 effect={set_fleet_flag=ok root={set_country_flag=ok} prev={set_country_flag=ok}}}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}
#[test]
fn random_fleet_negative_and_empty_semantics_remain_unknown() {
    let e = random_fleet_case(
        "create_random_fleet={size=2 ship_designs={{design=NAME_Alpha weight=-1}}}",
    );
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "random-fleet-domain-unverified"));
    let e = random_fleet_case("create_random_fleet={size=2 ship_designs={}}");
    assert!(has(&e, "random-fleet-empty-list-unverified"));
}
#[test]
fn random_fleet_runtime_bounds_remain_unknown() {
    let e = random_fleet_case(
        "create_random_fleet={size=10 ship_designs={{design=NAME_Alpha min=x max=y}}}",
    );
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "numeric-range-unproven"));
}
#[test]
fn random_fleet_large_finite_weight_is_not_a_balance_error() {
    let e = random_fleet_case(
        "create_random_fleet={size=10 ship_designs={{design=NAME_Alpha weight=1e30}}}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}
