use super::*;

// Local Stellaris Cygnus 4.5.2 primary sources (2026-10-24):
// effects.log:1761-1767,3800-3808
// SHA256 bd7bfa135c89a01a423833acab77fd78e19a5311bf4532d6f4a55584b0d4747b
// common/agreement_terms/00_agreement_terms.txt:1-2,31-36
// SHA256 cbb320878a0c9a4a173b25bbf304fd949b4c1ce6a7c2939170d8739cd39a9fed
// common/agreement_term_values/00_agreement_term_values.txt:7-8,497-498,929-930
// SHA256 5b92005b42fc9c24de0294578e67af7fb22a528435fb45d92ff2e161a32f3b4c
// events/machine_age_crisis_events.txt:5549-5556 (Status power:5539-5546)
// native SHA256 02a764024b3d24dff234fc249685ee67e7977e2a34b73578c7307403bfcd601f
// Status SHA256 56a382bed874155efc8ba27a1d427e9c37bc6cabd82643c6967bfbcc3db05b7a
// events/nomads_arkship_events.txt:5473-5502
// SHA256 838950f4ffa5a3d2eb48a03a8e45d757aab3fee8fb8130bbd51855e188541240
// Fixtures register only the metadata needed by the tested contracts; they do
// not certify the runtime availability/possible predicates of an agreement.
fn agreement_debris_engine() -> Engine {
    let mut e = make();
    for (category, text) in [
        (
            "agreement_terms",
            "joins_overlord_wars={term_type=discrete} subject_diplomacy={term_type=discrete} resource_subsidies={term_type=resource} counted_term={term_type=discrete_number} specialist_term={term_type=specialist_type} unknown_term={term_type=future_kind} metadata_missing={}",
        ),
        (
            "agreement_term_values",
            "joins_overlord_wars_none={term=joins_overlord_wars} subject_can_not_do_diplomacy={term=subject_diplomacy} no_membership={} dynamic_membership={term=$TERM$}",
        ),
        ("strategic_resources", "alloys={} minerals={} nanites={}"),
        ("script_values", "two={base=2} bad_divide={base=1 divide=x}"),
    ] {
        let nodes = ast(&mut e, text);
        e.program.index(File {
            path: format!("base-{category}.txt"),
            category: category.into(),
            ast: nodes.into_iter().map(Arc::new).collect(),
            target: false,
        });
    }
    e
}

fn agreement_debris_case(text: &str, ctx: Context, mut state: State) -> Engine {
    let mut e = agreement_debris_engine();
    let nodes = ast(&mut e, text);
    e.effects(&nodes, &ctx, &mut state);
    e
}

fn agreement_case(text: &str) -> Engine {
    agreement_debris_case(text, Context::new("agreement"), State::default())
}

fn debris_case(text: &str) -> Engine {
    let mut ctx = Context::new("country");
    ctx.froms = vec![Types::new("ambient_object"), Types::new("galactic_object")];
    agreement_debris_case(text, ctx, State::default())
}

#[test]
fn agreement_native_discrete_assignment_and_exported_multiple_terms_are_valid() {
    let e = agreement_case(
        "set_agreement_terms={joins_overlord_wars=joins_overlord_wars_none subject_diplomacy=subject_can_not_do_diplomacy}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn agreement_missing_term_and_value_references_remain_errors() {
    for text in [
        "set_agreement_terms={missing_term=joins_overlord_wars_none}",
        "set_agreement_terms={joins_overlord_wars=missing_value}",
    ] {
        let e = agreement_case(text);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "missing-item"),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn agreement_value_membership_rejects_a_value_from_another_term() {
    let e =
        agreement_case("set_agreement_terms={joins_overlord_wars=subject_can_not_do_diplomacy}");
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "value" && d.message.contains("belongs to")),
        "{:?}",
        e.diagnostics.rows
    );
}

