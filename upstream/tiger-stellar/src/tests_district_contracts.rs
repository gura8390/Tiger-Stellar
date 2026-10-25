use super::*;

fn district_case(text: &str, scope: &str) -> Engine {
    let mut e = make();
    e.program.entities.insert(
        "districts".into(),
        [
            "district_test",
            "district_farming",
            "district_orchard_forests",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
    );
    let body = ast(&mut e, text);
    e.triggers(&body, &Context::new(scope), &State::default());
    e
}

#[test]
fn has_district_native_literal_and_presence_forms_are_valid() {
    for scope in ["planet", "ship", "colony"] {
        for text in [
            "has_district=district_test",
            "has_district=yes",
            "has_district=no",
            "OR={has_district=district_farming has_district=district_orchard_forests}",
        ] {
            let e = district_case(text, scope);
            assert!(
                e.diagnostics.rows.is_empty(),
                "{scope}: {text}: {:?}",
                e.diagnostics.rows
            );
        }
    }
}

#[test]
fn has_district_missing_literal_still_fails_in_unreachable_code() {
    for text in [
        "has_district=missing_district",
        "if={limit={always=no} has_district=missing_district}",
    ] {
        let e = district_case(text, "planet");
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "missing-item"),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn has_district_wrong_scope_is_not_hidden_by_reference_support() {
    for scope in ["country", "fleet", "none"] {
        let e = district_case("has_district=district_test", scope);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "scope"),
            "{scope}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn has_district_rejects_undocumented_blocks_and_missing_id() {
    for (text, key) in [
        ("has_district={type=district_test}", "structure"),
        ("has_district={district_test}", "structure"),
        ("has_district=1", "missing-item"),
    ] {
        let e = district_case(text, "planet");
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn has_district_non_assignment_comparators_retain_unverified_semantics() {
    for text in ["has_district!=district_test", "has_district>=district_test"] {
        let e = district_case(text, "planet");
        assert_eq!(errors(&e), 0, "{text}: {:?}", e.diagnostics.rows);
        assert!(has(&e, "district-comparison-unverified"));
    }
}

#[test]
fn has_district_dynamic_identity_stays_explicitly_unknown() {
    let e = district_case("has_district=$DISTRICT$", "planet");
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "dynamic-reference"));
}

#[test]
fn has_district_guarded_polymorphic_input_retains_normal_narrowing() {
    let e = district_case(
        "if={limit={is_scope_type=planet} has_district=district_test}",
        "planet|country",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}
