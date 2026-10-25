use super::*;

const WEAPON_FIELDS: [&str; 7] = [
    "military_power_multiplier",
    "missile_speed",
    "missile_evasion",
    "missile_shield",
    "missile_armor",
    "missile_health",
    "missile_retarget_range",
];

fn weapon_field(text: &str) -> Engine {
    let mut e = make();
    let nodes = ast(&mut e, text);
    for n in &nodes {
        assert!(e.weapon_numeric_contract(n, &Context::new("ship")));
    }
    e
}

#[test]
fn weapon_numeric_current_native_missile_csv_values_are_valid() {
    // weapon_components.csv:98, MISSILE_1. Only the seven field values are
    // transcribed from the CSV: this is not evidence of inline range support.
    let e = weapon_field(
        "military_power_multiplier=1 missile_speed=400 missile_evasion=0 missile_shield=0 missile_armor=3 missile_health=3 missile_retarget_range=100",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn weapon_numeric_actual_status_power_inline_fields_are_valid() {
    // sp_independent_components.txt:1687-1693, unchanged scalar fields.
    let e = weapon_field(
        "military_power_multiplier=1.00 missile_speed=0.00 missile_evasion=0.00 missile_shield=0.00 missile_armor=0.00 missile_health=0.00 missile_retarget_range=0.00",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn weapon_numeric_finite_high_zero_and_signed_values_have_no_balance_cap() {
    for field in WEAPON_FIELDS {
        for value in ["0", "0.0", "-0.0", "1000000000000", "-2", "1e30"] {
            let e = weapon_field(&format!("{field}={value}"));
            assert!(
                e.diagnostics.rows.is_empty(),
                "{field}={value}: {:?}",
                e.diagnostics.rows
            );
        }
    }
}

#[test]
fn weapon_numeric_valid_ranges_retain_engine_support_boundary() {
    for field in WEAPON_FIELDS {
        let e = weapon_field(&format!("{field}={{min=0 max=1000000000000}}"));
        assert_eq!(errors(&e), 0);
        assert!(has(&e, "weapon-numeric-range-engine-unverified"));
    }
}

#[test]
fn weapon_numeric_equal_zero_range_is_not_reversed() {
    let e = weapon_field("missile_speed={min=0 max=0}");
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "weapon-numeric-range-engine-unverified"));
}

#[test]
fn weapon_numeric_reversed_ranges_are_detected_for_every_field() {
    for field in WEAPON_FIELDS {
        let e = weapon_field(&format!("{field}={{min=10 max=2}}"));
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "range-order"),
            "{field}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn weapon_numeric_omitted_endpoints_retain_unknown_defaults() {
    for range in ["{}", "{min=0}", "{max=1}"] {
        let e = weapon_field(&format!("missile_health={range}"));
        assert_eq!(errors(&e), 0, "{range}: {:?}", e.diagnostics.rows);
        assert!(has(&e, "weapon-numeric-range-engine-unverified"));
    }
}

#[test]
fn weapon_numeric_unknown_range_children_are_errors() {
    let e = weapon_field("missile_health={min=0 max=3 mx=4}");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "unknown-field"));
}

#[test]
fn weapon_numeric_duplicate_endpoints_are_errors() {
    let e = weapon_field("missile_armor={min=0 min=0 max=3}");
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "duplicate-field")
    );
}

#[test]
fn weapon_numeric_endpoint_requires_a_scalar() {
    let e = weapon_field("missile_shield={min={min=0 max=1} max=3}");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "structure"));
}

#[test]
fn weapon_numeric_field_requires_assignment() {
    let e = weapon_field("missile_speed>1");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "operator"));
}

#[test]
fn weapon_numeric_range_endpoint_requires_assignment() {
    let e = weapon_field("missile_retarget_range={min>0 max=3}");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "operator"));
}