#[test]
fn agreement_known_scalar_contracts_and_assignment_operators_are_checked() {
    for (text, key) in [
        ("set_agreement_terms=yes", "structure"),
        (
            "set_agreement_terms={joins_overlord_wars={value=joins_overlord_wars_none}}",
            "structure",
        ),
        (
            "set_agreement_terms>={joins_overlord_wars=joins_overlord_wars_none}",
            "operator",
        ),
        (
            "set_agreement_terms={joins_overlord_wars!=joins_overlord_wars_none}",
            "operator",
        ),
        (
            "set_agreement_terms={joins_overlord_wars_none}",
            "structure",
        ),
    ] {
        let e = agreement_case(text);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn agreement_dynamic_identity_and_value_remain_unknown() {
    for text in [
        "set_agreement_terms={$TERM$=1}",
        "set_agreement_terms={joins_overlord_wars=$VALUE$}",
    ] {
        let e = agreement_case(text);
        assert_eq!(errors(&e), 0, "{text}: {:?}", e.diagnostics.rows);
        assert!(
            has(&e, "dynamic-reference"),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn agreement_exported_alloys_subsidy_accepts_finite_values_without_balance_limits() {
    for value in ["0.5", "0", "-1000000000000", "1e30"] {
        let e = agreement_case(&format!(
            "set_agreement_terms={{resource_subsidies_alloys={value}}}"
        ));
        assert!(
            e.diagnostics.rows.is_empty(),
            "{value}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn agreement_alloys_subsidy_rejects_nonfinite_and_block_values() {
    for (value, key) in [
        ("NaN", "value"),
        ("inf", "value"),
        ("{amount=0.5}", "structure"),
    ] {
        let e = agreement_case(&format!(
            "set_agreement_terms={{resource_subsidies_alloys={value}}}"
        ));
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{value}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn agreement_other_resource_selectors_and_direct_resource_terms_stay_unknown() {
    for (text, reason) in [
        (
            "set_agreement_terms={resource_subsidies_minerals=0.5}",
            "agreement-resource-selector-unverified",
        ),
        (
            "set_agreement_terms={resource_subsidies=resource_subsidies_basic}",
            "agreement-resource-term-form-unverified",
        ),
    ] {
        let e = agreement_case(text);
        assert_eq!(errors(&e), 0, "{text}: {:?}", e.diagnostics.rows);
        assert!(has(&e, reason));
    }
}

#[test]
fn agreement_unproved_term_types_do_not_borrow_the_discrete_contract() {
    for term in [
        "counted_term",
        "specialist_term",
        "unknown_term",
        "metadata_missing",
    ] {
        let e = agreement_case(&format!(
            "set_agreement_terms={{{term}=not_a_proven_value}}"
        ));
        assert_eq!(errors(&e), 0, "{term}: {:?}", e.diagnostics.rows);
        assert!(has(&e, "agreement-term-contract-unverified"));
    }
}

#[test]
fn agreement_missing_or_dynamic_value_membership_is_not_assumed() {
    for value in ["no_membership", "dynamic_membership"] {
        let e = agreement_case(&format!(
            "set_agreement_terms={{joins_overlord_wars={value}}}"
        ));
        assert_eq!(errors(&e), 0, "{value}: {:?}", e.diagnostics.rows);
        assert!(has(&e, "agreement-term-membership-unverified"));
    }
}

#[test]
fn agreement_repeated_terms_and_empty_assignment_do_not_claim_full_validation() {
    for (text, reason) in [
        (
            "set_agreement_terms={joins_overlord_wars=joins_overlord_wars_none joins_overlord_wars=joins_overlord_wars_none}",
            "agreement-term-composition-unverified",
        ),
        (
            "set_agreement_terms={}",
            "agreement-term-contract-unverified",
        ),
    ] {
        let e = agreement_case(text);
        assert_eq!(errors(&e), 0, "{text}: {:?}", e.diagnostics.rows);
        assert!(has(&e, reason));
    }
}

#[test]
fn agreement_wrong_and_unknown_receivers_keep_the_normal_input_checks() {
    let e = agreement_debris_case(
        "set_agreement_terms={joins_overlord_wars=joins_overlord_wars_none}",
        Context::new("country"),
        State::default(),
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "scope"));
    let e = agreement_debris_case(
        "set_agreement_terms={joins_overlord_wars=missing_value}",
        Context::unknown_caller(),
        State::default(),
    );
    assert!(has(&e, "scope-type-unknown"));
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
}

#[test]
fn agreement_unreachable_missing_values_still_fail_static_reference_checks() {
    let e = agreement_case(
        "if={limit={always=no} set_agreement_terms={joins_overlord_wars=missing_value}}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
}

#[test]
fn agreement_runtime_resource_expressions_keep_an_unknown_evaluation_boundary() {
    for value in ["runtime_amount", "trigger:fleet_power", "value:two"] {
        let e = agreement_case(&format!(
            "set_agreement_terms={{resource_subsidies_alloys={value}}}"
        ));
        assert_eq!(errors(&e), 0, "{value}: {:?}", e.diagnostics.rows);
        assert!(has(&e, "agreement-resource-value-unverified"));
    }
}

#[test]
fn agreement_unknown_expression_frame_does_not_borrow_caller_numeric_facts() {
    let mut state = State::default();
    state.set_number(
        State::number_key("root", "x"),
        crate::domain::NumberFact::assigned(Some(0.0)),
    );
    let e = agreement_debris_case(
        "set_agreement_terms={resource_subsidies_alloys=value:bad_divide}",
        Context::new("agreement"),
        state,
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "agreement-resource-value-unverified"));
    assert!(has(&e, "numeric-divisor-unproven"));
}

#[test]
fn agreement_unknown_expression_context_still_checks_fixed_value_references() {
    let e = agreement_case("set_agreement_terms={resource_subsidies_alloys=value:missing_value}");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
    assert!(has(&e, "agreement-resource-value-unverified"));
}

#[test]
fn debris_shipped_alloys_awards_with_system_fromfrom_are_valid() {
    for amount in ["1000", "2000", "4000"] {
        let e = debris_case(&format!(
            "add_resource_from_debris={{alloys={amount} system=fromfrom}}"
        ));
        assert!(
            e.diagnostics.rows.is_empty(),
            "{amount}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn debris_multiple_resource_values_multiplier_and_high_values_are_valid() {
    let e = debris_case(
        "add_resource_from_debris={alloys=1e30 minerals=-1000000 nanites=0 system=fromfrom mult=2}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn debris_resource_typo_and_nonfinite_quantity_remain_errors() {
    for (text, key) in [
        (
            "add_resource_from_debris={allosy=1 system=fromfrom}",
            "missing-item",
        ),
        (
            "add_resource_from_debris={alloys=NaN system=fromfrom}",
            "value",
        ),
        (
            "add_resource_from_debris={alloys=1 system=fromfrom mult=inf}",
            "value",
        ),
    ] {
        let e = debris_case(text);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn debris_named_map_and_scalar_field_shapes_are_checked() {
    for text in [
        "add_resource_from_debris=yes",
        "add_resource_from_debris={alloys={amount=1}}",
        "add_resource_from_debris={alloys=1 system={solar_system=yes}}",
        "add_resource_from_debris={alloys=1 mult={amount=2}}",
        "add_resource_from_debris={alloys}",
    ] {
        let e = debris_case(text);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "structure"),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn debris_parent_and_child_assignment_operators_are_checked() {
    for text in [
        "add_resource_from_debris>={alloys=1 system=fromfrom}",
        "add_resource_from_debris={alloys>=1 system=fromfrom}",
        "add_resource_from_debris={alloys=1 system!=fromfrom}",
    ] {
        let e = debris_case(text);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "operator"),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn debris_wrong_receiver_and_wrong_system_are_distinct_scope_errors() {
    for (text, scope) in [
        ("add_resource_from_debris={alloys=1}", "planet"),
        ("add_resource_from_debris={alloys=1 system=root}", "country"),
    ] {
        let e = agreement_debris_case(text, Context::new(scope), State::default());
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "scope"),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn debris_unknown_system_binding_and_type_do_not_become_hard_errors() {
    let e = debris_case("add_resource_from_debris={alloys=1 system=event_target:unknown_system}");
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "target-binding-unproven"));
    assert!(has(&e, "scope-type-unknown"));
}

#[test]
fn debris_unknown_runtime_quantities_are_not_missing_variables() {
    let e = debris_case(
        "add_resource_from_debris={alloys=runtime_amount system=fromfrom mult=runtime_multiplier}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "numeric-binding-unproven"));
}

#[test]
fn debris_numeric_operands_use_pre_effect_inputs_and_check_value_references() {
    let mut e = agreement_debris_engine();
    let n = ast(&mut e, "add_resource_from_debris={alloys=value:bad_divide}").remove(0);
    let mut inputs = State::default();
    inputs.set_number(
        State::number_key("root", "x"),
        crate::domain::NumberFact::assigned(Some(0.0)),
    );
    assert!(e.agreement_debris_parameters(&n, &Context::new("country"), &inputs));
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "value" && d.message.contains("zero")),
        "{:?}",
        e.diagnostics.rows
    );
    let e = debris_case("add_resource_from_debris={alloys=value:missing_value}");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
}

#[test]
fn debris_dynamic_resource_identity_preserves_unknown_reference_diagnostics() {
    let e = debris_case("add_resource_from_debris={$RESOURCE$=1 system=fromfrom}");
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "dynamic-reference"));
}

#[test]
fn debris_missing_fields_are_not_invented_and_repetition_remains_unknown() {
    let e = debris_case("add_resource_from_debris={alloys=1}");
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "debris-system-default-unverified"));
    for (text, reason) in [
        (
            "add_resource_from_debris={}",
            "debris-resource-contract-unverified",
        ),
        (
            "add_resource_from_debris={alloys=1 alloys=2}",
            "debris-resource-composition-unverified",
        ),
    ] {
        let e = debris_case(text);
        assert_eq!(errors(&e), 0, "{text}: {:?}", e.diagnostics.rows);
        assert!(has(&e, reason));
    }
}

#[test]
fn debris_missing_references_are_checked_with_unknown_or_unreachable_contexts() {
    let e = agreement_debris_case(
        "add_resource_from_debris={missing_resource=1}",
        Context::unknown_caller(),
        State::default(),
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
    assert!(has(&e, "scope-type-unknown"));
    let e = debris_case("if={limit={always=no} add_resource_from_debris={missing_resource=1}}");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
}

#[test]
fn agreement_and_debris_hooks_do_not_claim_other_effects() {
    let mut e = agreement_debris_engine();
    let n = ast(&mut e, "set_country_flag=ordinary").remove(0);
    assert!(!e.agreement_debris_structure(&n));
    assert!(!e.agreement_debris_parameters(&n, &Context::new("country"), &State::default()));
    assert!(e.diagnostics.rows.is_empty());
}

fn unreachable_agreement_debris_numbers(value: &str) -> [String; 3] {
    [
        format!(
            "if={{limit={{always=no}} add_resource_from_debris={{alloys={value} system=fromfrom}}}}"
        ),
        format!(
            "if={{limit={{always=no}} add_resource_from_debris={{alloys=1 mult={value} system=fromfrom}}}}"
        ),
        format!(
            "if={{limit={{always=no}} set_agreement_terms={{resource_subsidies_alloys={value}}}}}"
        ),
    ]
}

#[test]
fn unreachable_numeric_amount_multiplier_and_subsidy_still_check_fixed_values() {
    for text in unreachable_agreement_debris_numbers("value:missing_value") {
        let e = debris_case(&text);
        assert!(
            e.diagnostics
                .rows
                .iter()
                .any(|d| d.key == "missing-item" && d.message.contains("missing_value")),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn unreachable_numeric_amount_multiplier_and_subsidy_accept_existing_values() {
    for text in unreachable_agreement_debris_numbers("value:two") {
        let e = debris_case(&text);
        assert!(
            e.diagnostics.rows.is_empty(),
            "{text}: {:?}",
            e.diagnostics.rows
        );
        assert!(!e.invoked_values.contains("two"));
    }
}

#[test]
fn unreachable_numeric_values_do_not_evaluate_or_borrow_caller_zero_facts() {
    for text in unreachable_agreement_debris_numbers("value:bad_divide") {
        let mut e = agreement_debris_engine();
        let body = ast(&mut e, &text);
        let mut state = State::default();
        state.set_number(
            State::number_key("root", "x"),
            crate::domain::NumberFact::assigned(Some(0.0)),
        );
        e.effects(&body, &Context::new("country"), &mut state);
        assert!(
            e.diagnostics.rows.is_empty(),
            "{text}: {:?}",
            e.diagnostics.rows
        );
        assert!(!e.invoked_values.contains("bad_divide"));
        assert_eq!(
            state.number(&State::number_key("root", "x")).value(),
            Some(0.0)
        );
    }
}

#[test]
fn unreachable_numeric_dynamic_identity_and_arguments_remain_unknown() {
    for value in ["\"value:@reader\"", "\"value:two|N|@reader|\""] {
        for text in unreachable_agreement_debris_numbers(value) {
            let e = debris_case(&text);
            assert_eq!(errors(&e), 0, "{text}: {:?}", e.diagnostics.rows);
            assert!(
                has(&e, "numeric-information-unresolved"),
                "{text}: {:?}",
                e.diagnostics.rows
            );
            assert!(e.invoked_values.is_empty());
        }
    }
}

#[test]
fn unreachable_numeric_fixed_missing_value_precedes_dynamic_argument_boundary() {
    for text in unreachable_agreement_debris_numbers("\"value:missing_value|N|@reader|\"") {
        let e = debris_case(&text);
        assert!(
            e.diagnostics
                .rows
                .iter()
                .any(|d| d.key == "missing-item" && d.message.contains("missing_value")),
            "{text}: {:?}",
            e.diagnostics.rows
        );
        assert!(has(&e, "numeric-information-unresolved"));
        assert!(e.invoked_values.is_empty());
    }
}
