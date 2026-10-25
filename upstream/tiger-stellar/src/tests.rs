use crate::{
    ast::Node,
    domain::{Availability, Binding, Context, State, Types},
    engine::Engine,
    loader::{File, Program},
    parser::ParserClient,
    report::Diagnostics,
    rules::{Declarations, Rules},
};
use std::sync::Arc;
#[path = "tests_agreement_debris_contracts.rs"]
mod agreement_debris_contracts;
#[path = "tests_complex_value_queries.rs"]
mod complex_value_queries;
#[path = "tests_creation_callbacks.rs"]
mod creation_callbacks;
#[path = "tests_district_contracts.rs"]
mod district_contracts;
#[path = "tests_documented_on_actions.rs"]
mod documented_on_actions;
#[path = "tests_effect_map_contracts.rs"]
mod effect_map_contracts;
#[path = "tests_event_option_inline.rs"]
mod event_option_inline;
#[path = "tests_graphics_detail_contracts.rs"]
mod graphics_detail_contracts;
#[path = "tests_guard_detection_edges.rs"]
mod guard_detection_edges;
#[path = "tests_named_engine_entries.rs"]
mod named_engine_entries;
#[path = "tests_on_action_contracts.rs"]
mod on_action_contracts;
#[path = "tests_random_fleet_contracts.rs"]
mod random_fleet_contracts;
#[path = "tests_saved_target_demands.rs"]
mod saved_target_demands;
#[path = "tests_saved_target_types.rs"]
mod saved_target_types;
#[path = "tests_spawn_planet_contracts.rs"]
mod spawn_planet_contracts;
#[path = "tests_timeline_contracts.rs"]
mod timeline_contracts;
#[path = "tests_value_contract_edges.rs"]
mod value_contract_edges;
#[path = "tests_weapon_numeric_contracts.rs"]
mod weapon_numeric_contracts;
fn make() -> Engine {
    let rules: Rules =
        serde_json::from_str(include_str!("../../../rules/stellaris-4.5.2.json")).unwrap();
    rules.validate().unwrap();
    let decl: Declarations =
        serde_json::from_str(include_str!("../../../rules/declarations-4.5.2.json")).unwrap();
    Engine::new(
        rules,
        decl,
        Program::default(),
        ParserClient::direct(),
        Diagnostics::default(),
    )
}
fn value_case(definitions: &str, body: &str) -> (Engine, State) {
    let mut e = make();
    let nodes = ast(&mut e, definitions);
    e.program.index(File {
        path: "fixture.txt".into(),
        category: "script_values".into(),
        ast: nodes.into_iter().map(Arc::new).collect(),
        target: false,
    });
    let body = ast(&mut e, body);
    let mut state = State::default();
    e.effects(&body, &Context::new("country"), &mut state);
    (e, state)
}
#[test]
fn script_value_derived_zero_is_detected() {
    let (e, _) = value_case(
        "zero={base=2 subtract=2}",
        "set_variable={which=x value=1} divide_variable={which=x value=value:zero}",
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
fn script_value_missing_reference_is_an_error() {
    let (e, _) = value_case("", "set_variable={which=x value=value:missing}");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
}
#[test]
fn script_value_unknown_operation_is_an_error() {
    let (e, _) = value_case(
        "v={base=2 multipy=0}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "unknown-field"));
}
#[test]
fn script_value_base_is_initial_not_an_inline_reset() {
    let (e, s) = value_case("v={add=1 base=2}", "set_variable={which=x value=value:v}");
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    assert_eq!(s.number(&State::number_key("root", "x")).value(), Some(3.0));
}
#[test]
fn script_value_min_is_a_floor_and_max_is_a_cap() {
    let (e, s) = value_case(
        "v={base=1 min=5 max=3}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    assert_eq!(s.number(&State::number_key("root", "x")).value(), Some(3.0));
}
#[test]
fn script_value_modifier_factor_multiplies_but_inline_factor_initializes() {
    let (e, s) = value_case(
        "v={factor=2 modifier={factor=3 always=yes}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    assert_eq!(s.number(&State::number_key("root", "x")).value(), Some(6.0));
}
#[test]
fn script_value_unknown_guard_joins_constants() {
    let (e, s) = value_case(
        "v={base=1 modifier={set=0 is_ai=yes}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    assert_eq!(s.number(&State::number_key("root", "x")).value(), None);
}
#[test]
fn script_value_false_modifier_does_not_apply_math() {
    let (e, s) = value_case(
        "v={base=2 modifier={set=0 always=no}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    assert_eq!(s.number(&State::number_key("root", "x")).value(), Some(2.0));
}
#[test]
fn script_value_false_modifier_still_checks_reference_grammar() {
    let (e, _) = value_case(
        "v={modifier={set=value:missing always=no}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
}
#[test]
fn script_value_guard_can_prove_variable_nonzero_without_exporting_facts() {
    let (e, s) = value_case(
        "v={base=1 modifier={divide=x is_variable_set=x check_variable={which=x value>0}}}",
        "set_variable={which=result value=value:v}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    assert_eq!(
        s.number(&State::number_key("root", "x")).availability,
        Availability::Maybe
    );
}
#[test]
fn script_value_wrong_scope_is_detected() {
    let (e, _) = value_case(
        "v={modifier={add=1 is_star=yes}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(
        e.diagnostics.rows.iter().any(|d| d.key == "scope"),
        "{:?}",
        e.diagnostics.rows
    );
}
#[test]
fn script_value_scoped_call_retains_root() {
    let (e, _) = value_case(
        "v={modifier={add=1 root={is_ai=yes} is_star=yes}}",
        "home_planet={set_variable={which=x value=value:v}}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}
#[test]
fn script_value_pipe_parameters_are_expanded() {
    let (e, s) = value_case(
        "v={base=$N$ add=1}",
        "set_variable={which=x value=value:v|N|2|}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    assert_eq!(s.number(&State::number_key("root", "x")).value(), Some(3.0));
}
#[test]
fn script_value_missing_macro_argument_is_detected() {
    let (e, _) = value_case("v={base=$N$}", "set_variable={which=x value=value:v}");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "macro-argument"));
}
#[test]
fn script_value_duplicate_macro_argument_is_detected() {
    let (e, _) = value_case(
        "v={base=$N$}",
        "set_variable={which=x value=value:v|N|1|N|2|}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "macro-argument"));
}
#[test]
fn script_value_nested_call_retains_caller_numeric_state() {
    let (e, s) = value_case(
        "v={base=value:w} w={base=x add=2}",
        "set_variable={which=x value=2} set_variable={which=y value=value:v}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    assert_eq!(s.number(&State::number_key("root", "y")).value(), Some(4.0));
}
#[test]
fn script_value_recursive_calls_stop_with_explicit_boundary() {
    let (e, _) = value_case(
        "v={base=value:w} w={base=value:v}",
        "set_variable={which=x value=value:v}",
    );
    assert!(has(&e, "analysis-limit"));
    assert!(!e.complete);
}
#[test]
fn script_value_unary_requires_boolean() {
    let (e, _) = value_case("v={base=1 round=2}", "set_variable={which=x value=value:v}");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "value"));
}
#[test]
fn script_value_unknown_runtime_quantity_is_not_missing() {
    let (e, s) = value_case(
        "v={base=trigger:is_difficulty add=1}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    assert_eq!(s.number(&State::number_key("root", "x")).value(), None);
}
#[test]
fn script_value_galaxy_setup_query_parameters_checked() {
    let (e, _) = value_case(
        "v={complex_trigger_modifier={trigger=check_galaxy_setup_value parameters={setting=typo} mode=add}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "value"));
}
#[test]
fn script_value_valid_galaxy_setup_query() {
    let (e, _) = value_case(
        "v={complex_trigger_modifier={trigger=check_galaxy_setup_value parameters={setting=fallen_empire_strength_scale} mode=mult}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}
#[test]
fn script_value_unproved_complex_query_retains_warning() {
    let (e, _) = value_case(
        "v={complex_trigger_modifier={trigger=count_owned_planet parameters={limit={is_star=yes}} mode=add}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(has(&e, "numeric-trigger-contract-unverified"));
}
fn numeric_effect(text: &str) -> (Engine, State) {
    let mut e = make();
    let body = ast(&mut e, text);
    let mut state = State::default();
    e.effects(&body, &Context::new("country"), &mut state);
    (e, state)
}
#[test]
fn script_value_count_comparison_uses_actual_caller_numeric_state() {
    let mut e = make();
    let nodes = ast(&mut e, "v={base=1 divide=x}");
    e.program.index(File {
        path: "fixture.txt".into(),
        category: "script_values".into(),
        ast: nodes.into_iter().map(Arc::new).collect(),
        target: false,
    });
    let count = ast(&mut e, "my_count={count_owned_planet={limit={always=yes}}}")[0].clone();
    e.program
        .scripts
        .insert((false, "my_count".into()), Arc::new(count));
    let nodes = ast(
        &mut e,
        "set_variable={which=x value=2} if={limit={my_count>=value:v} log=ok}",
    );
    e.effects(&nodes, &Context::new("country"), &mut State::default());
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}
#[test]
fn script_value_delay_argument_reads_pre_dispatch_numbers() {
    let mut e = make();
    let nodes = ast(&mut e, "v={base=x}");
    e.program.index(File {
        path: "fixture.txt".into(),
        category: "script_values".into(),
        ast: nodes.into_iter().map(Arc::new).collect(),
        target: false,
    });
    let event = ast(&mut e, "country_event={id=b}")[0].clone();
    e.program
        .events
        .insert("b".into(), ("country".into(), Arc::new(event)));
    let nodes = ast(
        &mut e,
        "set_variable={which=x value=2} country_event={id=b days=value:v}",
    );
    e.effects(&nodes, &Context::new("country"), &mut State::default());
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}
#[test]
fn script_value_round_to_does_not_fold_unverified_intermediate_precision() {
    let (e, _) = value_case(
        "v={base=1 round_to=3}",
        "set_variable={which=x value=1} divide_variable={which=x value=value:v}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "numeric-divisor-unproven"));
}
#[test]
fn script_value_unverified_unused_argument_is_not_a_confirmed_error() {
    let (e, _) = value_case("v={base=1}", "set_variable={which=x value=value:v|N|2|}");
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "numeric-information-unresolved"));
}
#[test]
fn script_value_target_demands_propagate_through_nested_values_and_events() {
    let mut e = make();
    for (category, text) in [
        (
            "script_values",
            "v={base=value:w} w={modifier={add=1 event_target:t={is_ai=yes}}}",
        ),
        (
            "events",
            "country_event={id=a immediate={save_event_target_as=t country_event={id=b}}} country_event={id=b is_triggered_only=yes immediate={set_variable={which=x value=value:v}}}",
        ),
    ] {
        let nodes = ast(&mut e, text);
        e.program.index(File {
            path: "fixture.txt".into(),
            category: category.into(),
            ast: nodes.into_iter().map(Arc::new).collect(),
            target: category == "events",
        });
    }
    e.analyze();
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}
#[test]
fn script_value_explicit_selector_is_resolved_before_value_name() {
    let (e, s) = value_case(
        "v={base=2 modifier={add=1 is_ai=yes}}",
        "set_variable={which=x value=root.value:v}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    assert_eq!(s.number(&State::number_key("root", "x")).value(), None);
}
#[test]
fn script_value_rounding_tie_remains_uncertain() {
    let (e, _) = value_case(
        "v={base=0.5 round=yes}",
        "set_variable={which=x value=1} divide_variable={which=x value=value:v}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "numeric-divisor-unproven"));
}
#[test]
fn script_value_conditional_parameter_can_be_omitted() {
    let (e, s) = value_case(
        "v={base=2 [[N] add=$N$ ]}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    assert_eq!(s.number(&State::number_key("root", "x")).value(), Some(2.0));
}
#[test]
fn script_value_unknown_complex_trigger_is_detected() {
    let (e, _) = value_case(
        "v={complex_trigger_modifier={trigger=missing_numeric_trigger mode=add}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "unknown-trigger")
    );
}
#[test]
fn script_value_unreachable_literal_division_is_still_invalid() {
    let (e, _) = value_case(
        "v={modifier={always=no divide=0}}",
        "set_variable={which=x value=value:v}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "value"));
}
#[test]
fn numeric_assignments_copy_and_arithmetic_detect_a_proven_zero_divisor() {
    let (e, state) = numeric_effect(
        "set_variable={which=x value=2} subtract_variable={which=x value=2} set_variable={which=copy value=var:root.x} set_variable={which=n value=10} divide_variable={which=n value=copy}",
    );
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "value" && d.message.contains("zero")),
        "{:?}",
        e.diagnostics.rows
    );
    assert_eq!(
        state.number(&State::number_key("root", "x")).value(),
        Some(0.0)
    );
}
#[test]
fn numeric_runtime_value_is_defined_without_inventing_its_quantity() {
    let (e, state) = numeric_effect(
        "set_variable={which=x value=external} set_variable={which=y value=x} if={limit={check_variable={which=y value>=0}} log=ok}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert_eq!(
        e.diagnostics
            .rows
            .iter()
            .filter(|d| d.reason["code"] == "numeric-binding-unproven")
            .count(),
        1
    );
    let fact = state.number(&State::number_key("root", "y"));
    assert_eq!(fact.availability, Availability::Definite);
    assert_eq!(fact.value(), None);
}
#[test]
fn numeric_branch_joins_keep_must_assignment_and_widen_different_constants() {
    let (e, state) = numeric_effect(
        "if={limit={is_ai=no} set_variable={which=x value=0}} else={set_variable={which=x value=1}} if={limit={check_variable={which=x value>=0}} log=ok}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    assert_eq!(
        state.number(&State::number_key("root", "x")).availability,
        Availability::Definite
    );
    assert_eq!(state.number(&State::number_key("root", "x")).value(), None);
    let (e, _) = numeric_effect(
        "if={limit={is_ai=no} set_variable={which=x value=0}} else={set_variable={which=x value=-0}} set_variable={which=n value=1} divide_variable={which=n value=x}",
    );
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.message.contains("must not be zero"))
    );
    let (e, _) = numeric_effect(
        "if={limit={is_ai=no} set_variable={which=x value=1}} if={limit={check_variable={which=x value>=0}} log=ok}",
    );
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "numeric-binding-unproven"));
}
#[test]
fn numeric_guards_and_clear_distinguish_unknown_from_proven_unset() {
    let (e, _) =
        numeric_effect("clear_variable=x if={limit={check_variable={which=x value>=0}} log=ok}");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "variable"));
    let (e, _) = numeric_effect(
        "if={limit={is_variable_set=x} if={limit={check_variable={which=x value>=0}} log=ok}}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    let (e, _) = numeric_effect(
        "if={limit={NOT={is_variable_set=x}} if={limit={check_variable={which=x value>=0}} log=ok}}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "variable"));
    let (e, _) = numeric_effect(
        "clear_variable=x if={limit={is_variable_set=x} divide_variable={which=n value=x}}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}
#[test]
fn numeric_stable_guard_recovers_branch_constants() {
    let (e, _) = numeric_effect(
        "if={limit={is_ai=no} set_variable={which=x value=0}} else={set_variable={which=x value=1}} set_variable={which=n value=1} if={limit={is_ai=no} divide_variable={which=n value=x}}",
    );
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.message.contains("must not be zero")),
        "{:?}",
        e.diagnostics.rows
    );
}
#[test]
fn numeric_object_paths_do_not_share_facts_and_possible_alias_writes_invalidate() {
    let (e, _) = numeric_effect(
        "set_variable={which=x value=0} owner={set_variable={which=n value=1} divide_variable={which=n value=x}}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "numeric-binding-unproven"));
    let (e, state) = numeric_effect(
        "set_variable={which=x value=0} owner={set_variable={which=x value=1}} set_variable={which=n value=1} divide_variable={which=n value=x}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert_eq!(state.number(&State::number_key("root", "x")).value(), None);
}
#[test]
fn numeric_unknown_copy_and_rounding_ties_do_not_create_false_constants() {
    for text in [
        "set_variable={which=x value=0} copy_flags_and_variables_from=owner set_variable={which=n value=1} divide_variable={which=n value=x}",
        "set_variable={which=x value=0.5} round_variable=x subtract_variable={which=x value=1} set_variable={which=n value=1} divide_variable={which=n value=x}",
    ] {
        let (e, state) = numeric_effect(text);
        assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
        assert_eq!(state.number(&State::number_key("root", "x")).value(), None);
    }
}
#[test]
fn numeric_loop_join_converges_and_keeps_zero_iteration_path() {
    let (e, state) = numeric_effect(
        "set_variable={which=x value=0} while={limit={is_ai=no} change_variable={which=x value=1}} if={limit={check_variable={which=x value>=0}} log=ok}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    assert!(e.metrics.loop_iterations <= 3);
    assert_eq!(state.number(&State::number_key("root", "x")).value(), None);
    let (e, _) = numeric_effect(
        "while={limit={is_ai=no} set_variable={which=x value=1}} if={limit={check_variable={which=x value>=0}} log=ok}",
    );
    assert!(has(&e, "numeric-binding-unproven"));
}
#[test]
fn numeric_variable_shapes_ranges_and_literal_zero_are_checked_statically() {
    for (text, key) in [
        ("set_variable=yes", "structure"),
        ("set_variable={which=x}", "missing-field"),
        ("set_variable={which={x} value=0}", "structure"),
        ("set_variable={which=x value=0 typo=1}", "unknown-field"),
        ("set_variable={which=x value=NaN}", "value"),
        ("set_variable={which=x value=event_target:t}", "value"),
        (
            "set_variable_to_random_value={which=x min=5 max=2}",
            "range-order",
        ),
        (
            "if={limit={always=no} divide_variable={which=x value=0}}",
            "value",
        ),
    ] {
        let (e, _) = numeric_effect(text);
        assert!(
            e.diagnostics
                .rows
                .iter()
                .any(|d| d.key == key && d.severity == "error"),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}
#[test]
fn numeric_export_establishes_a_number_without_assuming_runtime_value() {
    let mut e = make();
    e.program
        .entities
        .entry("strategic_resources".into())
        .or_default()
        .insert("energy".into());
    let body = ast(
        &mut e,
        "export_resource_stockpile_to_variable={resource=energy variable=stock} set_variable={which=copy value=stock} if={limit={check_variable={which=copy value>=0}} log=ok}",
    );
    let mut state = State::default();
    e.effects(&body, &Context::new("country"), &mut state);
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    assert_eq!(
        state
            .number(&State::number_key("root", "copy"))
            .availability,
        Availability::Definite
    );
    assert_eq!(
        state.number(&State::number_key("root", "copy")).value(),
        None
    );
}
#[test]
fn numeric_alias_write_invalidates_remembered_branch_constant() {
    let (e, state) = numeric_effect(
        "if={limit={is_ai=no} set_variable={which=x value=0}} else={set_variable={which=x value=1}} owner={set_variable={which=x value=2}} set_variable={which=n value=1} if={limit={is_ai=no} divide_variable={which=n value=x}}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert_eq!(state.number(&State::number_key("root", "x")).value(), None);
}
#[test]
fn numeric_scoped_read_requires_a_variable_storage_scope() {
    let mut e = make();
    let body = ast(&mut e, "set_variable={which=x value=root.external}");
    let mut ctx = Context::new("country");
    ctx.root = Types::new("none");
    ctx.root_object = "no-object".into();
    e.effects(&body, &ctx, &mut State::default());
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "scope" && d.message.contains("numeric variable storage")),
        "{:?}",
        e.diagnostics.rows
    );
}
#[test]
fn numeric_nonzero_guards_protect_unknown_runtime_values() {
    for condition in [
        "check_variable={which=x value>0}",
        "check_variable={which=x value<0}",
        "check_variable={which=x value!=0}",
        "NOT={check_variable={which=x value=0}}",
    ] {
        let mut e = make();
        let body = ast(
            &mut e,
            &format!(
                "set_variable={{which=n value=1}} if={{limit={{{condition}}} divide_variable={{which=n value=x}}}}"
            ),
        );
        let mut state = State::default();
        state.set_number(
            State::number_key("root", "x"),
            crate::domain::NumberFact::assigned(None),
        );
        e.effects(&body, &Context::new("country"), &mut state);
        assert!(
            e.diagnostics.rows.is_empty(),
            "{condition}: {:?}",
            e.diagnostics.rows
        );
    }
}
#[test]
fn numeric_unknown_divisor_and_zero_guard_are_not_silently_accepted() {
    for (condition, expected) in [
        (None, "numeric-divisor-unproven"),
        (Some("check_variable={which=x value=0}"), "zero"),
    ] {
        let mut e = make();
        let code = if let Some(condition) = condition {
            format!(
                "set_variable={{which=n value=1}} if={{limit={{{condition}}} divide_variable={{which=n value=x}}}}"
            )
        } else {
            "set_variable={which=n value=1} divide_variable={which=n value=x}".into()
        };
        let body = ast(&mut e, &code);
        let mut state = State::default();
        state.set_number(
            State::number_key("root", "x"),
            crate::domain::NumberFact::assigned(None),
        );
        e.effects(&body, &Context::new("country"), &mut state);
        if expected == "zero" {
            assert!(
                e.diagnostics
                    .rows
                    .iter()
                    .any(|d| d.severity == "error" && d.message.contains("must not be zero")),
                "{:?}",
                e.diagnostics.rows
            );
        } else {
            assert_eq!(errors(&e), 0);
            assert!(has(&e, expected));
            assert!(!has(&e, "numeric-binding-unproven"));
        }
    }
    let (e, _) = numeric_effect(
        "set_variable={which=x value=1} set_variable={which=n value=1} if={limit={check_variable={which=x value=0}} divide_variable={which=n value=x}}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}
#[test]
fn numeric_random_ranges_respect_rounding_and_nonzero_bounds() {
    let (e, _) = numeric_effect(
        "set_variable_to_random_value={which=x min=0.25 max=0.25 rounded=yes} set_variable={which=n value=1} divide_variable={which=n value=x}",
    );
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.message.contains("must not be zero")),
        "{:?}",
        e.diagnostics.rows
    );
    let (e, _) = numeric_effect(
        "set_variable_to_random_value={which=x min=1 max=2} set_variable={which=n value=1} divide_variable={which=n value=x}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    let (e, _) = numeric_effect(
        "set_variable_to_random_value={which=x min=1e100 max=1e101} set_variable={which=n value=1} divide_variable={which=n value=x}",
    );
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "numeric-divisor-unproven"));
}
#[test]
fn numeric_quantity_exports_validate_job_and_resource_parameters() {
    for (trigger, parameter, category, name) in [
        ("num_assigned_jobs", "job", "pop_jobs", "worker"),
        ("has_resource", "resource", "strategic_resources", "energy"),
    ] {
        let mut e = make();
        e.program
            .entities
            .entry(category.into())
            .or_default()
            .insert(name.into());
        let body = ast(
            &mut e,
            &format!(
                "export_trigger_value_to_variable={{trigger={trigger} parameters={{{parameter}={name}}} variable=x}}"
            ),
        );
        e.effects(&body, &Context::new("country"), &mut State::default());
        assert!(
            e.diagnostics.rows.is_empty(),
            "{trigger}: {:?}",
            e.diagnostics.rows
        );
        let body = ast(
            &mut e,
            &format!(
                "export_trigger_value_to_variable={{trigger={trigger} parameters={{{parameter}=missing}} variable=x}}"
            ),
        );
        e.effects(&body, &Context::new("country"), &mut State::default());
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "missing-item"),
            "{trigger}: {:?}",
            e.diagnostics.rows
        );
    }
}
#[test]
fn numeric_independent_branches_remain_factored_and_large_finite_values_are_allowed() {
    let text = (0..40).map(|i| format!("if={{limit={{has_country_flag=f{i}}} set_variable={{which=v{i} value=0}}}} else={{set_variable={{which=v{i} value=1}}}} ")).collect::<String>();
    let (e, state) = numeric_effect(&text);
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    assert_eq!(state.numbers.len(), 40);
    assert_eq!(e.metrics.enumerated_binding_paths, 0);
    let (e, _) =
        numeric_effect("set_variable={which=x value=1e30} multiply_variable={which=x value=1e30}");
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
}
#[test]
fn inferred_script_input_keeps_root_and_previous_caller_unknown() {
    let mut e = make();
    let nodes = ast(
        &mut e,
        "helper={set_country_flag=ok owner={prev={set_country_flag=ok}} root={set_ship_flag=maybe}}",
    );
    let ctx = e.inferred_script_context(true, "helper", &nodes[0]);
    assert_eq!(ctx.this.label(), "country");
    assert!(ctx.root.unknown);
    assert!(ctx.prev_tail_unknown);
    assert_ne!(ctx.object, ctx.root_object);
    e.effects(nodes[0].body(), &ctx, &mut State::default());
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "scope-type-unknown"));
    assert_eq!(e.inferred_script_inputs[0]["caller_proven"], false);
    assert_eq!(
        e.inferred_script_inputs[0]["engine_entry_registered"],
        false
    );
    assert_eq!(e.inferred_script_inputs[0]["complete_contract"], false);
}
#[test]
fn inferred_input_recovers_wrong_previous_receiver_without_guessing_root() {
    let mut e = make();
    let nodes = ast(
        &mut e,
        "helper={set_country_flag=ok owner={prev={set_ship_flag=bad}}}",
    );
    let ctx = e.inferred_script_context(true, "helper", &nodes[0]);
    e.effects(nodes[0].body(), &ctx, &mut State::default());
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "scope" && d.message.contains("set_ship_flag")),
        "{:?}",
        e.diagnostics.rows
    );
}
#[test]
fn inferred_input_aliases_and_scope_entry_contracts_are_transitive() {
    let mut e = make();
    for n in ast(&mut e, "country_test={is_ai=no} alias={country_test=yes}") {
        e.program
            .scripts
            .insert((false, n.key.clone()), Arc::new(n));
    }
    for text in [
        "helper={alias=yes}",
        "helper={exists=capital_scope}",
        "helper={capital_scope={has_planet_flag=ok}}",
    ] {
        let nodes = ast(&mut e, text);
        let ctx = e.inferred_script_context(false, "helper", &nodes[0]);
        assert_eq!(ctx.this.label(), "country", "{text}");
        assert!(ctx.root.unknown);
    }
}
#[test]
fn inferred_input_conflicts_remain_unknown_instead_of_picking_one_type() {
    let mut e = make();
    let nodes = ast(&mut e, "helper={set_country_flag=ok set_ship_flag=bad}");
    let ctx = e.inferred_script_context(true, "helper", &nodes[0]);
    assert!(ctx.this.unknown);
    assert!(has(&e, "script-input-contract-conflict"));
    assert_eq!(
        e.inferred_script_inputs[0]["status"],
        "conflicting-input-constraints"
    );
}
#[test]
fn inferred_input_does_not_narrow_guarded_alternatives_or_foreign_frames() {
    let mut e = make();
    for text in [
        "helper={OR={AND={is_scope_type=country is_ai=no} AND={is_scope_type=ship is_ship_class=shipclass_science_ship}}}",
        "helper={root={set_country_flag=ok} from={set_ship_flag=ok} prev={set_planet_flag=ok}}",
        "helper={create_species={effect={set_species_flag=ok}}}",
    ] {
        let nodes = ast(&mut e, text);
        assert!(
            e.inferred_script_context(true, "helper", &nodes[0])
                .this
                .unknown,
            "{text}"
        );
    }
    assert!(e.inferred_script_inputs.is_empty());
}
#[test]
fn inferred_input_depth_limit_retains_unknown_and_reports_incompleteness() {
    let mut e = make();
    let text = (0..70)
        .map(|i| format!("s{i}={{s{}=yes}}", i + 1))
        .collect::<String>()
        + "s70={is_ai=no}";
    for n in ast(&mut e, &text) {
        e.program
            .scripts
            .insert((false, n.key.clone()), Arc::new(n));
    }
    let node = e.program.scripts[&(false, "s0".into())].clone();
    assert!(e.inferred_script_context(false, "s0", &node).this.unknown);
    assert!(!e.complete);
    assert!(has(&e, "analysis-limit"));
}
#[test]
fn inferred_input_preserves_engine_no_scope_variant() {
    let mut e = make();
    let nodes = ast(&mut e, "helper={spawn_system={initializer=anything}}");
    let ctx = e.inferred_script_context(true, "helper", &nodes[0]);
    assert!(ctx.this.known.contains("no_scope"), "{:?}", ctx.this);
    assert_eq!(
        ctx.this.known,
        e.rules.effects["spawn_system"]
            .input
            .iter()
            .cloned()
            .collect()
    );
    assert!(ctx.root.unknown);
}
#[test]
fn inferred_input_guard_scan_budget_is_not_silently_treated_as_a_guard() {
    let mut e = make();
    let text = "helper={".to_string() + &"log=ok ".repeat(4100) + "}";
    let nodes = ast(&mut e, &text);
    assert!(
        e.inferred_script_context(true, "helper", &nodes[0])
            .this
            .unknown
    );
    assert!(!e.complete);
    assert!(has(&e, "analysis-limit"));
    assert_eq!(e.inferred_script_inputs[0]["status"], "analysis-limit");
}
fn project_activation_fixture(text: &str, ctx: &Context) -> Engine {
    let mut e = make();
    e.program
        .entities
        .entry("special_projects".into())
        .or_default()
        .insert("P".into());
    let nodes = ast(&mut e, text);
    e.effects(&nodes, ctx, &mut State::default());
    e
}
#[test]
fn project_activation_owner_defaults_to_root_and_keeps_unknown_inputs() {
    for (ctx, text) in [
        (Context::new("country"), "enable_special_project={name=P}"),
        (
            Context::new("country").push(Types::new("planet"), "capital"),
            "enable_special_project={name=P location=this}",
        ),
        (
            Context::new("ship"),
            "enable_special_project={name=P owner=owner location=this}",
        ),
    ] {
        let e = project_activation_fixture(text, &ctx);
        assert!(
            e.diagnostics.rows.is_empty(),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
    let e = project_activation_fixture("enable_special_project={name=P}", &Context::new("ship"));
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "scope" && d.message.contains("default owner"))
    );
    let e = project_activation_fixture(
        "enable_special_project={name=P owner=this}",
        &Context::new("country").push(Types::new("ship"), "vessel"),
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "scope"));
    let mut ctx = Context::new("ship");
    ctx.root = Types::new("?");
    let e = project_activation_fixture("enable_special_project={name=P}", &ctx);
    assert_eq!(errors(&e), 0);
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.reason["code"] == "scope-type-unknown")
    );
}
#[test]
fn project_activation_checks_static_shape_and_project_references() {
    for (text, key) in [
        ("enable_special_project=yes", "structure"),
        ("enable_special_project={}", "missing-field"),
        ("enable_special_project={name={P}}", "structure"),
        ("enable_special_project={name=MISSING}", "missing-item"),
        ("enable_special_project={name=\"\"}", "missing-field"),
        ("enable_special_project={name=P owner={root}}", "structure"),
        (
            "enable_special_project={name=P location={root}}",
            "structure",
        ),
        (
            "enable_special_project={name=P owenr=root}",
            "unknown-field",
        ),
        ("enable_special_project={name=P key=P}", "unknown-field"),
        ("enable_special_project={name=P root=root}", "unknown-field"),
        (
            "enable_special_project={name=P location=root.bad_link}",
            "unknown-scope",
        ),
    ] {
        let e = project_activation_fixture(text, &Context::new("country"));
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
    let e = project_activation_fixture(
        "if={limit={always=no} enable_special_project={name=MISSING}}",
        &Context::new("ship"),
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
    assert!(!e.diagnostics.rows.iter().any(|d| d.key == "scope"));
}
#[test]
fn project_activation_keeps_unproved_targets_separate_from_name_validation() {
    let mut e = make();
    e.program
        .entities
        .entry("special_projects".into())
        .or_default()
        .insert("P".into());
    let nodes = ast(
        &mut e,
        "enable_special_project={name=P owner=event_target:t location=event_target:p}",
    );
    let mut state = optional_target("t", "country");
    state
        .bindings
        .extend(optional_target("p", "planet").bindings);
    let before = state.clone();
    e.effects(&nodes, &Context::new("country"), &mut state);
    assert_eq!(
        state, before,
        "activation must not execute future callbacks"
    );
    assert_eq!(errors(&e), 0);
    for target in ["event_target:t", "event_target:p"] {
        assert!(
            e.diagnostics
                .rows
                .iter()
                .any(|d| d.reason["code"] == "target-binding-unproven"
                    && d.message.ends_with(target))
        );
    }
    let e = project_activation_fixture(
        "enable_special_project={name=P@root}",
        &Context::new("country"),
    );
    assert_eq!(errors(&e), 0);
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.reason["code"] == "dynamic-reference")
    );
}
#[test]
fn named_scope_predicates_export_receiver_facts_only_on_success() {
    for name in [
        "event_target:t",
        "global_event_target:t",
        "event_target:t.THIS.this",
    ] {
        let mut e = make();
        let nodes = ast(&mut e, &format!("{name}={{is_scope_type=leader}}"));
        let mut state = State::default();
        let result = e
            .refine(&nodes, &Context::new("country"), &mut state, true)
            .unwrap();
        assert_eq!(result.this.label(), "country");
        let key = name.split('.').next().unwrap();
        assert_eq!(state.get(key).availability, Availability::Definite);
        assert_eq!(state.get(key).types.label(), "leader");
    }
    let mut e = make();
    let nodes = ast(&mut e, "event_target:t={is_scope_type=country}");
    let mut state = State::default();
    state.set(
        "global_event_target:t".into(),
        Binding::assigned(Types::new("country|planet")),
    );
    e.refine(&nodes, &Context::new("leader"), &mut state, true)
        .unwrap();
    assert_eq!(state.get("global_event_target:t").types.label(), "country");
    assert!(!state.bindings.contains_key("event_target:t"));
    state.set(
        "event_target:t".into(),
        Binding::assigned(Types::new("planet")),
    );
    assert!(
        e.refine(&nodes, &Context::new("country"), &mut state, true)
            .is_none()
    );
    assert_eq!(state.get("global_event_target:t").types.label(), "country");
}
#[test]
fn optional_named_scope_suffix_uses_the_same_binding_without_proving_presence() {
    let mut e = make();
    let nodes = ast(&mut e, "event_target:t?={save_event_target_as=copy}");
    let mut state = State::default();
    e.effects(&nodes, &Context::new("country"), &mut state);
    assert_eq!(
        state.get("event_target:copy").availability,
        Availability::Maybe
    );
    assert!(state.get("event_target:copy").types.unknown);
    assert!(!state.bindings.contains_key("event_target:t?"));
    assert!(
        !e.diagnostics
            .rows
            .iter()
            .any(|d| d.reason["code"] == "target-binding-unproven")
    );
    let mut state = State::default();
    state.set(
        "event_target:t".into(),
        Binding::assigned(Types::new("leader")),
    );
    e.effects(&nodes, &Context::new("country"), &mut state);
    assert_eq!(
        state.get("event_target:copy").availability,
        Availability::Definite
    );
    assert_eq!(state.get("event_target:copy").types.label(), "leader");
    let wrong = ast(&mut e, "event_target:t?={set_country_flag=wrong}");
    e.effects(&wrong, &Context::new("country"), &mut state);
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "scope" && d.message.contains("set_country_flag"))
    );
}
#[test]
fn optional_negative_and_other_object_predicates_do_not_invent_target_facts() {
    let mut e = make();
    let node = ast(&mut e, "who=event_target:t?").remove(0);
    e.resolve(
        node.scalar(),
        &Context::new("country"),
        &optional_target("t", "country"),
        &node,
        false,
    );
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.reason["code"] == "target-binding-unproven")
    );
    for guard in [
        "NOT={event_target:t={is_scope_type=country}}",
        "event_target:t? = {is_scope_type=country}",
        "event_target:t.owner={is_scope_type=country}",
        "event_target:t.ROOT={is_scope_type=country}",
        "event_target:t@root={is_scope_type=country}",
        "exists=event_target:t.owner",
        "exists=event_target:t@root",
    ] {
        let mut e = make();
        let nodes = ast(&mut e, guard);
        let mut state = State::default();
        e.refine(&nodes, &Context::new("country"), &mut state, true)
            .unwrap();
        assert!(state.bindings.is_empty(), "{guard}: {:?}", state.bindings);
    }
}
#[test]
fn named_scope_predicate_alternatives_join_receiver_types_and_availability() {
    let mut e = make();
    let nodes = ast(
        &mut e,
        "OR={event_target:t={is_scope_type=country} event_target:t={is_scope_type=planet}}",
    );
    let mut state = State::default();
    e.refine(&nodes, &Context::new("leader"), &mut state, true)
        .unwrap();
    assert_eq!(
        state.get("event_target:t").availability,
        Availability::Definite
    );
    assert_eq!(state.get("event_target:t").types.label(), "country|planet");
    let nodes = ast(
        &mut e,
        "OR={event_target:t={is_scope_type=country} event_target:u={is_scope_type=planet}}",
    );
    let mut state = State::default();
    e.refine(&nodes, &Context::new("leader"), &mut state, true)
        .unwrap();
    assert_eq!(
        state.get("event_target:t").availability,
        Availability::Maybe
    );
    assert_eq!(
        state.get("event_target:u").availability,
        Availability::Maybe
    );
    let nodes = ast(&mut e, "NOT={event_target:t={is_scope_type=planet}}");
    state.set(
        "event_target:t".into(),
        Binding::assigned(Types::new("country|planet")),
    );
    e.refine(&nodes, &Context::new("leader"), &mut state, true)
        .unwrap();
    assert_eq!(state.get("event_target:t").types.label(), "country");
}
#[test]
fn named_scope_predicate_reaches_effects_without_leaking_outside_guard() {
    let mut e = make();
    let nodes = ast(
        &mut e,
        "if={limit={event_target:t={is_scope_type=leader}} event_target:t={set_leader_flag=ok set_country_flag=wrong}} event_target:t={set_leader_flag=outside}",
    );
    e.effects(&nodes, &Context::new("country"), &mut State::default());
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "scope" && d.message.contains("set_country_flag"))
    );
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.reason["code"] == "target-binding-unproven"
                && d.message.ends_with("event_target:t"))
    );
    assert!(!e.diagnostics.rows.iter().any(|d| {
        d.message
            .contains("scope cannot be inferred for set_country_flag")
    }));
}
#[test]
fn nested_predicates_and_aliases_cannot_prove_their_own_receiver_reads() {
    for condition in [
        "AND={event_target:t={is_scope_type=leader}}",
        "OR={AND={event_target:t={is_scope_type=leader}} AND={event_target:t={is_scope_type=country}}}",
        "hidden_trigger={AND={event_target:t={is_scope_type=leader}}}",
        "NOT={NOT={event_target:t={is_scope_type=leader}}}",
        "alias_guard=yes",
    ] {
        let mut e = make();
        let def = ast(
            &mut e,
            "alias_guard={AND={event_target:t={is_scope_type=leader}}}",
        )
        .remove(0);
        e.program
            .scripts
            .insert((false, "alias_guard".into()), Arc::new(def));
        let nodes = ast(&mut e, condition);
        e.triggers(&nodes, &Context::new("country"), &State::default());
        assert!(
            e.diagnostics
                .rows
                .iter()
                .any(|d| d.reason["code"] == "target-binding-unproven"
                    && d.message.ends_with("event_target:t")),
            "{condition}: {:?}",
            e.diagnostics.rows
        );
    }
    let mut e = make();
    let nodes = ast(
        &mut e,
        "AND={exists=event_target:t event_target:t={is_scope_type=leader}}",
    );
    e.triggers(&nodes, &Context::new("country"), &State::default());
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}
fn parameter_fixture(text: &str, scope: &str, effect: bool) -> Engine {
    let mut e = make();
    for (category, keys) in [
        ("strategic_resources", vec!["alloys", "nanites"]),
        ("governments/civics", vec!["civic_ok"]),
        ("governments/authorities", vec!["auth_ok"]),
        ("traits", vec!["leader_trait_ok", "trait_ok"]),
        ("leader_tiers", vec!["tier_ok"]),
        ("planet_classes", vec!["pc_ok", "custom_world"]),
        ("ethics", vec!["ethic_ok"]),
        ("flag_colors", vec!["red", "blue"]),
    ] {
        e.program
            .entities
            .entry(category.into())
            .or_default()
            .extend(keys.into_iter().map(String::from));
    }
    e.program.loc.entry("english".into()).or_default().extend([
        "TT".into(),
        "MSG".into(),
        "NAME".into(),
    ]);
    let events = ast(&mut e, "country_event={id=a}");
    e.program.index(File {
        path: "base-events.txt".into(),
        category: "events".into(),
        ast: events.into_iter().map(Arc::new).collect(),
        target: false,
    });
    let nodes = ast(&mut e, text);
    if effect {
        e.effects(&nodes, &Context::new(scope), &mut State::default());
    } else {
        e.triggers(&nodes, &Context::new(scope), &State::default());
    }
    e
}
fn optional_target(name: &str, scope: &str) -> State {
    let mut state = State::default();
    state.set(
        format!("event_target:{name}"),
        Binding::assigned(Types::new(scope)).join(&Binding::default()),
    );
    state
}
#[test]
fn composite_guards_preserve_conjunctions_without_borrowing_or_facts() {
    let mut e = make();
    for (text, scope, definite) in [
        (
            "exists=event_target:t is_scope_type=country",
            "country",
            true,
        ),
        (
            "AND={exists=event_target:t is_scope_type=country}",
            "country",
            true,
        ),
        (
            "OR={AND={exists=event_target:t is_scope_type=country} AND={exists=event_target:t is_scope_type=planet}}",
            "country|planet",
            true,
        ),
        (
            "OR={exists=event_target:t is_scope_type=planet}",
            "country|planet|ship",
            false,
        ),
    ] {
        let nodes = ast(&mut e, text);
        let mut state = optional_target("t", "country");
        let frame = e
            .refine(
                &nodes,
                &Context::new("country|planet|ship"),
                &mut state,
                true,
            )
            .unwrap();
        assert_eq!(frame.this.label(), scope, "{text}");
        assert_eq!(
            state.get("event_target:t").availability == Availability::Definite,
            definite,
            "{text}"
        );
    }
    let nodes = ast(&mut e, "OR={is_scope_type=planet is_scope_type=country}");
    let frame = e
        .refine(
            &nodes,
            &Context::new("country|planet|ship"),
            &mut State::default(),
            false,
        )
        .unwrap();
    assert_eq!(frame.this.label(), "ship");
}
#[test]
fn guard_truth_tables_do_not_exclude_any_satisfying_world() {
    fn evaluate(nodes: &[Node], scope: &str, x: bool, y: bool, z: bool) -> bool {
        nodes
            .iter()
            .all(|n| match n.key.to_ascii_lowercase().as_str() {
                "is_scope_type" => n.scalar() == scope,
                "exists" => {
                    if n.scalar() == "event_target:x" {
                        x
                    } else {
                        y
                    }
                }
                "always" => n.scalar() == "yes",
                "has_country_flag" => z,
                "and" | "hidden_trigger" => evaluate(n.body(), scope, x, y, z),
                "or" => n
                    .body()
                    .iter()
                    .any(|a| evaluate(std::slice::from_ref(a), scope, x, y, z)),
                "nor" => !n
                    .body()
                    .iter()
                    .any(|a| evaluate(std::slice::from_ref(a), scope, x, y, z)),
                "nand" | "not" => !evaluate(n.body(), scope, x, y, z),
                _ => panic!("unexpected oracle predicate"),
            })
    }
    let mut e = make();
    for text in [
        "exists=event_target:x is_scope_type=country",
        "AND={exists=event_target:x has_country_flag=z}",
        "OR={exists=event_target:x has_country_flag=z}",
        "OR={AND={exists=event_target:x is_scope_type=planet} AND={exists=event_target:x is_scope_type=country}}",
        "NAND={exists=event_target:x is_scope_type=planet}",
        "NOR={is_scope_type=country is_scope_type=planet}",
        "NOT={OR={exists=event_target:x exists=event_target:y}}",
        "NOT={AND={exists=event_target:x is_scope_type=ship}}",
        "OR={always=no exists=event_target:y}",
        "AND={always=yes OR={is_scope_type=planet exists=event_target:x}}",
        "AND={exists=event_target:x NOT={exists=event_target:y}}",
    ] {
        let nodes = ast(&mut e, text);
        for sign in [false, true] {
            let mut state = optional_target("x", "country");
            state
                .bindings
                .extend(optional_target("y", "planet").bindings);
            let refined = e.refine(
                &nodes,
                &Context::new("country|planet|ship"),
                &mut state,
                sign,
            );
            for scope in ["country", "planet", "ship"] {
                for x in [false, true] {
                    for y in [false, true] {
                        for z in [false, true] {
                            if evaluate(&nodes, scope, x, y, z) != sign {
                                continue;
                            }
                            let frame = refined.as_ref().unwrap_or_else(|| {
                                panic!(
                                    "excluded satisfying world: {text}={sign} {scope} {x}/{y}/{z}"
                                )
                            });
                            assert!(
                                frame.this.unknown || frame.this.known.contains(scope),
                                "{text}={sign} excluded {scope}"
                            );
                            for (name, exists) in [("x", x), ("y", y)] {
                                assert!(
                                    state.get(&format!("event_target:{name}")).availability
                                        != Availability::Definite
                                        || exists,
                                    "{text}={sign} invented existence of {name}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn scripted_guards_refine_aliases_but_bound_cycles_and_diamonds() {
    let mut e = make();
    let defs = ast(
        &mut e,
        "guard={exists=event_target:t is_scope_type=country} alias_guard={guard=yes} inverse_guard={alias_guard=no} cycle_a={cycle_b=yes} cycle_b={cycle_a=yes}",
    );
    e.program.index(File {
        path: "guard-definitions.txt".into(),
        category: "scripted_triggers".into(),
        ast: defs.into_iter().map(Arc::new).collect(),
        target: false,
    });
    let nodes = ast(&mut e, "alias_guard=yes");
    let mut state = optional_target("t", "country");
    let frame = e
        .refine(&nodes, &Context::new("country|planet"), &mut state, true)
        .unwrap();
    assert_eq!(frame.this.label(), "country");
    assert_eq!(
        state.get("event_target:t").availability,
        Availability::Definite
    );
    let nodes = ast(&mut e, "inverse_guard=no");
    let mut state = optional_target("t", "country");
    let frame = e
        .refine(&nodes, &Context::new("country|planet"), &mut state, true)
        .unwrap();
    assert_eq!(frame.this.label(), "country");
    let nodes = ast(&mut e, "cycle_a=yes");
    let mut state = optional_target("t", "country");
    let frame = e
        .refine(&nodes, &Context::new("?"), &mut state, true)
        .unwrap();
    assert!(frame.this.unknown);
    assert_eq!(
        state.get("event_target:t").availability,
        Availability::Maybe
    );
    let mut text = String::from("diamond_10={exists=event_target:t}");
    for i in 0..10 {
        text.push_str(&format!(
            "diamond_{i}={{OR={{diamond_{}=yes diamond_{}=yes}}}}",
            i + 1,
            i + 1
        ));
    }
    let defs = ast(&mut e, &text);
    e.program.index(File {
        path: "diamonds.txt".into(),
        category: "scripted_triggers".into(),
        ast: defs.into_iter().map(Arc::new).collect(),
        target: false,
    });
    let nodes = ast(&mut e, "diamond_0=yes");
    let mut state = optional_target("t", "country");
    e.refine(&nodes, &Context::new("country"), &mut state, true)
        .unwrap();
    assert_eq!(
        state.get("event_target:t").availability,
        Availability::Maybe
    );
}
#[test]
fn branch_guards_reduce_false_unknowns_and_keep_wrong_scope_detection() {
    let mut e = make();
    let nodes = ast(
        &mut e,
        "if={limit={exists=event_target:t is_ai=no} event_target:t={set_country_flag=ok}} ",
    );
    e.effects(
        &nodes,
        &Context::new("country"),
        &mut optional_target("t", "country"),
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    let mut e = make();
    let nodes = ast(
        &mut e,
        "if={limit={exists=event_target:t is_ai=no} event_target:t={set_country_flag=wrong}} ",
    );
    e.effects(
        &nodes,
        &Context::new("country"),
        &mut optional_target("t", "planet"),
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "scope"));
    let mut e = make();
    let nodes = ast(
        &mut e,
        "if={limit={OR={exists=event_target:t is_ai=no}} event_target:t={set_country_flag=maybe}} event_target:t={set_country_flag=outside}",
    );
    e.effects(
        &nodes,
        &Context::new("country"),
        &mut optional_target("t", "country"),
    );
    assert!(has(&e, "target-binding-unproven"));
}
#[test]
fn impossible_type_arms_do_not_use_the_outer_receiver_but_keep_static_errors() {
    let mut e = make();
    let nodes = ast(
        &mut e,
        "OR={AND={is_scope_type=planet is_star=yes} AND={is_scope_type=country is_ai=no}}",
    );
    e.triggers(&nodes, &Context::new("country"), &State::default());
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    for (text, key) in [
        ("is_scope_type=unknown_type", "value"),
        ("is_scope_type={bad=yes}", "structure"),
        ("AND={is_scope_type=planet is_satr=yes}", "unknown-trigger"),
        ("AND={is_scope_type=planet is_star=maybe}", "value"),
        (
            "AND={is_scope_type=planet has_ethic=missing_ethic}",
            "missing-item",
        ),
        (
            "AND={is_scope_type=planet relative_power={who=this vlaue=superior}}",
            "unknown-field",
        ),
        (
            "AND={is_scope_type=planet has_resource={type=missing_resource amount=1}}",
            "missing-item",
        ),
        ("AND={is_scope_type=country is_star=yes}", "scope"),
    ] {
        let mut e = make();
        let nodes = ast(&mut e, text);
        e.triggers(&nodes, &Context::new("country"), &State::default());
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
    let mut e = make();
    let defs = ast(&mut e, "required_macro={has_country_flag=$FLAG$}");
    e.program.index(File {
        path: "macro-defs.txt".into(),
        category: "scripted_triggers".into(),
        ast: defs.into_iter().map(Arc::new).collect(),
        target: false,
    });
    let nodes = ast(&mut e, "AND={is_scope_type=planet required_macro=yes}");
    e.triggers(&nodes, &Context::new("country"), &State::default());
    assert!(
        e.diagnostics.rows.iter().any(|d| d.key == "macro-argument"),
        "{:?}",
        e.diagnostics.rows
    );
}
#[test]
fn dds_headers_check_lengths_sizes_dimensions_and_dx10_extension() {
    use crate::asset_contracts::dds_header_valid;
    let mut good = vec![0; 128];
    good[..4].copy_from_slice(b"DDS ");
    for (offset, value) in [(4, 124u32), (12, 16), (16, 16), (76, 32)] {
        good[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    assert!(dds_header_valid(&good));
    for length in [0, 4, 76, 127] {
        assert!(!dds_header_valid(&good[..length]));
    }
    for (offset, value) in [(4, 123u32), (12, 0), (16, 0), (76, 31)] {
        let mut bad = good.clone();
        bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        assert!(!dds_header_valid(&bad));
    }
    let mut bad = good.clone();
    bad[0] = b'X';
    assert!(!dds_header_valid(&bad));
    good[84..88].copy_from_slice(b"DX10");
    assert!(!dds_header_valid(&good));
    good.resize(148, 0);
    assert!(dds_header_valid(&good));
}
#[test]
fn changing_country_flags_keeps_its_own_color_and_scalar_contract() {
    for text in [
        "change_country_flag=random",
        "change_country_flag={colors={red blue}}",
        "change_country_flag={colors={red blue null null}}",
    ] {
        let e = parameter_fixture(text, "country", true);
        assert!(
            e.diagnostics.rows.is_empty(),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
    for (text, key) in [
        ("change_country_flag=this", "value"),
        ("change_country_flag={colros={red blue}}", "unknown-field"),
        ("change_country_flag={colors={red missing}}", "missing-item"),
        ("change_country_flag={colors={red=blue}}", "structure"),
        (
            "change_country_flag={background={category=special}}",
            "missing-field",
        ),
    ] {
        let e = parameter_fixture(text, "country", true);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
    let e = parameter_fixture("change_country_flag=random", "planet", true);
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "scope"));
    let e = parameter_fixture("change_country_flag={colors={red}}", "country", true);
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "engine-contract-unverified"));
}
#[test]
fn fleet_settings_validate_values_without_guessing_unlisted_names() {
    for (text, scope) in [
        (
            "set_fleet_settings={can_upgrade=no can_change_leader=no garrison=yes}",
            "fleet",
        ),
        (
            "create_fleet={settings={can_upgrade=no can_change_leader=no garrison=yes spawn_debris=no is_boss=yes} effect={set_fleet_flag=created}}",
            "country",
        ),
    ] {
        let e = parameter_fixture(text, scope, true);
        assert!(
            e.diagnostics.rows.is_empty(),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
    for (text, key) in [
        ("create_fleet={settings=yes}", "structure"),
        ("create_fleet={settings={is_boss=maybe}}", "value"),
        ("create_fleet={settings={garrison=maybe}}", "value"),
        ("set_fleet_settings={can_upgrade={always=yes}}", "value"),
        ("create_fleet={settings={yes}}", "structure"),
        ("create_fleet={effect={set_country_flag=wrong}}", "scope"),
    ] {
        let e = parameter_fixture(text, "country", true);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
    let e = parameter_fixture("set_fleet_settings={can_upgarde=no}", "fleet", true);
    assert!(has(&e, "engine-contract-unverified"));
    let e = parameter_fixture("set_fleet_settings={spawn_debris=no}", "fleet", true);
    assert!(has(&e, "engine-contract-unverified"));
}
#[test]
fn ambient_offsets_check_numeric_ranges_booleans_and_order() {
    let e = parameter_fixture(
        "create_ambient_object={type=fixture entity_offset={min=-100 max=10000} entity_offset_angle={min=0 max=360} entity_offset_height=-5 use_3d_location=yes is_wreck=no entity_scale_to_size=yes play_animation_once=no scale=1000 entity_face_object=this}",
        "country",
        true,
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    for (body, key) in [
        ("entity_offset={min=8 max=2}", "range-order"),
        ("entity_offset={mni=1 max=2}", "unknown-field"),
        ("entity_offset_height={min={bad=yes}}", "structure"),
        ("use_3d_location=maybe", "value"),
        ("scale={bad=yes}", "structure"),
    ] {
        let e = parameter_fixture(
            &format!("create_ambient_object={{type=fixture {body}}}"),
            "country",
            true,
        );
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{body}: {:?}",
            e.diagnostics.rows
        );
    }
    let e = parameter_fixture(
        "create_ambient_object={type=fixture entity_offset={min=runtime_min max=runtime_max}}",
        "country",
        true,
    );
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "numeric-information-unresolved"));
    let e = parameter_fixture(
        "create_ambient_object={type=fixture effect={save_event_target_as=ambient}}",
        "country",
        true,
    );
    assert!(has(&e, "rule-unimplemented"), "{:?}", e.diagnostics.rows);
}
#[test]
fn static_fleet_and_flag_parameter_checks_survive_unreachable_branches() {
    let e = parameter_fixture(
        "while={count=0 limit={always=yes} change_country_flag={colors={red missing}} create_fleet={settings={garrison=maybe}} create_ambient_object={type=fixture entity_offset={min=8 max=2}}}",
        "country",
        true,
    );
    for key in ["missing-item", "value", "range-order"] {
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{key}: {:?}",
            e.diagnostics.rows
        );
    }
}
#[test]
fn country_flags_check_colors_shapes_fields_and_source_scopes() {
    for text in [
        "create_country={flag=random}",
        "create_country={flag=none}",
        "create_country={flag=this}",
        "create_country={flag={colors={red blue null null} use_ship_color=yes}}",
    ] {
        let e = parameter_fixture(text, "country", true);
        assert!(
            e.diagnostics.rows.is_empty(),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
    for (text, key) in [
        (
            "create_country={flag={colros={red red null null}}}",
            "unknown-field",
        ),
        ("create_country={flag={colors={red red}}}", "structure"),
        (
            "create_country={flag={colors={red missing null null}}}",
            "missing-item",
        ),
        (
            "create_country={flag={colors={red red null={bad=yes} null}}}",
            "structure",
        ),
        (
            "create_country={flag={icon={category=special}}}",
            "missing-field",
        ),
        (
            "create_country={flag={icon={category=special file=missing.dds}}}",
            "missing-item",
        ),
        ("create_country={flag={use_ship_color=maybe}}", "value"),
    ] {
        let e = parameter_fixture(text, "country", true);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
    let e = parameter_fixture("create_country={flag=this}", "planet", true);
    assert!(
        e.diagnostics.rows.iter().any(|d| d.key == "scope"),
        "{:?}",
        e.diagnostics.rows
    );
}
#[test]
fn graphics_resource_paths_keep_missing_unsafe_and_dynamic_distinct() {
    for (text, key) in [
        (
            "spriteType={texturefile=\"gfx/missing.dds\"}",
            "missing-item",
        ),
        ("pdxmesh={file=\"../../outside.mesh\"}", "value"),
        (
            "spriteType={texturefile=\"gfx/$RUNTIME$.dds\"}",
            "validation-coverage",
        ),
    ] {
        let mut e = make();
        let node = ast(&mut e, text).remove(0);
        e.graphics_references(&node);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
    let mut e = make();
    let n = ast(&mut e, "picture_event_data={room=missing_room}").remove(0);
    e.picture_event_data(&n, &Context::new("country"), &State::default());
    assert!(
        e.diagnostics.rows.iter().any(|d| d.key == "missing-item"),
        "{:?}",
        e.diagnostics.rows
    );
}
#[test]
fn counted_triggers_and_tooltip_containers_validate_nested_grammar() {
    let good = "success_text={text=TT is_ai=no} custom_tooltip_fail={text=TT is_ai=no success_text={text=TT is_ai=no}} calc_true_if={amount>=2 is_ai=no always=yes} has_active_event={a}";
    let e = parameter_fixture(good, "country", false);
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    for (text, key) in [
        ("success_text={text=TT is_star=yes}", "scope"),
        (
            "custom_tooltip_fail={text=TT success_text={text=TT is_star=yes}}",
            "scope",
        ),
        ("AND={success_text=TT}", "structure"),
        ("custom_tooltip_fail={text=TT is_aai=no}", "unknown-trigger"),
        ("success_text={text={bad=yes} is_ai=no}", "structure"),
        ("calc_true_if={always=yes}", "missing-field"),
        ("calc_true_if={amount=1 is_star=yes}", "scope"),
        ("calc_true_if={amount=1 is_aai=yes}", "unknown-trigger"),
        ("has_active_event={missing}", "missing-event"),
        ("has_active_event={id=a}", "structure"),
        ("has_active_event=yes", "structure"),
    ] {
        let e = parameter_fixture(text, "country", false);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}
#[test]
fn counted_trigger_arms_do_not_borrow_each_others_scope_guard() {
    let e = parameter_fixture(
        "calc_true_if={amount=1 is_scope_type=planet is_star=yes}",
        "?",
        false,
    );
    assert!(has(&e, "scope-type-unknown"), "{:?}", e.diagnostics.rows);
    assert_eq!(errors(&e), 0);
    let e = parameter_fixture(
        "calc_true_if={amount=1 if={limit={is_ai=yes} is_ai=yes} else={is_ai=no}}",
        "country",
        false,
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    for text in [
        "while={count=0 limit={always=yes} success_text={text=TT is_ai=no}}",
        "while={count=0 limit={always=yes} success_text=TT}",
    ] {
        let e = parameter_fixture(text, "country", true);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "unknown-effect"),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}
#[test]
fn message_variables_distinguish_placeholder_literal_and_scope() {
    let e = parameter_fixture(
        "create_message={type=BYPASS_EXPLORED localization=MSG target=root days=30 variable={type=name localization=SYSTEM scope=this} variable={key=border value=GFX_border}}",
        "country",
        true,
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    for (text, key) in [
        (
            "create_message={variable={type=name localization=SYSTEM scpoe=this}}",
            "unknown-field",
        ),
        ("create_message={variable={key=border}}", "missing-field"),
        (
            "create_message={variable={type=name localization=SYSTEM scope=this.misspelled}}",
            "unknown-scope",
        ),
        (
            "create_message={variable={key=border value={x=y}}}",
            "structure",
        ),
        ("create_message=yes", "structure"),
        (
            "while={count=0 limit={always=yes} create_message={variable={key=border}}}",
            "missing-field",
        ),
    ] {
        let e = parameter_fixture(text, "country", true);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
    let e = parameter_fixture(
        "create_message={variable={type=undocumented localization=SYSTEM scope=this}}",
        "country",
        true,
    );
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "engine-contract-unverified"));
}
#[test]
fn resource_pairs_validate_resource_names_numeric_forms_and_caller() {
    let e = parameter_fixture(
        "add_resource={alloys=999999 nanites=1 mult=2}",
        "country",
        true,
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    for (text, scope, key) in [
        ("add_resource={allosy=1}", "country", "missing-item"),
        ("add_resource={alloys={bad=yes}}", "country", "structure"),
        ("add_resource={alloys=1}", "planet", "scope"),
        ("add_resource=yes", "country", "structure"),
    ] {
        let e = parameter_fixture(text, scope, true);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
    let e = parameter_fixture("add_resource={alloys=var:unresolved}", "country", true);
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "numeric-information-unresolved"));
    let e = parameter_fixture("add_resource={resource=alloys amount=3}", "country", true);
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "engine-contract-unverified"));
}
#[test]
fn government_civic_contracts_validate_refs_shapes_and_flags() {
    let e = parameter_fixture(
        "change_government={authority=auth_ok civics={civic=civic_ok civic=random} cooldown=no remove_invalid_civics=yes} change_government=random",
        "country",
        true,
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    for (text, key) in [
        ("change_government={authority=bad}", "missing-item"),
        ("change_government={civics={civic=bad}}", "missing-item"),
        (
            "change_government={civics={civci=civic_ok}}",
            "unknown-field",
        ),
        ("change_government={cooldown=maybe}", "value"),
        ("change_government={authority={bad=yes}}", "structure"),
        ("change_government=yes", "value"),
    ] {
        let e = parameter_fixture(text, "country", true);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}
#[test]
fn traits_support_noninteger_leader_levels_and_typed_species_fields() {
    let e = parameter_fixture(
        "create_leader={traits={0=leader_trait_ok non_integer=leader_trait_ok} tier=tier_ok} create_species={traits={trait=trait_ok random_traits ideal_planet_class=pc_ok}} create_species={traits=this} create_species={traits={ideal_planet_class=custom_world}}",
        "country",
        true,
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    for (text, key) in [
        (
            "create_leader={traits={0=leader_trait_missing}}",
            "missing-item",
        ),
        ("create_leader={traits={leader_trait_ok}}", "structure"),
        ("create_leader={traits={0={bad=yes}}}", "structure"),
        ("create_leader={tier=missing}", "missing-item"),
        ("create_species={traits={trait=missing}}", "missing-item"),
        ("create_species={traits={traait=trait_ok}}", "unknown-field"),
        ("create_species={traits={ideal_planet_class=this}}", "scope"),
    ] {
        let e = parameter_fixture(text, "country", true);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
    let e = parameter_fixture(
        "create_species={traits={ideal_planet_class=this}}",
        "species",
        true,
    );
    assert_eq!(errors(&e), 0);
    let e = parameter_fixture(
        "create_leader={traits={0=unknown_special_token}}",
        "country",
        true,
    );
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "engine-contract-unverified"));
}
#[test]
fn failure_callbacks_and_specimen_targets_retain_scope_and_conditional_state() {
    let e = parameter_fixture(
        "add_random_research_option={fail_effects={set_country_flag=ok}} give_specimen={targets={this root}}",
        "country",
        true,
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    let e = parameter_fixture(
        "add_random_research_option={fail_effects={set_planet_flag=bad}}",
        "country",
        true,
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "scope"));
    let e = parameter_fixture(
        "add_random_research_option={fail_effects={save_event_target_as=f}} event_target:f={set_country_flag=ok}",
        "country",
        true,
    );
    assert!(
        has(&e, "target-binding-unproven"),
        "{:?}",
        e.diagnostics.rows
    );
    for (text, key) in [
        ("give_specimen={targets={target=this}}", "structure"),
        ("give_specimen={targets={this.misspelled}}", "unknown-scope"),
    ] {
        let e = parameter_fixture(text, "country", true);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{:?}",
            e.diagnostics.rows
        );
    }
}
#[test]
fn country_creation_ethics_civics_and_names_use_their_own_contracts() {
    let e = parameter_fixture(
        "create_country={name={key=NAME variable_string=x} ethos={ethic=ethic_ok} civics={civic=civic_ok}} create_country={ethos=this civics=this}",
        "country",
        true,
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    for (text, scope, key) in [
        (
            "create_country={ethos={ethiic=ethic_ok}}",
            "country",
            "unknown-field",
        ),
        (
            "create_country={ethos={ethic=bad}}",
            "country",
            "missing-item",
        ),
        ("create_country={ethos=this}", "planet", "scope"),
        ("create_country={civics=this}", "planet", "scope"),
        (
            "create_country={name={variable_string=x}}",
            "country",
            "missing-field",
        ),
    ] {
        let e = parameter_fixture(text, scope, true);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}
#[test]
fn compact_ast_retains_empty_block_null_scalar_and_nested_json() {
    for text in [
        r#"{"key":"x","value":"literal","line":3,"column":7,"op":"="}"#,
        r#"{"key":"x","children":[],"line":3,"column":7,"op":"="}"#,
        r#"{"key":"x","children":null,"line":3,"column":7,"op":"="}"#,
        r#"{"key":"x","source":"spoofed","id":999}"#,
        r#"{"key":"x","children":[{"key":"n","value":"1","op":">"}],"macro_variables":{"v":"2"}}"#,
    ] {
        let n: Node = serde_json::from_str(text).unwrap();
        assert!(n.source.is_empty());
        assert_eq!(n.id, 0);
        let encoded = serde_json::to_value(&n).unwrap();
        let clone = serde_json::to_value(n.clone()).unwrap();
        assert_eq!(encoded, clone);
        let restored: Node = serde_json::from_value(encoded.clone()).unwrap();
        assert_eq!(encoded, serde_json::to_value(restored).unwrap());
        let input: serde_json::Value = serde_json::from_str(text).unwrap();
        if input["children"].is_array() {
            assert!(n.children.is_some());
        } else {
            assert!(n.children.is_none());
        }
        assert_eq!(n.line, input["line"].as_u64().unwrap_or(0) as u32);
    }
}
#[test]
fn compact_ast_operator_keeps_all_spellings_and_signatures() {
    for op in [
        "",
        "=",
        "!=",
        "<",
        "<=",
        ">",
        ">=",
        "?=",
        "==",
        "unknown-运算符",
        "escaped\\value",
    ] {
        let input = serde_json::json!({"key":"x","op":op,"value":"v"});
        let n: Node = serde_json::from_value(input).unwrap();
        assert!(n.op == op);
        assert_eq!(serde_json::to_value(&n.op).unwrap(), op);
        assert_eq!(n.signature(), format!("x{op}\"v\""));
    }
}
#[test]
fn compact_ast_macro_metadata_keeps_objects_empty_parts_and_values() {
    for text in [
        r#"{"macro_variables":{}}"#,
        r#"{"macro_variables":{"a":"1","b":"two"},"macro_parts":[]}"#,
        r#"{"macro_parts":[{"parameter":true,"text":"NAME"}]}"#,
    ] {
        let node: Node = serde_json::from_str(text).unwrap();
        let output = serde_json::to_value(&node).unwrap();
        let input: serde_json::Value = serde_json::from_str(text).unwrap();
        if let Some(vars) = input.get("macro_variables") {
            assert_eq!(output["macro_variables"], *vars);
        }
        if let Some(parts) = input.get("macro_parts") {
            assert_eq!(output["macro_parts"], *parts);
        }
        assert_eq!(serde_json::to_value(node.clone()).unwrap(), output);
    }
    assert!(serde_json::from_str::<Node>(r#"{"macro_variables":null}"#).is_err());
}
fn display_event(text: &str) -> Engine {
    let mut e = make();
    for (kind, name) in [
        ("gfx_spritetype", "GFX_good"),
        ("event_chains", "good_chain"),
        ("specimens", "good_specimen"),
    ] {
        e.program
            .entities
            .entry(kind.into())
            .or_default()
            .insert(name.into());
    }
    let body = ast(&mut e, text);
    e.event_body(&body[0], &Context::new("country"), &mut State::default());
    e
}
#[test]
fn description_conditions_switches_and_nested_groups_are_checked() {
    let e = display_event(
        "country_event={id=a desc={if={limit={is_ai=no} text=loc} else_if={limit={is_ai=yes} first_valid={desc=loc}} else={text=loc} switch={trigger=is_ai yes=loc no={text=loc} default=loc}}}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(!has(&e, "rule-unimplemented"));
    for text in [
        "desc={trigger={is_star=yes} text=loc}",
        "desc={switch={trigger=is_star yes=loc default=loc}}",
        "desc={if={limit={is_star=yes} text=loc}}",
    ] {
        let e = display_event(&format!("country_event={{id=a {text}}}"));
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "scope"),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}
#[test]
fn description_malformed_fields_and_chains_are_errors() {
    for (body, key) in [
        ("desc={txet=loc}", "unknown-field"),
        ("desc={if={text=loc}}", "missing-field"),
        ("desc={else={text=loc}}", "structure"),
        ("desc={switch={yes=loc}}", "missing-field"),
        (
            "desc={switch={trigger=not_a_trigger yes=loc}}",
            "unknown-trigger",
        ),
        ("desc={text={wrong=yes}}", "structure"),
        ("desc={trigger=yes text=loc}", "structure"),
    ] {
        let e = display_event(&format!("country_event={{id=a {body}}}"));
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{body}: {:?}",
            e.diagnostics.rows
        );
    }
}
#[test]
fn description_impossible_branches_still_check_shape_without_false_scope_errors() {
    let e = display_event(
        "country_event={id=a desc={if={limit={is_scope_type=planet} trigger={is_star=yes} text=loc} else={text=loc}}}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    let e = display_event("country_event={id=a desc={if={limit={is_scope_type=planet} txet=loc}}}");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "unknown-field"));
}
#[test]
fn description_switch_values_use_selector_contract() {
    let e =
        display_event("country_event={id=a desc={switch={trigger=is_ai maybe=loc default=loc}}}");
    assert!(
        e.diagnostics.rows.iter().any(|d| d.key == "value"),
        "{:?}",
        e.diagnostics.rows
    );
}
#[test]
fn conditional_pictures_and_option_icons_check_symbols_and_scope() {
    let e = display_event(
        "country_event={id=a picture={trigger={is_ai=no} picture=GFX_good} option={name=loc icon={icon=GFX_good icon_background=GFX_good text=loc trigger={is_ai=no}}}}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    for (body, key) in [
        ("picture=GFX_missing", "missing-item"),
        ("picture={trigger={is_star=yes} picture=GFX_good}", "scope"),
        ("picture={picutre=GFX_good}", "unknown-field"),
        ("option={icon={icon_background={x=y}}}", "structure"),
        (
            "option={icon={trigger={is_star=yes} icon=GFX_good}}",
            "scope",
        ),
    ] {
        let e = display_event(&format!("country_event={{id=a {body}}}"));
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{body}: {:?}",
            e.diagnostics.rows
        );
    }
}
#[test]
fn event_metadata_checks_refs_flags_and_scalar_shapes() {
    let e = display_event(
        "country_event={id=a event_chain=good_chain specimen=good_specimen archaeology=yes force_open=no auto_select=yes desc_clear=yes}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(!has(&e, "rule-unimplemented"));
    for (body, key) in [
        ("event_chain=bad", "missing-item"),
        ("specimen=bad", "missing-item"),
        ("force_open=maybe", "value"),
        ("desc_clear=maybe", "value"),
        ("show_sound={bad=yes}", "structure"),
        ("trigger=yes", "structure"),
    ] {
        let e = display_event(&format!("country_event={{id=a {body}}}"));
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{body}: {:?}",
            e.diagnostics.rows
        );
    }
}
fn ast(e: &mut Engine, text: &str) -> Vec<Node> {
    let mut nodes = e.parser.parse("fixture.txt", text, "").unwrap().ast;
    for n in &mut nodes {
        n.locate("fixture.txt", &mut e.program.next_id);
    }
    nodes
}
fn effect(text: &str) -> Engine {
    let mut e = make();
    let body = ast(&mut e, text);
    e.effects(&body, &Context::new("country"), &mut State::default());
    e
}
fn errors(e: &Engine) -> usize {
    e.diagnostics
        .rows
        .iter()
        .filter(|d| d.severity == "error")
        .count()
}
fn has(e: &Engine, reason: &str) -> bool {
    e.diagnostics
        .rows
        .iter()
        .any(|d| d.reason["code"] == reason)
}
fn return_contracts() -> Engine {
    let mut e = make();
    let defs = ast(
        &mut e,
        "arbitrary_name={optimize_memory count_owned_pop_amount={}} alias={arbitrary_name=yes} threshold={count_owned_pop_amount={count>5}} truth={is_ai=no} recursive_a={recursive_b=yes} recursive_b={recursive_a=yes}",
    );
    for n in defs {
        e.program
            .scripts
            .insert((false, n.key.clone()), Arc::new(n));
    }
    e
}
#[test]
fn count_script_return_uses_known_iterator_contract_not_script_name() {
    let mut e = return_contracts();
    let body = ast(&mut e, "arbitrary_name>=2000 alias=0");
    e.triggers(&body, &Context::new("planet"), &State::default());
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    let body = ast(&mut e, "arbitrary_name>=NaN");
    e.triggers(&body, &Context::new("planet"), &State::default());
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "value"));
    let mut e = return_contracts();
    let body = ast(&mut e, "arbitrary_name=0");
    e.triggers(&body, &Context::new("species"), &State::default());
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "scope"));
}
#[test]
fn boolean_and_unknown_script_returns_remain_distinct() {
    for text in ["truth=5", "threshold=5", "arbitrary_name={}"] {
        let mut e = return_contracts();
        let body = ast(&mut e, text);
        e.triggers(&body, &Context::new("country"), &State::default());
        assert!(errors(&e) > 0, "{text}: {:?}", e.diagnostics.rows);
    }
    let mut e = return_contracts();
    let body = ast(&mut e, "arbitrary_name>=runtime_value recursive_a>=5");
    e.triggers(&body, &Context::new("planet"), &State::default());
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "numeric-information-unresolved"));
    assert!(has(&e, "analysis-limit"));
    assert!(!e.complete);
}
#[test]
fn fleet_action_dsl_validates_ids_scopes_and_nested_commands() {
    let mut e = make();
    let body = ast(
        &mut e,
        "queue_actions={move_to=solar_system wait={duration=3 random=2} repeat={max_iterations=3 while={id=condition always=yes} effect={id=callback set_fleet_flag=ok}} find_closest_planet={trigger={id=selection is_star=no} found_planet={move_to=this}}}",
    );
    e.effects(&body, &Context::new("fleet"), &mut State::default());
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    for (text, key) in [
        (
            "queue_actions={effect={set_fleet_flag=ok}}",
            "missing-field",
        ),
        (
            "queue_actions={effect={id=a} effect={id=a}}",
            "duplicate-field",
        ),
        (
            "queue_actions={effect={id=a set_country_flag=bad}}",
            "scope",
        ),
        ("queue_actions={set_fleet_flag=bad}", "structure"),
        ("queue_actions={wait={duration=1 typo=3}}", "unknown-field"),
        ("queue_actions={orbit_planet=solar_system}", "scope"),
        ("id=not_fleet_metadata", "unknown-effect"),
    ] {
        let mut e = make();
        let body = ast(&mut e, text);
        e.effects(&body, &Context::new("fleet"), &mut State::default());
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}
#[test]
fn queued_callback_writes_do_not_establish_immediate_caller_bindings() {
    let mut e = make();
    let body = ast(
        &mut e,
        "queue_actions={effect={id=deferred save_event_target_as=t}} event_target:t={set_fleet_flag=ok}",
    );
    let mut state = State::default();
    e.effects(&body, &Context::new("fleet"), &mut state);
    assert_eq!(
        state.get("event_target:t").availability,
        Availability::Absent
    );
    assert!(has(&e, "target-binding-unproven"));
    assert_eq!(errors(&e), 0);
}
#[test]
fn component_weights_are_country_while_potential_is_design() {
    let good = declaration(
        "component_templates",
        "utility_component_template={key=A potential={FROM={is_ai=no}} ai_weight={weight=1 modifier={factor=0 is_ai=no}}}",
    );
    assert!(
        good.diagnostics.rows.is_empty(),
        "{:?}",
        good.diagnostics.rows
    );
    for text in [
        "utility_component_template={key=A ai_weight={modifier={factor=1 is_star=yes}}}",
        "utility_component_template={key=A potential={is_ai=no}}",
    ] {
        assert!(
            declaration("component_templates", text)
                .diagnostics
                .rows
                .iter()
                .any(|d| d.key == "scope")
        );
    }
}
#[test]
fn unknown_previous_stack_is_distinct_from_known_empty_stack() {
    let mut e = make();
    let body = ast(&mut e, "prev={set_country_flag=ok}");
    e.effects(&body, &Context::unknown_caller(), &mut State::default());
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "caller-context-unknown"));
    let mut e = make();
    let body = ast(&mut e, "prev={set_country_flag=bad}");
    e.effects(&body, &Context::new("country"), &mut State::default());
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "scope" && d.message.contains("does not exist"))
    );
    assert!(
        effect("capital_scope={prev={set_country_flag=ok}}")
            .diagnostics
            .rows
            .is_empty()
    );
}
#[test]
fn uninvoked_script_does_not_invent_absent_prev_but_called_script_checks_it() {
    for called in [false, true] {
        let mut e = make();
        let script = ast(&mut e, "helper={prev={set_country_flag=ok}}");
        e.program.index(File {
            path: "fixture.txt".into(),
            category: "scripted_effects".into(),
            ast: script.into_iter().map(Arc::new).collect(),
            target: true,
        });
        if called {
            let event = ast(&mut e, "country_event={id=a immediate={helper=yes}}");
            e.program.index(File {
                path: "event.txt".into(),
                category: "events".into(),
                ast: event.into_iter().map(Arc::new).collect(),
                target: true,
            });
        }
        e.analyze();
        if called {
            assert!(
                e.diagnostics
                    .rows
                    .iter()
                    .any(|d| d.key == "scope" && d.message.contains("does not exist"))
            );
        } else {
            assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
            assert!(has(&e, "caller-context-unknown"));
        }
    }
}
#[test]
fn inline_failure_retains_origin_and_indexing_target_status() {
    for target in [false, true] {
        let mut e = make();
        e.program.inline.insert(
            "outer".into(),
            (
                "outer.txt".into(),
                "inline_script={script=inner PROVIDED=yes}".into(),
            ),
        );
        e.program.inline.insert(
            "inner".into(),
            (
                "inner.txt".into(),
                "country_event={id=a immediate={set_country_flag=$MISSING$}}".into(),
            ),
        );
        let roots = ast(&mut e, "inline_script={script=outer}");
        e.program.index(File {
            path: "fixture.txt".into(),
            category: "events".into(),
            ast: roots.into_iter().map(Arc::new).collect(),
            target,
        });
        e.prepare_templates();
        let row = e
            .diagnostics
            .rows
            .iter()
            .find(|d| d.key == "macro-argument")
            .unwrap();
        assert_eq!(row.call_chain[0]["target"], target);
        assert_eq!(row.call_chain[1]["name"], "outer");
        assert_eq!(row.call_chain[2]["name"], "inner");
        assert!(e.chain.is_empty());
    }
}
#[test]
fn indexing_generated_base_events_defers_body_errors_until_reached() {
    for (target, called, invalid) in [
        (false, false, false),
        (false, true, true),
        (true, false, true),
    ] {
        let mut e = make();
        e.program.inline.insert("generator".into(),("generator.txt".into(),"country_event={id=generated is_triggered_only=yes immediate={inline_script={script=bad_inner}}}".into()));
        e.program.inline.insert(
            "bad_inner".into(),
            ("bad_inner.txt".into(), "set_country_flag=$MISSING$".into()),
        );
        let roots = ast(&mut e, "inline_script={script=generator}");
        e.program.index(File {
            path: "base.txt".into(),
            category: "events".into(),
            ast: roots.into_iter().map(Arc::new).collect(),
            target,
        });
        let body = ast(
            &mut e,
            if called {
                "country_event={id=mod immediate={country_event={id=generated}}}"
            } else {
                "country_event={id=mod immediate={set_country_flag=ok}}"
            },
        );
        e.program.index(File {
            path: "mod.txt".into(),
            category: "events".into(),
            ast: body.into_iter().map(Arc::new).collect(),
            target: true,
        });
        e.analyze();
        assert!(e.program.events.contains_key("generated"));
        assert_eq!(
            e.diagnostics.rows.iter().any(|d| d.key == "macro-argument"),
            invalid,
            "{target}/{called}: {:?}",
            e.diagnostics.rows
        );
        if !invalid {
            assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
        }
    }
}
#[test]
fn generated_event_header_templates_still_supply_index_ids() {
    let mut e = make();
    e.program.inline.insert("generator".into(),("generator.txt".into(),"country_event={inline_script=header is_triggered_only=yes immediate={set_country_flag=ok}}".into()));
    e.program.inline.insert(
        "header".into(),
        ("header.txt".into(), "id=generated".into()),
    );
    let roots = ast(&mut e, "inline_script=generator");
    e.program.index(File {
        path: "base.txt".into(),
        category: "events".into(),
        ast: roots.into_iter().map(Arc::new).collect(),
        target: false,
    });
    let roots = ast(
        &mut e,
        "country_event={id=mod immediate={country_event={id=generated}}}",
    );
    e.program.index(File {
        path: "mod.txt".into(),
        category: "events".into(),
        ast: roots.into_iter().map(Arc::new).collect(),
        target: true,
    });
    e.analyze();
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    assert!(e.visited_events.contains("generated"));
}
#[test]
fn native_memory_directive_keeps_typo_detection() {
    assert!(
        effect("optimize_memory set_country_flag=ok")
            .diagnostics
            .rows
            .is_empty()
    );
    let mut e = make();
    let body = ast(&mut e, "optimize_memory is_ai=no");
    e.triggers(&body, &Context::new("country"), &State::default());
    assert!(e.diagnostics.rows.is_empty());
    assert!(
        effect("optmize_memory set_country_flag=ok")
            .diagnostics
            .rows
            .iter()
            .any(|d| d.key == "unknown-effect")
    );
}
#[test]
fn weighted_selector_uses_selected_scope() {
    let good = effect(
        "random_owned_colony={weights={base=10 modifier={factor=0.1 owner={is_ai=no}}} set_planet_flag=ok}",
    );
    assert!(
        good.diagnostics.rows.is_empty(),
        "{:?}",
        good.diagnostics.rows
    );
    let bad = effect(
        "random_owned_colony={weights={base=10 modifier={factor=0.1 is_ai=no}} set_planet_flag=ok}",
    );
    assert!(bad.diagnostics.rows.iter().any(|d| d.key == "scope"));
    assert!(
        !bad.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "unknown-effect")
    );
}
#[test]
fn or_arm_keeps_if_else_chain_without_cross_arm_narrowing() {
    let mut e = make();
    let body = ast(
        &mut e,
        "OR={if={limit={is_ai=yes} has_country_flag=a} else={has_country_flag=b} always=yes}",
    );
    e.triggers(&body, &Context::new("country"), &State::default());
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    let bad = ast(&mut e, "OR={else={always=yes} always=no}");
    e.triggers(&bad, &Context::new("country"), &State::default());
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "structure"));
}
#[test]
fn opinion_modifier_reference_uses_its_own_category() {
    let mut e = make();
    e.program
        .entities
        .entry("opinion_modifiers".into())
        .or_default()
        .insert("known".into());
    let good = ast(&mut e, "add_opinion_modifier={who=this modifier=known}");
    e.effects(&good, &Context::new("country"), &mut State::default());
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    let bad = ast(&mut e, "add_opinion_modifier={who=this modifier=missing}");
    e.effects(&bad, &Context::new("country"), &mut State::default());
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "missing-item" && d.message.contains("opinion_modifiers"))
    );
}
#[test]
fn macro_cache_is_not_disabled_by_unrelated_diagnostics() {
    let mut e = make();
    let defs = ast(&mut e, "save_named={save_event_target_as=$NAME$}");
    e.program
        .scripts
        .insert((true, "save_named".into()), Arc::new(defs[0].clone()));
    e.gap(
        &defs[0],
        "unrelated unresolved context",
        "caller-context-unknown",
        None,
    );
    let body = ast(&mut e, "save_named={NAME=t} save_named={NAME=t}");
    e.effects(&body, &Context::new("country"), &mut State::default());
    assert_eq!(e.metrics.script_body_compilations, 1);
    assert_eq!(e.metrics.script_body_cache_hits, 1);
    assert!(has(&e, "caller-context-unknown"));
    assert_eq!(errors(&e), 0);
}
#[test]
fn macro_cache_never_hides_expansion_syntax_diagnostics() {
    let mut e = make();
    let def = Node {
        key: "broken".into(),
        id: 90,
        source: "broken.txt".into(),
        line: 1,
        macro_parts: Some(
            vec![crate::ast::MacroPart {
                parameter: false,
                text: "set_country_flag = { $NAME$".into(),
            }]
            .into_boxed_slice(),
        ),
        ..Node::default()
    };
    e.program
        .scripts
        .insert((true, "broken".into()), Arc::new(def));
    let body = ast(&mut e, "broken={NAME=t} broken={NAME=t}");
    e.effects(&body, &Context::new("country"), &mut State::default());
    assert!(errors(&e) > 0);
    assert_eq!(e.metrics.script_body_compilations, 2);
    assert_eq!(e.metrics.script_body_cache_hits, 0);
}
#[test]
fn native_end_fleet_contract_block_does_not_weaken_scalar_triggers() {
    let mut e = make();
    let body = ast(
        &mut e,
        "end_fleet_contract={initiator=owner reason=cancelled}",
    );
    e.effects(&body, &Context::new("fleet"), &mut State::default());
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    let body = ast(
        &mut e,
        "end_fleet_contract={initiator={is_ai=no} reason=cancelled}",
    );
    e.effects(&body, &Context::new("fleet"), &mut State::default());
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "structure"));
}

fn declaration(category: &str, text: &str) -> Engine {
    let mut e = make();
    let nodes = ast(&mut e, text);
    for n in nodes {
        e.definition(category, &n);
    }
    e
}
#[test]
fn job_pretriggers_validate_their_own_boolean_grammar() {
    let e = declaration(
        "pop_jobs",
        "job={possible_pre_triggers={has_owner=yes is_enslaved=no is_being_purged=no is_being_assimilated=no has_planet=yes is_sapient=yes is_robotic=no}}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    for (body, key) in [
        ("possible_pre_triggers=yes", "structure"),
        ("possible_pre_triggers={is_robotic=maybe}", "value"),
        ("possible_pre_triggers={is_robotci=no}", "unknown-field"),
        ("possible_pre_triggers={has_owner={always=yes}}", "value"),
    ] {
        let e = declaration("pop_jobs", &format!("job={{{body}}}"));
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{body}: {:?}",
            e.diagnostics.rows
        );
    }
}
#[test]
fn job_swaps_validate_planet_conditions_and_nested_contracts() {
    let text = "job={swappable_data={default={name=job_name condition_string=GENERATED_JOB_LABEL} swap_type={trigger={has_planet_flag=eligible owner={is_ai=no}} potential={has_planet_flag=eligible} weight={base=100000 modifier={factor=2 always=yes}}}}}";
    let e = declaration("pop_jobs", text);
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    for (body, key) in [
        ("swappable_data=yes", "structure"),
        ("swappable_data={default=yes}", "structure"),
        ("swappable_data={swap_type={trigger={is_ai=no}}}", "scope"),
        (
            "swappable_data={default={buidling_icon=bad}}",
            "unknown-field",
        ),
        (
            "swappable_data={default={building_icon=missing}}",
            "missing-item",
        ),
        ("swappable_data={swap_type={trigger=yes}}", "structure"),
        ("swappable_data={default={weight=NaN}}", "value"),
    ] {
        let e = declaration("pop_jobs", &format!("job={{{body}}}"));
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{body}: {:?}",
            e.diagnostics.rows
        );
    }
    let mut e = make();
    e.program
        .entities
        .entry("buildings".into())
        .or_default()
        .insert("known_building".into());
    let n = ast(
        &mut e,
        "job={swappable_data={default={building_icon=known_building}}}",
    )
    .remove(0);
    e.definition("pop_jobs", &n);
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}
#[test]
fn megastructure_placement_and_upgrade_contexts_keep_scope_stacks() {
    let e = declaration(
        "megastructures",
        "mega={entity_offset={x=-7 y=8 z=0} potential={is_ai=no from={if={limit={is_scope_type=megastructure} has_megastructure_flag=upgrading}}} possible={has_star_flag=build_here from={is_ai=no} fromfrom={if={limit={is_scope_type=megastructure} has_megastructure_flag=upgrading}}} placement_rules={planet_possible={is_star=yes root={has_star_flag=system} prev={has_star_flag=system} from={is_ai=no}}} context_menu_potential={has_star_flag=build_here fromfrom={has_fleet_flag=constructor}}}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    for body in [
        "potential={is_star=yes}",
        "possible={is_ai=no}",
        "context_menu_potential={fromfrom={has_megastructure_flag=wrong}}",
        "placement_rules={planet_possible={is_ai=no}}",
        "placement_rules={planet_possible={prev={is_ai=no}}}",
    ] {
        let e = declaration("megastructures", &format!("mega={{{body}}}"));
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "scope"),
            "{body}: {:?}",
            e.diagnostics.rows
        );
    }
}
#[test]
fn megastructure_dismantle_and_callback_entries_are_distinct() {
    let e = declaration(
        "megastructures",
        "mega={dismantle_potential={has_megastructure_flag=mine from={is_ai=no}} dismantle_possible={has_megastructure_flag=mine} should_ai_dismantle={has_megastructure_flag=mine} on_build_complete={set_star_flag=built from={set_country_flag=builder} fromfrom={set_megastructure_flag=finished} fromfromfrom={set_fleet_flag=constructor}} on_dismantle_start={set_star_flag=dismantling fromfrom={set_megastructure_flag=marked}} on_dismantle_cancel={fromfrom={set_megastructure_flag=cancelled}} on_build_cancel={from={set_country_flag=cancelled}} on_dismantle_complete={from={set_country_flag=complete}}}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    for body in [
        "dismantle_possible={is_ai=no}",
        "on_build_complete={set_planet_flag=wrong}",
        "on_build_complete={fromfrom={set_country_flag=wrong}}",
        "on_dismantle_start={fromfrom={set_fleet_flag=wrong}}",
    ] {
        let e = declaration("megastructures", &format!("mega={{{body}}}"));
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "scope"),
            "{body}: {:?}",
            e.diagnostics.rows
        );
    }
    let e = declaration(
        "megastructures",
        "mega={on_build_start={fromfrom={set_megastructure_flag=maybe}} on_build_complete={prev={set_starbase_flag=maybe}}}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.reason["code"] == "scope-type-unknown"),
        "{:?}",
        e.diagnostics.rows
    );
}
#[test]
fn megastructure_coordinates_and_modifier_conditions_are_checked() {
    let e = declaration(
        "megastructures",
        "mega={triggered_country_modifier={potential={has_megastructure_flag=active}} tooltip_system_filter={has_star_flag=ranked from={is_ai=no}} tooltip_system_score={base=5000 modifier={factor=2 has_star_flag=ranked}}}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    for (body, key) in [
        ("entity_offset=yes", "structure"),
        ("entity_offset={w=2}", "unknown-field"),
        ("entity_offset={x=NaN}", "value"),
        ("triggered_country_modifier={potential={is_ai=no}}", "scope"),
        ("tooltip_system_filter={is_ai=no}", "scope"),
    ] {
        let e = declaration("megastructures", &format!("mega={{{body}}}"));
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{body}: {:?}",
            e.diagnostics.rows
        );
    }
    let e = declaration(
        "megastructures",
        "mega={placement_rules={unverified_selector={always=yes}}}",
    );
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "validation-coverage"),
        "{:?}",
        e.diagnostics.rows
    );
}
#[test]
fn special_project_description_uses_vessel_and_keeps_unknown_root() {
    let e = declaration(
        "special_projects",
        "special_project={key=P event_scope=ship_event desc={trigger={is_ship_class=shipclass_science_ship} text=loc}}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(!has(&e, "rule-unimplemented"));
    let e = declaration(
        "special_projects",
        "special_project={key=P event_scope=ship_event desc={trigger={is_ai=no} text=loc}}",
    );
    assert!(
        e.diagnostics.rows.iter().any(|d| d.key == "scope"),
        "{:?}",
        e.diagnostics.rows
    );
    let e = declaration(
        "special_projects",
        "special_project={key=P event_scope=ship_event desc={trigger={is_aai=no} text=loc}}",
    );
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "unknown-trigger"),
        "{:?}",
        e.diagnostics.rows
    );
    let e = declaration(
        "special_projects",
        "special_project={key=P event_scope=ship_event desc={trigger={root={is_ai=no}} text=loc}}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "scope-type-unknown"), "{:?}", e.diagnostics.rows);
}
#[test]
fn special_project_failure_conditions_use_country_and_project_from() {
    for field in ["abort_trigger", "fail_trigger"] {
        let good = declaration(
            "special_projects",
            &format!(
                "special_project={{key=P event_scope=ship_event {field}={{is_ai=no from={{is_ship_class=shipclass_science_ship}}}}}}"
            ),
        );
        assert_eq!(errors(&good), 0, "{field}: {:?}", good.diagnostics.rows);
        assert!(
            !has(&good, "scope-type-unknown"),
            "{:?}",
            good.diagnostics.rows
        );
        let bad = declaration(
            "special_projects",
            &format!(
                "special_project={{key=P event_scope=ship_event {field}={{from={{is_ai=no}}}}}}"
            ),
        );
        assert!(
            bad.diagnostics.rows.iter().any(|d| d.key == "scope"),
            "{field}: {:?}",
            bad.diagnostics.rows
        );
        let unknown = declaration(
            "special_projects",
            &format!(
                "special_project={{key=P event_scope=ship_event {field}={{root={{is_ai=no}} fromfrom={{is_ai=no}}}}}}"
            ),
        );
        assert_eq!(errors(&unknown), 0, "{:?}", unknown.diagnostics.rows);
        assert!(
            has(&unknown, "scope-type-unknown"),
            "{:?}",
            unknown.diagnostics.rows
        );
    }
}
#[test]
fn special_project_callback_frames_keep_success_and_failure_distinct() {
    for field in [
        "on_success",
        "on_start",
        "on_progress_25",
        "on_progress_50",
        "on_progress_75",
    ] {
        let good = declaration(
            "special_projects",
            &format!(
                "special_project={{key=P event_scope=ship_event {field}={{set_ship_flag=ok prev={{set_country_flag=ok}}}}}}"
            ),
        );
        assert_eq!(errors(&good), 0, "{field}: {:?}", good.diagnostics.rows);
        let bad = declaration(
            "special_projects",
            &format!(
                "special_project={{key=P event_scope=ship_event {field}={{set_country_flag=bad}}}}"
            ),
        );
        assert!(
            bad.diagnostics.rows.iter().any(|d| d.key == "scope"),
            "{field}: {:?}",
            bad.diagnostics.rows
        );
    }
    for field in ["on_fail", "on_cancel"] {
        let good = declaration(
            "special_projects",
            &format!(
                "special_project={{key=P event_scope=ship_event {field}={{set_country_flag=ok from={{set_ship_flag=ok}}}}}}"
            ),
        );
        assert_eq!(errors(&good), 0, "{field}: {:?}", good.diagnostics.rows);
        let bad = declaration(
            "special_projects",
            &format!(
                "special_project={{key=P event_scope=ship_event {field}={{set_ship_flag=bad}}}}"
            ),
        );
        assert!(
            bad.diagnostics.rows.iter().any(|d| d.key == "scope"),
            "{field}: {:?}",
            bad.diagnostics.rows
        );
    }
}
#[test]
fn special_project_ai_wait_is_country_with_unknown_creation_scope() {
    let good = declaration(
        "special_projects",
        "special_project={key=P event_scope=ship_event AI_wait_days={weight=80 modifier={factor=2 is_ai=no}}}",
    );
    assert!(
        good.diagnostics.rows.is_empty(),
        "{:?}",
        good.diagnostics.rows
    );
    for body in [
        "weight={oops}",
        "modifier={factor=2 is_ship_class=shipclass_science_ship}",
        "modifier={factor=2 is_aai=no}",
    ] {
        let bad = declaration(
            "special_projects",
            &format!("special_project={{key=P event_scope=ship_event AI_wait_days={{{body}}}}}"),
        );
        assert!(errors(&bad) > 0, "{body}: {:?}", bad.diagnostics.rows);
    }
    let unknown = declaration(
        "special_projects",
        "special_project={key=P event_scope=ship_event AI_wait_days={modifier={factor=2 from={is_ai=no}}}}",
    );
    assert_eq!(errors(&unknown), 0, "{:?}", unknown.diagnostics.rows);
    assert!(
        has(&unknown, "scope-type-unknown"),
        "{:?}",
        unknown.diagnostics.rows
    );
}
#[test]
fn special_project_callbacks_require_blocks() {
    for field in [
        "on_success",
        "on_start",
        "on_progress_25",
        "on_progress_50",
        "on_progress_75",
        "on_fail",
        "on_cancel",
        "abort_trigger",
        "fail_trigger",
        "AI_wait_days",
    ] {
        let bad = declaration(
            "special_projects",
            &format!("special_project={{key=P {field}=yes}}"),
        );
        assert!(
            bad.diagnostics.rows.iter().any(|d| d.key == "structure"),
            "{field}: {:?}",
            bad.diagnostics.rows
        );
    }
}
#[test]
fn project_requirements_validate_nested_refs_and_numeric_shapes() {
    let mut e = make();
    for (category, name) in [
        ("ship_sizes", "science"),
        ("leader_classes", "scientist"),
        ("traits", "trait_good"),
        ("technology", "tech_good"),
    ] {
        e.program
            .entities
            .entry(category.into())
            .or_default()
            .insert(name.into());
    }
    let nodes = ast(
        &mut e,
        "special_project={key=P requirements={shipclass_science_ship=1 skill=2 leader=scientist has_trait=trait_good has_technology=tech_good research_station=yes size_requirement={ship_size=science count=1}}}",
    );
    e.definition("special_projects", &nodes[0]);
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    for (body, key) in [
        (
            "size_requirement={ship_size=missing count=1}",
            "missing-item",
        ),
        ("leader=missing", "missing-item"),
        ("has_technology=missing", "missing-item"),
        ("has_trait=missing", "missing-item"),
        ("size_requirement={ship_size={bad=yes}}", "structure"),
        ("skill={bad=yes}", "structure"),
        ("research_station=maybe", "value"),
        ("shipclass_science_shpi=1", "unknown-field"),
        ("size_requirement={conut=1}", "unknown-field"),
    ] {
        let e = declaration(
            "special_projects",
            &format!("special_project={{key=P requirements={{{body}}}}}"),
        );
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{body}: {:?}",
            e.diagnostics.rows
        );
    }
}
#[test]
fn project_triggered_requirement_uses_fleet_not_project_receiver() {
    let e = declaration(
        "special_projects",
        "special_project={key=P event_scope=ship_event triggered_requirement={text=loc count=1 trigger={is_mobile=yes}}}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(!has(&e, "rule-unimplemented"));
    for (body, key) in [
        ("text=loc trigger={is_star=yes}", "scope"),
        ("text=loc trigger={is_aai=yes}", "unknown-trigger"),
        ("text=loc", "missing-field"),
        ("trigger={is_mobile=yes}", "missing-field"),
        ("text=loc trigger=yes", "structure"),
        ("text=loc trigger={} conut=1", "unknown-field"),
    ] {
        let e = declaration(
            "special_projects",
            &format!("special_project={{key=P triggered_requirement={{{body}}}}}"),
        );
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{body}: {:?}",
            e.diagnostics.rows
        );
    }
}
#[test]
fn project_requirement_runtime_count_is_unknown_not_missing() {
    let e = declaration(
        "special_projects",
        "special_project={key=P requirements={fleet_size=var:dynamic_count}}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "numeric-information-unresolved"));
}
#[test]
fn crisis_country_contracts_and_unknown_root() {
    for category in ["crisis_levels", "menace_perks"] {
        let good = declaration(category, "x={on_unlock={set_country_flag=ok}}");
        assert!(
            good.diagnostics.rows.is_empty(),
            "{:?}",
            good.diagnostics.rows
        );
        let bad = declaration(category, "x={on_unlock={set_planet_flag=bad}}");
        assert!(bad.diagnostics.rows.iter().any(|d| d.key == "scope"));
        let root = declaration(category, "x={on_unlock={ROOT={set_country_flag=unknown}}}");
        assert_eq!(errors(&root), 0);
        assert!(has(&root, "scope-type-unknown"));
        assert!(
            declaration(category, "x={on_unlock=yes}")
                .diagnostics
                .rows
                .iter()
                .any(|d| d.key == "structure")
        );
    }
}
#[test]
fn crisis_structures_references_and_values() {
    for (category, text, key) in [
        ("crisis_levels", "x={perks={missing}}", "missing-item"),
        ("crisis_levels", "x={perks={missing=yes}}", "structure"),
        ("crisis_levels", "x={on_unlok={}}", "unknown-field"),
        ("crisis_paths", "x={levels={missing}}", "missing-item"),
        ("crisis_objectives", "x={recurring=maybe}", "value"),
        ("crisis_objectives", "x={reward={round=maybe}}", "value"),
        (
            "crisis_objectives",
            "x={reward={round_to={base=2}}}",
            "structure",
        ),
        (
            "crisis_objectives",
            "x={reward={base=10 modifier={factor=2 is_star=yes}}}",
            "scope",
        ),
    ] {
        let e = declaration(category, text);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
    assert!(
        declaration(
            "crisis_objectives",
            "x={potential={is_ai=no} recurring=yes reward=999999999}"
        )
        .diagnostics
        .rows
        .is_empty()
    );
}
#[test]
fn name_list_scope_and_structure_contracts() {
    assert!(
        declaration(
            "name_lists",
            "bio={trigger={is_ai=no} randomized=no should_name_home_system_planets=no}"
        )
        .diagnostics
        .rows
        .is_empty()
    );
    assert!(
        declaration("name_lists", "bio={selectable={always=no}}")
            .diagnostics
            .rows
            .is_empty()
    );
    for (text, key) in [
        ("bio={selectable={is_ai=no}}", "scope"),
        ("bio={selectable={is_star=yes}}", "scope"),
        ("bio={selectable=yes}", "structure"),
        ("bio={randomized=maybe}", "value"),
        ("bio={by_rarity_names={commmon={NAME}}}", "unknown-field"),
        (
            "bio={by_rarity_names={common={bad=NAME}}}",
            "name-list-structure",
        ),
    ] {
        let e = declaration("name_lists", text);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}
#[test]
fn boolean_trigger_rejects_block_shape() {
    let e = events("country_event={id=a trigger={is_ai={always=yes}}}");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "structure"));
}
#[test]
fn on_action_list_checks_zero_weight_references_without_dispatch() {
    let mut e = make();
    let event = ast(&mut e, "country_event={id=a is_triggered_only=yes}");
    e.program
        .events
        .insert("a".into(), ("country".into(), Arc::new(event[0].clone())));
    let nodes = ast(&mut e, "random_events={0=a 0=missing 5=0 -1=a 1e999=a}");
    e.on_action_list(&nodes[0], &Context::new("?"));
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-event"));
    assert_eq!(
        e.diagnostics
            .rows
            .iter()
            .filter(|d| d.key == "value")
            .count(),
        2
    );
    assert!(e.event_inputs.is_empty());
}
#[test]
fn on_action_unverified_presets_remain_unknown() {
    let mut e = make();
    let event = ast(&mut e, "country_event={id=a is_triggered_only=yes}");
    e.program
        .events
        .insert("a".into(), ("country".into(), Arc::new(event[0].clone())));
    let nodes = ast(&mut e, "events={a}");
    e.on_action_list(&nodes[0], &Context::new("?"));
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "engine-contract-unverified"));
    let input = e.event_inputs.values().next().unwrap();
    assert!(input.ctx.root.unknown);
    assert!(input.ctx.froms.is_empty());
}
#[test]
fn picture_selectors_check_shapes_without_guessing_unknown_contracts() {
    assert!(
        events("country_event={id=a picture_event_data={portrait=this room=this}}")
            .diagnostics
            .rows
            .is_empty()
    );
    let bad = events("country_event={id=a picture_event_data={portrait={x=y}}}");
    assert!(bad.diagnostics.rows.iter().any(|d| d.key == "structure"));
    let unsupported = events("ship_event={id=a picture_event_data={portrait=this}}");
    assert_eq!(errors(&unsupported), 0);
    assert!(has(&unsupported, "engine-contract-unverified"));
}
#[test]
fn aura_graphics_contract_references_boolean_and_required_fields() {
    let mut e = make();
    e.program
        .entities
        .entry("gfx_entity".into())
        .or_default()
        .insert("circle".into());
    let good = ast(
        &mut e,
        "graphics={area_effect={entity=circle dynamic_scale=yes} ship_effect={entity=circle dynamic_scale=no}}",
    );
    e.aura_graphics(&good[0], &Context::new("ship"));
    assert!(e.diagnostics.rows.is_empty());
    for (text, key) in [
        ("graphics={area_effect={entity=missing}}", "missing-item"),
        (
            "graphics={area_effect={entity=circle dynamic_scale=maybe}}",
            "value",
        ),
        (
            "graphics={area_effect={entity=circle dynamik_scale=yes}}",
            "unknown-field",
        ),
        (
            "graphics={area_effect={dynamic_scale=yes}}",
            "missing-field",
        ),
        ("graphics=123", "structure"),
    ] {
        let node = ast(&mut e, text);
        e.aura_graphics(&node[0], &Context::new("ship"));
        assert!(e.diagnostics.rows.iter().any(|d| d.key == key));
    }
}
#[test]
fn graphics_index_resolves_known_entities_and_meshes() {
    let mut e = make();
    let nodes = ast(
        &mut e,
        "pdxmesh={name=mesh} entity={name=circle pdxmesh=mesh}",
    );
    e.program.index(File {
        path: "fixture.asset".into(),
        category: "gfx".into(),
        ast: nodes.into_iter().map(Arc::new).collect(),
        target: true,
    });
    e.analyze();
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    let bad = ast(&mut e, "entity={name=bad pdxmesh=absent}");
    e.graphics_references(&bad[0]);
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
}