#[test]
fn weapon_numeric_nonfinite_scalars_are_errors() {
    for value in ["NaN", "inf", "-inf", "1e999"] {
        let e = weapon_field(&format!("military_power_multiplier={value}"));
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "value"),
            "{value}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn weapon_numeric_nonfinite_endpoints_are_errors() {
    for range in ["{min=NaN max=1}", "{min=0 max=inf}", "{min=-inf max=3}"] {
        let e = weapon_field(&format!("missile_evasion={range}"));
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "value"),
            "{range}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn weapon_numeric_malformed_token_is_not_a_runtime_variable() {
    let e = weapon_field("missile_speed=\"1!\"");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "value"));
}

#[test]
fn weapon_numeric_runtime_quantity_is_unknown_not_nonexistent() {
    let e = weapon_field("missile_speed=runtime_missile_speed");
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "weapon-numeric-expression-unverified"));
    assert!(has(&e, "numeric-binding-unproven"));
    assert!(
        !e.diagnostics
            .rows
            .iter()
            .any(|d| matches!(d.key.as_str(), "variable" | "missing-item"))
    );
}

#[test]
fn weapon_numeric_runtime_endpoint_does_not_prove_order() {
    let e = weapon_field("missile_speed={min=runtime_min max=0}");
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "weapon-numeric-range-engine-unverified"));
    assert!(has(&e, "weapon-numeric-expression-unverified"));
}

#[test]
fn weapon_numeric_boolean_tokens_retain_type_contract_boundary() {
    for token in ["yes", "no"] {
        let e = weapon_field(&format!("missile_health={token}"));
        assert_eq!(errors(&e), 0);
        assert!(has(&e, "weapon-numeric-expression-unverified"));
    }
}

#[test]
fn weapon_numeric_missing_script_value_reference_remains_an_error() {
    let e = weapon_field("missile_armor=value:missing");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
}

#[test]
fn weapon_numeric_unknown_numeric_trigger_remains_an_error() {
    let e = weapon_field("missile_speed=trigger:not_a_trigger");
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "unknown-trigger")
    );
}

#[test]
fn weapon_numeric_evaluation_scope_is_not_borrowed_from_on_hit() {
    let e = weapon_field("missile_health=trigger:fleet_power");
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "scope-type-unknown"));
    assert!(has(&e, "weapon-numeric-expression-unverified"));
}

#[test]
fn weapon_numeric_script_value_constant_does_not_certify_declaration_semantics() {
    let mut e = make();
    let definitions = ast(&mut e, "runtime_value={base=10}");
    e.program.index(File {
        path: "fixture.txt".into(),
        category: "script_values".into(),
        ast: definitions.into_iter().map(Arc::new).collect(),
        target: false,
    });
    let nodes = ast(&mut e, "missile_speed={min=value:runtime_value max=0}");
    assert!(e.weapon_numeric_contract(&nodes[0], &Context::new("ship")));
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "weapon-numeric-expression-unverified"));
    assert!(!e.diagnostics.rows.iter().any(|d| d.key == "range-order"));
}

#[test]
fn weapon_numeric_unresolved_reader_retains_boundary() {
    // Normal parser-resolved readers arrive as literals; this deliberately
    // leaves a late/dynamic reader token to test that it is not a missing item.
    let mut e = make();
    let mut nodes = ast(&mut e, "missile_speed=1");
    nodes[0].value = Some("@unresolved_speed".into());
    assert!(e.weapon_numeric_contract(&nodes[0], &Context::new("ship")));
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "weapon-numeric-expression-unverified"));
}

#[test]
fn weapon_numeric_existing_windup_and_fire_time_handlers_are_not_shadowed() {
    let mut e = make();
    let nodes = ast(
        &mut e,
        "windup={min=1 max=2} total_fire_time=1 cooldown=1 damage=3",
    );
    for n in &nodes {
        assert!(!e.weapon_numeric_contract(n, &Context::new("ship")));
    }
    assert!(e.diagnostics.rows.is_empty());
}