fn ship_fixture(replacements: &[(&str, &str)]) -> Engine {
    let mut e = make();
    for (category, source) in [
        (
            "ship_sizes",
            "hull={section_slots={mid={locator=mid}} required_component_set=reactor}",
        ),
        (
            "section_templates",
            "ship_section_template={key=section ship_size=hull fits_on_slot=mid component_slot={name=GUN template=small_turret}}",
        ),
        (
            "component_slot_templates",
            "small_turret={size=small component=weapon}",
        ),
        ("component_sets", "component_set={key=reactor}"),
        (
            "component_templates",
            "weapon_component_template={key=gun size=small power=-5 potential={is_ship_size=hull}} utility_component_template={key=core size=small component_set=reactor power=10 potential={is_ship_size=hull}}",
        ),
        (
            "global_ship_designs",
            "ship_design={name=design ship_size=hull section={slot=mid template=section component={slot=GUN template=gun}} required_component=core}",
        ),
    ] {
        let mut source = source.to_string();
        for (from, to) in replacements {
            source = source.replace(from, to);
        }
        let nodes = ast(&mut e, &source);
        e.program.index(File {
            path: format!("{category}.txt"),
            category: category.into(),
            ast: nodes.into_iter().map(Arc::new).collect(),
            target: false,
        });
    }
    e
}
fn check_ship(mut e: Engine) -> Engine {
    let design = e.program.definitions["global_ship_designs"]["design"].clone();
    e.design_contract(&design);
    e
}
#[test]
fn section_slot_declaration_checks_fields_shapes_and_references() {
    let mut e = ship_fixture(&[]);
    let nodes = ast(
        &mut e,
        "ship_section_template={key=s ship_size=hull fits_on_slot=mid component_slot={name=GUN template=small_turret locatorname=gun is_side_slot=no rotation=90 section_instance=0 position={x=0 y=1 z=2}}}",
    );
    e.definition("section_templates", &nodes[0]);
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    for (body, key) in [
        ("name=GUN template=missing", "missing-item"),
        ("name=GUN", "missing-field"),
        ("template=small_turret", "missing-field"),
        (
            "name=GUN template=small_turret templat=small_turret",
            "unknown-field",
        ),
        ("name=GUN template=small_turret is_side_slot=maybe", "value"),
        ("name=GUN template=small_turret position=yes", "structure"),
        (
            "name=GUN template=small_turret position={q=1}",
            "unknown-field",
        ),
        (
            "name=GUN template=small_turret position={x={bad=yes}}",
            "structure",
        ),
        (
            "name=GUN template=small_turret name=OTHER",
            "duplicate-field",
        ),
    ] {
        let mut e = ship_fixture(&[]);
        let nodes = ast(
            &mut e,
            &format!(
                "ship_section_template={{key=s ship_size=hull fits_on_slot=mid component_slot={{{body}}}}}"
            ),
        );
        e.definition("section_templates", &nodes[0]);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{body}: {:?}",
            e.diagnostics.rows
        );
    }
}
#[test]
fn slot_templates_check_size_type_and_fixed_flag() {
    let e = declaration(
        "component_slot_templates",
        "a={size=torpedo component=weapon is_fixed=yes entities={weapon_type_explosive=turret}} b={size=large component=strike_craft} c={size=any component=any}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    for (body, key) in [
        ("size=smoll component=weapon", "value"),
        ("size=small component=wepon", "value"),
        ("size=small", "missing-field"),
        ("component=weapon", "missing-field"),
        ("size=small component=weapon is_fixed=maybe", "value"),
        (
            "size=small component=weapon entities={weapon_type_energy={bad=yes}}",
            "structure",
        ),
    ] {
        let e = declaration("component_slot_templates", &format!("a={{{body}}}"));
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{body}: {:?}",
            e.diagnostics.rows
        );
    }
    let e = declaration(
        "component_slot_templates",
        "a={size=small component=weapon future_field=yes}",
    );
    assert!(has(&e, "rule-unimplemented"));
}
#[test]
fn ship_declaration_scalar_block_shapes_cannot_skip_validation() {
    for body in [
        "section_slots=yes",
        "section_slots={mid={template={bad=yes}}}",
        "section_slots={mid={locator={bad=yes}}}",
        "required_component_set={bad=yes}",
        "upgrades_to={bad=yes}",
        "min_upgrade_cost=1",
    ] {
        let e = declaration("ship_sizes", &format!("hull={{{body}}}"));
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "structure"),
            "{body}: {:?}",
            e.diagnostics.rows
        );
    }
    let e = declaration(
        "component_templates",
        "utility_component_template={key=core component_set={bad=yes}}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "structure"));
}
#[test]
fn required_component_set_is_typed_by_declaration_category() {
    let e = declaration(
        "component_sets",
        "component_set={key=reactor required_component_set=yes is_core_component_set=yes}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    let e = declaration(
        "component_sets",
        "component_set={key=reactor required_component_set=maybe}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "value"));
    let e = declaration("ship_sizes", "hull={required_component_set=yes}");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
}
#[test]
fn section_and_default_hull_section_crosschecks_run_without_designs() {
    for (from, to) in [
        ("fits_on_slot=mid", "fits_on_slot=stern"),
        ("key=section", "key=section"),
    ] {
        let mut e = ship_fixture(&[(from, to)]);
        if from == to {
            let nodes = ast(&mut e, "hull={section_slots={stern={template=section}}}");
            e.definition("ship_sizes", &nodes[0]);
        } else {
            let n = e.program.definitions["section_templates"]["section"].clone();
            e.definition("section_templates", &n);
        }
        assert!(
            e.diagnostics
                .rows
                .iter()
                .any(|d| d.key == "ship-compatibility"),
            "{:?}",
            e.diagnostics.rows
        );
    }
    let mut e = ship_fixture(&[]);
    let nodes = ast(
        &mut e,
        "ship_section_template={key=s ship_size=hull fits_on_slot=mid component_slot={name=GUN template=small_turret} component_slot={name=GUN template=small_turret}}",
    );
    e.definition("section_templates", &nodes[0]);
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "ship-compatibility")
    );
}
#[test]
fn required_set_contract_distinguishes_impossible_and_unresolved() {
    for (from, to, expected) in [
        (
            "potential={is_ship_size=hull}",
            "potential={is_ship_size=other}",
            "ship-compatibility",
        ),
        (
            "size=small component_set=reactor",
            "size=small component_set=other",
            "ship-compatibility",
        ),
        (
            "potential={is_ship_size=hull}",
            "potential={is_ai=no}",
            "validation-coverage",
        ),
    ] {
        let mut e = ship_fixture(&[(from, to)]);
        let n = e.program.definitions["ship_sizes"]["hull"].clone();
        e.definition("ship_sizes", &n);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == expected),
            "{to}: {:?}",
            e.diagnostics.rows
        );
        if expected == "validation-coverage" {
            assert!(
                !e.diagnostics
                    .rows
                    .iter()
                    .any(|d| d.key == "ship-compatibility")
            );
        }
    }
}
fn potential_proof(
    conditions: &[&str],
    assumption: &str,
    scripts: &str,
) -> crate::potential_proof::RequiredSetProof {
    let mut e = make();
    for def in ast(&mut e, scripts) {
        e.program
            .scripts
            .insert((false, def.key.clone()), Arc::new(def));
    }
    let hull = ast(
        &mut e,
        &format!("hull={{class=shipclass_military potential_country={{{assumption}}}}}"),
    )
    .remove(0);
    let candidates = conditions
        .iter()
        .map(|condition| ast(&mut e, &format!("component={{potential={{{condition}}}}}")).remove(0))
        .map(Arc::new)
        .collect::<Vec<_>>();
    e.prove_required_set(&hull, "reactor", &candidates)
}
#[test]
fn required_set_proof_restores_finite_boolean_and_hull_contracts() {
    for (conditions, status) in [
        (vec!["is_ship_size=hull"], "covered"),
        (vec!["is_ship_size=other"], "impossible"),
        (vec!["is_ship_class=shipclass_military"], "covered"),
        (vec!["is_ai=yes", "is_ai=no"], "covered"),
        (vec!["owner={is_ai=yes}", "owner={is_ai=no}"], "unknown"),
        (vec!["owner={is_ai=yes}", "root={is_ai=no}"], "unknown"),
        (vec!["not_documented=yes", "not_documented=no"], "unknown"),
        (
            vec!["opaque={always=yes}", "NOT={opaque={always=yes}}"],
            "unknown",
        ),
        (vec!["from={is_ai=no}"], "unknown"),
    ] {
        let p = potential_proof(&conditions, "", "");
        assert_eq!(p.status, status, "{conditions:?}: {p:?}");
    }
}
#[test]
fn required_set_proof_uses_country_assumptions_without_runtime_claims() {
    let p = potential_proof(&["is_ship_size=hull from={is_ai=no}"], "is_ai=no", "");
    assert_eq!(p.status, "covered", "{p:?}");
    assert!(p.proof_domain.contains("not proved"));
    let p = potential_proof(&["is_ship_size=hull from={is_ai=yes}"], "is_ai=no", "");
    assert_eq!(p.status, "impossible", "{p:?}");
    assert!(p.counterexample.is_some());
    let p = potential_proof(&["always=yes"], "always=no", "");
    assert_eq!(p.status, "unknown", "{p:?}");
}
#[test]
fn required_set_proof_expands_scripts_but_bounds_recursion_and_symbols() {
    let p = potential_proof(
        &["uses_hull=yes"],
        "",
        "uses_hull={optimize_memory OR={is_ship_size=hull is_ship_size=other}}",
    );
    assert_eq!(p.status, "covered", "{p:?}");
    let p = potential_proof(&["recursive=yes"], "", "recursive={recursive=yes}");
    assert_eq!(p.status, "unknown", "{p:?}");
    let body = (0..13)
        .map(|i| format!("has_country_flag=flag_{i} "))
        .collect::<String>();
    let p = potential_proof(&[&body], "", "");
    assert_eq!(p.status, "unknown");
    assert_eq!(p.reason, Some("symbol budget"));
    let body = "always=yes ".repeat(4097);
    let p = potential_proof(&[&body], "", "");
    assert_eq!(p.status, "unknown");
    assert_eq!(p.reason, Some("expression budget"));
    let scripts = (0..80)
        .map(|i| format!("p{i}={{p{}=yes}} ", i + 1))
        .collect::<String>();
    let p = potential_proof(&["p0=yes"], "", &scripts);
    assert_eq!(p.status, "unknown");
    assert_eq!(p.reason, Some("depth budget"));
    let potential = (0..13)
        .map(|i| format!("has_country_flag=flag_{i} "))
        .collect::<String>();
    let mut e = ship_fixture(&[(
        "potential={is_ship_size=hull}",
        &format!("potential={{{potential}}}"),
    )]);
    let hull = e.program.definitions["ship_sizes"]["hull"].clone();
    e.definition("ship_sizes", &hull);
    assert!(!e.complete);
    assert!(has(&e, "analysis-limit"));
}
#[test]
fn required_set_proof_does_not_treat_flag_names_as_boolean_values() {
    let p = potential_proof(&["has_country_flag=yes", "has_country_flag=no"], "", "");
    assert_eq!(p.status, "unknown", "{p:?}");
    let p = potential_proof(
        &[
            "has_country_flag=some_flag",
            "NOT={has_country_flag=some_flag}",
        ],
        "",
        "",
    );
    assert_eq!(p.status, "covered", "{p:?}");
}
#[test]
fn upgrade_cycles_are_checked_from_target_declarations() {
    for (category, source) in [
        ("ship_sizes", "a={upgrades_to=b} b={upgrades_to=a}"),
        (
            "component_templates",
            "utility_component_template={key=a upgrades_to=b} utility_component_template={key=b upgrades_to=a}",
        ),
    ] {
        let mut e = make();
        let nodes = ast(&mut e, source);
        e.program.index(File {
            path: "base.txt".into(),
            category: category.into(),
            ast: nodes.into_iter().map(Arc::new).collect(),
            target: false,
        });
        let n = e.program.definitions[category]["a"].clone();
        e.definition(category, &n);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "upgrade-cycle"),
            "{:?}",
            e.diagnostics.rows
        );
    }
}
#[test]
fn explicit_ship_loadout_and_compatibility_contracts() {
    let e = check_ship(ship_fixture(&[]));
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    assert_eq!(e.ship_feasibility[0].net_component_power, Some(5.));
    assert_eq!(e.ship_feasibility[0].status, "static_explicit_loadout");
    assert!(!e.ship_feasibility[0].automatic_design_validated);
    for (from, to, key) in [
        (
            "key=gun size=small",
            "key=gun size=large",
            "ship-compatibility",
        ),
        (
            "weapon_component_template=",
            "utility_component_template=",
            "ship-compatibility",
        ),
        ("template=section ", "template=absent ", "missing-item"),
        (
            "slot=GUN template=gun",
            "slot=MISSING template=gun",
            "ship-compatibility",
        ),
        (
            "size=small component_set=reactor",
            "size=small component_set=other",
            "ship-compatibility",
        ),
    ] {
        let e = check_ship(ship_fixture(&[(from, to)]));
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == key),
            "{from}: {:?}",
            e.diagnostics.rows
        );
    }
}
#[test]
fn explicit_ship_power_preserves_partial_and_adjusted_unknowns() {
    let e = check_ship(ship_fixture(&[("power=10", "power=1")]));
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "ship-power"));
    assert_eq!(e.ship_feasibility[0].status, "infeasible");
    for replacements in [
        vec![("required_component=core", "")],
        vec![("power=10", "power=1 modifier={ship_power_add=10}")],
        vec![("power=10", "power=dynamic")],
        vec![
            (
                "component_slot={name=GUN template=small_turret}",
                "small_utility_slots=dynamic",
            ),
            ("slot=GUN", "slot=SMALL_UTILITY_1"),
        ],
        vec![("power=10", "power=1"), ("is_ship_size=hull", "is_ai=no")],
    ] {
        let e = check_ship(ship_fixture(&replacements));
        assert!(
            !e.diagnostics.rows.iter().any(|d| d.key == "ship-power"),
            "{:?}",
            e.diagnostics.rows
        );
        assert_eq!(e.ship_feasibility[0].status, "unknown");
        assert!(!e.ship_feasibility[0].missing_conditions.is_empty());
    }
}
#[test]
fn explicit_ship_eligibility_is_three_valued() {
    let e = check_ship(ship_fixture(&[("is_ship_size=hull", "is_ship_size=other")]));
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "ship-compatibility")
    );
    let e = check_ship(ship_fixture(&[(
        "is_ship_size=hull",
        "OR={is_ship_size=hull is_ai=yes}",
    )]));
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    let e = check_ship(ship_fixture(&[(
        "is_ship_size=hull",
        "AND={is_ship_size=hull is_ai=yes}",
    )]));
    assert_eq!(e.ship_feasibility[0].status, "unknown");
}
#[test]
fn explicit_ship_csv_power_overrides_inline_power() {
    let mut e = ship_fixture(&[]);
    e.program.weapon_stats.insert(
        "gun".into(),
        crate::csv::WeaponStats {
            source: "stats.csv".into(),
            line: 2,
            fields: std::collections::BTreeMap::from([("power".into(), "-20".into())]),
        },
    );
    let e = check_ship(e);
    assert_eq!(e.ship_feasibility[0].net_component_power, Some(-10.));
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "ship-power"));
}
#[test]
fn explicit_ship_growth_stages_are_checked_independently() {
    let mut e = ship_fixture(&[]);
    let design = ast(
        &mut e,
        "ship_design={name=growing growth_stages={{ship_size=hull section={slot=mid template=section component={slot=GUN template=gun}} required_component=core}{ship_size=hull section={slot=mid template=absent} required_component=core}}}",
    );
    e.design_contract(&design[0]);
    assert_eq!(e.ship_feasibility.len(), 2);
    assert_eq!(e.ship_feasibility[0].status, "static_explicit_loadout");
    assert_eq!(e.ship_feasibility[1].status, "unknown");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
}
#[test]
fn repeated_design_checks_do_not_turn_deduplicated_errors_into_success() {
    let mut e = check_ship(ship_fixture(&[("power=10", "power=1")]));
    let errors = errors(&e);
    let design = e.program.definitions["global_ship_designs"]["design"].clone();
    e.design_contract(&design);
    assert_eq!(e.diagnostics.counts().0, errors);
    assert!(e.ship_feasibility.iter().all(|r| r.status == "infeasible"));
}
#[test]
fn valid_assignment_and_overwrite() {
    assert_eq!(effect("capital_scope={save_event_target_as=t} save_event_target_as=t event_target:t={set_country_flag=ok}").diagnostics.rows.len(),0);
}
#[test]
fn definite_wrong_type_detected() {
    assert!(
        errors(&effect(
            "save_event_target_as=t event_target:t={set_planet_flag=bad}"
        )) > 0
    );
}
#[test]
fn optional_type_not_erased_by_unrelated_branches() {
    let noise = (0..40)
        .map(|i| format!("random={{chance=50 save_event_target_as=x{i}}} "))
        .collect::<String>();
    let e = effect(&format!(
        "random={{chance=50 save_event_target_as=t}} {noise} if={{limit={{exists=event_target:t}} event_target:t={{set_planet_flag=bad}}}}"
    ));
    assert!(errors(&e) > 0);
    assert_eq!(e.metrics.enumerated_binding_paths, 0);
    assert!(!has(&e, "analysis-limit"));
}
#[test]
fn legal_optional_type_guarded() {
    let e = effect(
        "random={chance=50 capital_scope={save_event_target_as=t}} if={limit={exists=event_target:t} event_target:t={set_planet_flag=ok}}",
    );
    assert_eq!(e.diagnostics.rows.len(), 0);
}
#[test]
fn absent_sibling_cannot_borrow_type() {
    let e = effect(
        "random_list={1={capital_scope={save_event_target_as=t}} 1={event_target:t={set_planet_flag=ok}}}",
    );
    assert!(has(&e, "target-binding-unproven"));
}
#[test]
fn repeated_stable_guard_carries_fact() {
    let e = effect(
        "if={limit={is_ai=yes} capital_scope={save_event_target_as=t}} if={limit={is_ai=yes} event_target:t={set_planet_flag=ok}}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}
#[test]
fn opposite_stable_guard_uses_else_fact() {
    let e = effect(
        "if={limit={is_ai=yes} save_event_target_as=t} else={capital_scope={save_event_target_as=t}} if={limit={is_ai=no} event_target:t={set_planet_flag=ok}}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}
#[test]
fn different_objects_do_not_share_guards() {
    let e = effect(
        "if={limit={is_ai=yes} capital_scope={save_event_target_as=t}} random_country={if={limit={is_ai=yes} event_target:t={set_planet_flag=ok}}}",
    );
    assert!(has(&e, "target-binding-unproven"));
    assert_eq!(errors(&e), 0);
}
#[test]
fn assignment_invalidates_guard_delta() {
    let e = effect(
        "if={limit={is_ai=yes} capital_scope={save_event_target_as=t}} save_event_target_as=t if={limit={is_ai=yes} event_target:t={set_planet_flag=bad}}",
    );
    assert!(errors(&e) > 0);
}
#[test]
fn chance_zero_does_not_establish_binding() {
    let e = effect("random={chance=0 save_event_target_as=t} event_target:t={set_country_flag=ok}");
    assert!(has(&e, "target-binding-unproven"));
}
#[test]
fn chance_hundred_establishes_binding() {
    assert!(
        effect("random={chance=100 save_event_target_as=t} event_target:t={set_country_flag=ok}")
            .diagnostics
            .rows
            .is_empty()
    );
}
#[test]
fn every_loop_can_select_no_objects() {
    let e =
        effect("every_owned_planet={save_event_target_as=t} event_target:t={set_planet_flag=ok}");
    assert!(has(&e, "target-binding-unproven"));
}
#[test]
fn selector_has_no_second_iteration() {
    let e = effect(
        "save_event_target_as=t random_owned_planet={event_target:t={set_country_flag=ok} save_event_target_as=t}",
    );
    assert_eq!(errors(&e), 0);
}
#[test]
fn loop_carried_incompatible_type() {
    let e = effect(
        "save_event_target_as=t every_owned_planet={event_target:t={set_country_flag=bad} save_event_target_as=t}",
    );
    assert!(errors(&e) > 0 || has(&e, "scope-type-ambiguous"));
    assert!(e.complete);
}
#[test]
fn clear_does_not_keep_known_binding() {
    let e = effect(
        "save_global_event_target_as=t clear_global_event_target=t global_event_target:t={set_country_flag=ok}",
    );
    assert!(has(&e, "target-binding-unproven"));
}
#[test]
fn unknown_field_still_error() {
    assert!(errors(&effect("tiger_typo_effect=yes")) > 0);
}
#[test]
fn orphan_else_still_error() {
    assert!(errors(&effect("else={set_country_flag=ok}")) > 0);
}
#[test]
fn branch_count_scales_without_paths() {
    let mut previous = 0;
    for n in [10, 20, 40] {
        let body = (0..n)
            .map(|i| format!("random={{chance=50 save_event_target_as=t{i}}} "))
            .collect::<String>();
        let e = effect(&body);
        assert!(e.diagnostics.rows.is_empty());
        assert_eq!(e.metrics.transfer_nodes, 2 * n);
        assert_eq!(e.metrics.structure_nodes, 2 * n);
        assert_eq!(e.metrics.max_binding_facts, n as usize - 1);
        assert_eq!(e.metrics.enumerated_binding_paths, 0);
        if previous > 0 {
            assert_eq!(e.metrics.transfer_nodes, previous * 2);
        }
        previous = e.metrics.transfer_nodes;
    }
}
#[test]
fn trigger_or_does_not_borrow_type_guard() {
    let mut e = make();
    let body = ast(&mut e, "OR={is_scope_type=planet is_ai=yes}");
    e.triggers(&body, &Context::new("?"), &State::default());
    assert!(has(&e, "scope-type-unknown"));
    assert_eq!(errors(&e), 0);
}
fn events(text: &str) -> Engine {
    let mut e = make();
    let nodes = ast(&mut e, text).into_iter().map(Arc::new).collect();
    e.program.index(File {
        path: "fixture.txt".into(),
        category: "events".into(),
        ast: nodes,
        target: true,
    });
    e.analyze();
    e
}
#[test]
fn immediate_before_option_independent_of_source_order() {
    let e = events(
        "country_event={id=a option={event_target:t={set_country_flag=ok}} immediate={save_event_target_as=t}}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}
fn display_phase_event(text: &str) -> Engine {
    let mut e = make();
    e.program
        .loc
        .entry("english".into())
        .or_default()
        .insert("TT".into());
    e.program
        .entities
        .entry("gfx_spritetype".into())
        .or_default()
        .insert("GFX_good".into());
    let n = ast(&mut e, text).remove(0);
    e.event_body(&n, &Context::new("country"), &mut State::default());
    e
}
#[test]
fn event_display_reads_immediate_targets_independent_of_field_order() {
    let display = "picture_event_data={portrait=event_target:face} desc={trigger={event_target:face={has_leader_flag=ready}} text=TT} notification_event_icon={icon=GFX_good trigger={event_target:face={has_leader_flag=ready}}}";
    let immediate = "immediate={create_leader={class=commander effect={save_event_target_as=face set_leader_flag=ready}}}";
    for text in [
        format!("country_event={{id=a {display} {immediate}}}"),
        format!("country_event={{id=a {immediate} {display}}}"),
    ] {
        let e = display_phase_event(&text);
        assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
        assert_eq!(e.metrics.transfer_nodes, 3, "immediate must run once");
    }
}
#[test]
fn event_display_does_not_borrow_future_option_or_after_targets() {
    for later in [
        "option={create_leader={class=commander effect={save_event_target_as=face}}}",
        "after={create_leader={class=commander effect={save_event_target_as=face}}}",
    ] {
        let e = display_phase_event(&format!(
            "country_event={{id=a picture_event_data={{portrait=event_target:face}} {later}}}"
        ));
        assert!(
            has(&e, "scope-type-unknown"),
            "{later}: {:?}",
            e.diagnostics.rows
        );
    }
    let e = display_phase_event(
        "country_event={id=a desc={trigger={event_target:face={has_leader_flag=ready}} text=TT} immediate={random={chance=50 create_leader={class=commander effect={save_event_target_as=face}}}}}",
    );
    assert!(
        has(&e, "target-binding-unproven"),
        "{:?}",
        e.diagnostics.rows
    );
}
#[test]
fn event_entry_conditions_do_not_borrow_immediate_state() {
    let e = display_phase_event(
        "country_event={id=a immediate={create_leader={class=commander effect={save_event_target_as=face}}} trigger={event_target:face={has_leader_flag=ready}} picture_event_data={portrait=event_target:face}}",
    );
    assert!(
        has(&e, "target-binding-unproven"),
        "{:?}",
        e.diagnostics.rows
    );
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.message.contains("has_leader_flag")),
        "{:?}",
        e.diagnostics.rows
    );
}
#[test]
fn optional_named_receivers_only_export_conditional_writes() {
    let mut e = make();
    let body = ast(
        &mut e,
        "event_target:source={if={limit={is_scope_type=country} save_event_target_as=source}}",
    );
    let mut state = State::default();
    e.effects(&body, &Context::new("country"), &mut state);
    assert!(state.get("event_target:source").types.unknown);
    let mut e = make();
    let body = ast(
        &mut e,
        "event_target:source={save_event_target_as=copy event_target:source={set_country_flag=inside}} ",
    );
    let mut state = optional_target("source", "country");
    e.effects(&body, &Context::new("country"), &mut state);
    assert_eq!(
        state.get("event_target:copy").availability,
        Availability::Maybe
    );
    assert_eq!(state.get("event_target:copy").types.label(), "country");
    assert_eq!(
        e.diagnostics
            .rows
            .iter()
            .filter(|d| d.reason["code"] == "target-binding-unproven"
                && d.message.ends_with("event_target:source"))
            .count(),
        1,
        "the successful scope body knows its own receiver exists"
    );
    let mut e = make();
    let body = ast(&mut e, "event_target:source={save_event_target_as=copy}");
    let mut state = State::default();
    state.set(
        "event_target:source".into(),
        Binding::assigned(Types::new("country")),
    );
    e.effects(&body, &Context::new("country"), &mut state);
    assert_eq!(
        state.get("event_target:copy").availability,
        Availability::Definite
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    let mut e = make();
    let body = ast(
        &mut e,
        "event_target:source={save_event_target_as=copy} global_event_target:source={set_country_flag=wrong}",
    );
    let mut state = optional_target("source", "country");
    state.set(
        "global_event_target:source".into(),
        Binding::assigned(Types::new("leader")),
    );
    e.effects(&body, &Context::new("country"), &mut state);
    assert_eq!(state.get("event_target:copy").types.label(), "country");
    assert_eq!(
        state.get("event_target:copy").availability,
        Availability::Maybe
    );
    assert!(
        e.diagnostics.rows.iter().any(|d| d.key == "scope"),
        "global lookup must not fall back to a local shadow"
    );
    let mut e = make();
    let body = ast(
        &mut e,
        "event_target:source.owner={save_event_target_as=copy}",
    );
    let mut state = State::default();
    state.set(
        "event_target:source".into(),
        Binding::assigned(Types::new("planet")),
    );
    e.effects(&body, &Context::new("country"), &mut state);
    assert_eq!(
        state.get("event_target:copy").availability,
        Availability::Maybe,
        "a link type is not proof of owner presence"
    );
    for tail in [
        "this",
        "root",
        "this.root.this",
        "THIS",
        "ROOT",
        "this.ROOT.THIS",
    ] {
        let mut e = make();
        let body = ast(
            &mut e,
            &format!("event_target:source.{tail}={{save_event_target_as=copy}}"),
        );
        let mut state = State::default();
        state.set(
            "event_target:source".into(),
            Binding::assigned(Types::new("planet")),
        );
        e.effects(&body, &Context::new("country"), &mut state);
        assert_eq!(
            state.get("event_target:copy").availability,
            Availability::Definite,
            "{tail}"
        );
    }
}
#[test]
fn event_display_checks_the_overwritten_type_and_static_negative() {
    let mut e = make();
    e.program
        .loc
        .entry("english".into())
        .or_default()
        .insert("TT".into());
    let n=ast(&mut e,"country_event={id=a desc={trigger={event_target:face={is_ai=no}} text=TT} immediate={create_leader={class=commander effect={save_event_target_as=face}}}}").remove(0);
    let mut state = State::default();
    state.set(
        "event_target:face".into(),
        Binding::assigned(Types::new("country")),
    );
    e.event_body(&n, &Context::new("country"), &mut state);
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "scope" && d.message.contains("is_ai")),
        "{:?}",
        e.diagnostics.rows
    );
    let e = display_phase_event(
        "country_event={id=a picture_event_data={portrait={bad=yes}} immediate={save_event_target_as=face}}",
    );
    assert!(
        e.diagnostics.rows.iter().any(|d| d.key == "structure"),
        "{:?}",
        e.diagnostics.rows
    );
}
#[test]
fn options_are_alternatives() {
    let e = events(
        "country_event={id=a option={save_event_target_as=t} option={event_target:t={set_country_flag=ok}}}",
    );
    assert!(has(&e, "target-binding-unproven"));
}
#[test]
fn after_observes_option_join() {
    let e = events(
        "country_event={id=a option={save_event_target_as=t} option={save_event_target_as=t} after={event_target:t={set_country_flag=ok}}}",
    );
    assert!(e.diagnostics.rows.is_empty());
}
#[test]
fn event_carries_target_and_from() {
    let e = events(
        "country_event={id=a immediate={save_event_target_as=t country_event={id=b}}} country_event={id=b is_triggered_only=yes immediate={event_target:t={set_country_flag=ok} from={set_country_flag=ok}}}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    assert_eq!(e.metrics.event_evaluations, 2);
}
#[test]
fn overriding_from_preserves_deeper_event_frames() {
    let text = "planet_event={id=a immediate={every_country={country_event={id=b}}}} country_event={id=b is_triggered_only=yes immediate={country_event={id=c scopes={from=root}}}} country_event={id=c is_triggered_only=yes immediate={from={set_country_flag=ok} fromfrom={set_planet_flag=ok}}}";
    let e = events(text);
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    let e = events(&text.replace(
        "fromfrom={set_planet_flag=ok}",
        "fromfrom={set_country_flag=bad}",
    ));
    assert!(
        e.diagnostics.rows.iter().any(|d| d.key == "scope"),
        "{:?}",
        e.diagnostics.rows
    );
    assert!(!has(&e, "scope-type-unknown"));
}
#[test]
fn event_scope_overrides_require_block_and_scalar_values() {
    for call in ["scopes=yes", "scopes={from={bad=yes}}"] {
        let e = events(&"country_event={id=a immediate={country_event={id=b CALL}}} country_event={id=b is_triggered_only=yes}".replace("CALL", call));
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "structure"),
            "{call}: {:?}",
            e.diagnostics.rows
        );
    }
}
#[test]
fn event_type_mismatch() {
    let e = events(
        "country_event={id=a immediate={planet_event={id=b}}} country_event={id=b is_triggered_only=yes}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "event-type"));
}
#[test]
fn scripted_call_preserves_root_and_frame() {
    let mut e = make();
    let defs = ast(&mut e, "save={root={save_event_target_as=t}}");
    e.program
        .scripts
        .insert((true, "save".into()), Arc::new(defs[0].clone()));
    let body = ast(
        &mut e,
        "capital_scope={save=yes} event_target:t={set_country_flag=ok}",
    );
    e.effects(&body, &Context::new("country"), &mut State::default());
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}
#[test]
fn macro_context_is_validated_each_call() {
    let mut e = make();
    let defs = ast(&mut e, "check={event_target:$NAME$={set_country_flag=ok}}");
    e.program
        .scripts
        .insert((true, "check".into()), Arc::new(defs[0].clone()));
    let body = ast(
        &mut e,
        "save_event_target_as=t check={NAME=t} capital_scope={save_event_target_as=t} check={NAME=t}",
    );
    e.effects(&body, &Context::new("country"), &mut State::default());
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "scope"));
    assert!(!e.diagnostics.rows.iter().any(|d| d.key == "macro-argument"));
    assert_eq!(e.metrics.script_body_compilations, 1);
    assert_eq!(e.metrics.script_body_cache_hits, 1);
}
#[test]
fn macro_cache_separates_arguments_and_rechecks_missing_arguments() {
    let mut e = make();
    let defs = ast(&mut e, "save_named={save_event_target_as=$NAME$}");
    e.program
        .scripts
        .insert((true, "save_named".into()), Arc::new(defs[0].clone()));
    let body = ast(
        &mut e,
        "save_named={NAME=a} capital_scope={save_named={NAME=b}} save_named={NAME=a} save_named={}",
    );
    let mut state = State::default();
    e.effects(&body, &Context::new("country"), &mut state);
    assert_eq!(state.get("event_target:a").types.label(), "country");
    assert_eq!(state.get("event_target:b").types.label(), "colony");
    assert_eq!(e.metrics.script_body_compilations, 2);
    assert_eq!(e.metrics.script_body_cache_hits, 1);
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "macro-argument"));
}
#[test]
fn cached_macro_error_retains_each_call_site() {
    let mut e = make();
    let defs = ast(&mut e, "check={event_target:$NAME$={set_country_flag=ok}}");
    e.program
        .scripts
        .insert((true, "check".into()), Arc::new(defs[0].clone()));
    let body = ast(
        &mut e,
        "save_event_target_as=t check={NAME=t}\ncapital_scope={save_event_target_as=t} check={NAME=t}\ncheck={NAME=t}",
    );
    e.effects(&body, &Context::new("country"), &mut State::default());
    let row = e
        .diagnostics
        .rows
        .iter()
        .find(|d| d.key == "scope")
        .unwrap();
    assert_eq!(row.occurrences.len(), 2);
    assert_ne!(row.occurrences[0].call_chain, row.occurrences[1].call_chain);
    assert_eq!(e.metrics.script_body_cache_hits, 2);
    assert!(!row.occurrences_truncated);
}
#[test]
fn inline_if_else_materialized_before_grouping() {
    let mut e = make();
    e.program.inline.insert(
        "branch".into(),
        (
            "inline.txt".into(),
            "if={limit={is_ai=yes} save_event_target_as=t}".into(),
        ),
    );
    let body = ast(
        &mut e,
        "inline_script=branch else={save_event_target_as=t} event_target:t={set_country_flag=ok}",
    );
    e.effects(&body, &Context::new("country"), &mut State::default());
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}
#[test]
fn inline_recursion_reports_boundary() {
    let mut e = make();
    e.program.inline.insert(
        "loop".into(),
        ("inline.txt".into(), "inline_script=loop".into()),
    );
    let body = ast(&mut e, "inline_script=loop");
    e.effects(&body, &Context::new("country"), &mut State::default());
    assert!(has(&e, "analysis-limit"));
}
#[test]
fn fixed_point_produces_maybe_not_definite() {
    let mut e = make();
    let body = ast(&mut e, "every_owned_planet={save_event_target_as=t}");
    let mut s = State::default();
    e.effects(&body, &Context::new("country"), &mut s);
    assert_eq!(s.get("event_target:t").availability, Availability::Maybe);
    assert_eq!(s.get("event_target:t").types.label(), "planet");
}
#[test]
fn foreign_rules_rejected() {
    let mut e = make();
    e.rules.effects.get_mut("set_country_flag").unwrap().input = vec!["landed_title".into()];
    assert!(e.rules.validate().is_err());
}
#[test]
fn weapon_damage_contract() {
    let mut e = make();
    let nodes = ast(
        &mut e,
        "weapon_component_template={key=a damage={min=20 max=10} cooldown=0}",
    );
    e.definition("component_templates", &nodes[0]);
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "range-order"));
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "unknown-field"));
}
#[test]
fn high_damage_is_not_invalid() {
    let mut e = make();
    let nodes = ast(
        &mut e,
        "weapon_component_template={key=a damage={min=100000000 max=999999999} total_fire_time=1}",
    );
    e.definition("component_templates", &nodes[0]);
    assert_eq!(errors(&e), 0);
}
#[test]
fn ship_upgrade_is_not_component_reference() {
    let mut e = make();
    e.program
        .entities
        .entry("ship_sizes".into())
        .or_default()
        .insert("next".into());
    let nodes = ast(&mut e, "test={upgrades_to=next}");
    e.definition("ship_sizes", &nodes[0]);
    assert_eq!(errors(&e), 0);
}
#[test]
fn count_one_has_no_second_loop_visit() {
    let e = effect(
        "save_event_target_as=t while={count=1 limit={always=yes} event_target:t={set_country_flag=ok} capital_scope={save_event_target_as=t}}",
    );
    assert_eq!(errors(&e), 0);
    assert!(!has(&e, "scope-type-ambiguous"));
}
#[test]
fn count_zero_still_checks_grammar() {
    let e = effect("while={count=0 limit={always=yes} typo_effect=yes}");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "unknown-effect"));
    assert_eq!(e.metrics.loop_iterations, 0);
}
#[test]
fn impossible_loop_does_not_establish_target() {
    let e = effect(
        "while={limit={always=no} save_event_target_as=t} event_target:t={set_country_flag=ok}",
    );
    assert!(has(&e, "target-binding-unproven"));
    assert_eq!(e.metrics.loop_iterations, 0);
}
#[test]
fn root_alias_preserves_same_object_guard() {
    let e = effect(
        "if={limit={is_ai=yes} capital_scope={save_event_target_as=t}} root={if={limit={is_ai=yes} event_target:t={set_planet_flag=ok}}}",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}
#[test]
fn this_link_does_not_reset_selected_object() {
    let mut e = make();
    let n = ast(&mut e, "test=yes")[0].clone();
    let a = e.resolve(
        "capital_scope.this",
        &Context::new("country"),
        &State::default(),
        &n,
        false,
    );
    assert_ne!(a.object, "root");
    assert_eq!(a.this.label(), "colony");
}
#[test]
fn event_does_not_borrow_caller_predicate() {
    let e = events(
        "country_event={id=a immediate={if={limit={is_ai=yes} capital_scope={save_event_target_as=t}} random_country={country_event={id=b}}}} country_event={id=b is_triggered_only=yes immediate={if={limit={is_ai=yes} event_target:t={set_planet_flag=ok}}}}",
    );
    assert!(
        has(&e, "target-binding-unproven"),
        "{:?}",
        e.diagnostics.rows
    );
}
#[test]
fn else_if_guard_needs_preceding_conditions() {
    let e = effect(
        "if={limit={is_ai=yes} set_country_flag=ok} else_if={limit={exists=capital_scope} capital_scope={save_event_target_as=t}} if={limit={exists=capital_scope} event_target:t={set_planet_flag=ok}}",
    );
    assert!(has(&e, "target-binding-unproven"));
}

#[test]
fn global_lookup_does_not_borrow_local_save() {
    let e = effect("save_event_target_as=t global_event_target:t={set_country_flag=ok}");
    assert!(has(&e, "target-binding-unproven"));
}
#[test]
fn nested_inline_recursion_under_block_is_bounded() {
    let mut e = make();
    e.program.inline.insert(
        "loop".into(),
        (
            "inline.txt".into(),
            "if={limit={always=yes} inline_script=loop}".into(),
        ),
    );
    let body = ast(&mut e, "inline_script=loop");
    e.effects(&body, &Context::new("country"), &mut State::default());
    assert!(has(&e, "analysis-limit"));
    assert!(!e.complete);
}
#[test]
fn root_event_template_is_indexed_and_checked() {
    let mut e = make();
    e.program.inline.insert(
        "event".into(),
        (
            "inline.txt".into(),
            "country_event={id=a immediate={set_planet_flag=bad}}".into(),
        ),
    );
    let nodes = ast(&mut e, "inline_script=event");
    e.program.index(File {
        path: "fixture.txt".into(),
        category: "events".into(),
        ast: nodes.into_iter().map(Arc::new).collect(),
        target: true,
    });
    e.analyze();
    assert!(e.program.events.contains_key("a"));
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "scope"));
}

#[test]
fn uppercase_logic_and_root_are_valid() {
    let e = effect("IF={limit={is_ai=yes} ROOT={set_country_flag=ok}} ELSE={set_country_flag=ok}");
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}
#[test]
fn conditional_triggers_and_iterator_limit_are_valid() {
    let mut e = make();
    let nodes = ast(
        &mut e,
        "OR={always=yes always=no} if={limit={is_ai=yes} has_country_flag=a} else={has_country_flag=b} any_owned_planet={limit={has_planet_flag=a} has_planet_flag=b}",
    );
    e.triggers(&nodes, &Context::new("country"), &State::default());
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}
