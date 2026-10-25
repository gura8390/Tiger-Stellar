use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
use tiger_stellar::hash::sha256;
static SERIAL: AtomicUsize = AtomicUsize::new(0);
#[test]
fn cli_variable_arithmetic_crosses_synchronous_script_calls() {
    let f = fixture("numeric-script-transfer");
    write(
        &f.mod_path.join("common/scripted_effects/a.txt"),
        "subtract_two={subtract_variable={which=x value=2}}",
    );
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a immediate={set_variable={which=x value=2} subtract_two=yes set_variable={which=n value=10} divide_variable={which=n value=root.x}}}",
    );
    let (code, r) = invoke(&f, "proven-zero", &[]);
    assert_eq!(code, 1, "{r}");
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["key"] == "value"
                && d["message"].as_str().unwrap().contains("must not be zero")),
        "{r}"
    );
}
#[test]
fn cli_runtime_numeric_export_is_not_an_unset_variable() {
    let f = fixture("numeric-export");
    write(
        &f.game.join("common/strategic_resources/a.txt"),
        "energy={}",
    );
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a immediate={export_resource_stockpile_to_variable={resource=energy variable=stock} set_variable={which=copy value=stock} if={limit={check_variable={which=copy value>=0}} log=ok}}}",
    );
    let (code, r) = invoke(&f, "runtime-number", &[]);
    assert_eq!(code, 0, "{r}");
    assert_eq!(r["errors"], 0);
    assert_eq!(r["warnings"], 0);
}
#[test]
fn cli_variable_shapes_and_presence_are_distinct_contracts() {
    let f = fixture("numeric-shapes");
    let path = f.mod_path.join("events/a.txt");
    for (label, body, expected, key) in [
        (
            "typo",
            "set_variable={which=x value=1 valuue=2}",
            1,
            "unknown-field",
        ),
        (
            "scope-not-number",
            "set_variable={which=x value=event_target:t}",
            1,
            "value",
        ),
        (
            "cleared",
            "clear_variable=x if={limit={check_variable={which=x value>=0}} log=bad}",
            1,
            "variable",
        ),
        (
            "unknown-external",
            "set_variable={which=x value=external}",
            2,
            "validation-coverage",
        ),
        (
            "guarded-runtime",
            "if={limit={is_variable_set=x check_variable={which=x value>=0}} log=ok}",
            0,
            "",
        ),
    ] {
        write(
            &path,
            &format!("country_event={{id=a immediate={{{body}}}}}"),
        );
        let (code, r) = invoke(&f, label, &[]);
        assert_eq!(code, expected, "{label}: {r}");
        if !key.is_empty() {
            assert!(
                r["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["key"] == key),
                "{label}: {r}"
            );
        }
        if label == "unknown-external" {
            assert_eq!(r["errors"], 0);
            assert!(
                r["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["reason"]["code"] == "numeric-binding-unproven")
            );
        }
    }
}
#[test]
fn cli_event_delay_does_not_borrow_old_numeric_constants() {
    let f = fixture("numeric-delay");
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a immediate={set_variable={which=x value=0} country_event={id=b days=1}}} country_event={id=b is_triggered_only=yes immediate={set_variable={which=n value=1} from={divide_variable={which=n value=x}}}}",
    );
    let (code, r) = invoke(&f, "future-event-values", &[]);
    assert_eq!(code, 2, "{r}");
    assert_eq!(r["errors"], 0);
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["reason"]["code"] == "numeric-binding-unproven"),
        "{r}"
    );
}
#[test]
fn cli_uninvoked_input_contract_preserves_unknown_caller_and_root() {
    let f = fixture("input-contract");
    write(
        &f.mod_path.join("common/scripted_effects/a.txt"),
        "helper={set_country_flag=ok owner={prev={set_country_flag=ok}} root={set_ship_flag=unknown_root}}",
    );
    let (code, r) = invoke(&f, "unknown-caller", &[]);
    assert_eq!(code, 2, "{r}");
    assert_eq!(r["errors"], 0);
    let row = &r["inferred_script_inputs"][0];
    assert_eq!(row["input_types"]["known"], json!(["country"]));
    assert_eq!(row["root_types"]["unknown"], true);
    assert_eq!(row["caller_proven"], false);
    assert_eq!(row["runtime_availability_proven"], false);
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["reason"]["code"] == "caller-context-unknown")
    );
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["reason"]["code"] == "scope-type-unknown")
    );
}
#[test]
fn cli_uninvoked_contract_detects_wrong_prev_and_actual_calls_keep_real_root() {
    let f = fixture("input-contract-negative");
    let script = f.mod_path.join("common/scripted_effects/a.txt");
    write(
        &script,
        "helper={set_country_flag=ok owner={prev={set_ship_flag=bad}}}",
    );
    let (code, r) = invoke(&f, "wrong-prev", &[]);
    assert_eq!(code, 1, "{r}");
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["key"] == "scope"
                && d["call_chain"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|c| c["kind"] == "inferred-script-input")),
        "{r}"
    );
    write(
        &script,
        "helper={set_country_flag=ok root={set_ship_flag=ok}}",
    );
    write(
        &f.mod_path.join("events/a.txt"),
        "ship_event={id=a immediate={owner={helper=yes}}}",
    );
    let (code, r) = invoke(&f, "nested-country-with-ship-root", &[]);
    assert_eq!(code, 0, "{r}");
    assert!(
        r["inferred_script_inputs"].as_array().unwrap().is_empty(),
        "{r}"
    );
    write(
        &f.mod_path.join("events/a.txt"),
        "planet_event={id=a immediate={helper=yes}}",
    );
    let (code, r) = invoke(&f, "actual-wrong-input", &[]);
    assert_eq!(code, 1, "{r}");
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["key"] == "scope"),
        "{r}"
    );
}
#[test]
fn cli_uninvoked_guarded_script_is_not_forced_to_one_input_type() {
    let f = fixture("input-contract-guards");
    write(
        &f.mod_path.join("common/scripted_triggers/a.txt"),
        "helper={OR={AND={is_scope_type=country is_ai=no} AND={is_scope_type=ship is_ship_class=shipclass_science_ship}}}",
    );
    let (code, r) = invoke(&f, "guarded-polymorphic", &[]);
    assert_eq!(code, 2, "{r}");
    assert_eq!(r["errors"], 0);
    assert!(
        r["inferred_script_inputs"].as_array().unwrap().is_empty(),
        "{r}"
    );
}
#[test]
fn cli_uninvoked_script_deferred_events_are_checked_with_hypothetical_trace() {
    let f = fixture("input-contract-callee");
    write(
        &f.mod_path.join("common/scripted_effects/a.txt"),
        "helper={country_event={id=native.1}}",
    );
    write(
        &f.game.join("events/a.txt"),
        "country_event={id=native.1 is_triggered_only=yes immediate={misspelled_effect=yes}}",
    );
    let (code, r) = invoke(&f, "deferred-native-callee", &[]);
    assert_eq!(code, 1, "{r}");
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["key"] == "unknown-effect"
                && d["call_chain"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|c| c["kind"] == "inferred-script-input" && c["caller_proven"] == false)),
        "{r}"
    );
    write(
        &f.mod_path.join("common/scripted_effects/a.txt"),
        "helper={root={country_event={id=native.1}}}",
    );
    let (code, r) = invoke(&f, "unknown-input-deferred-callee", &[]);
    assert_eq!(code, 1, "{r}");
    assert!(r["inferred_script_inputs"].as_array().unwrap().is_empty());
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["key"] == "unknown-effect"
                && d["call_chain"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|c| c["kind"] == "uninvoked-script-contract"
                        && c["definition"] == "helper"
                        && c["caller_proven"] == false)),
        "{r}"
    );
}
#[test]
fn cli_project_callbacks_follow_location_without_lending_future_writes() {
    let f = fixture("project-location-flow");
    write(
        &f.game.join("common/special_projects/a.txt"),
        "special_project={key=P event_scope=ship_event on_success={from={set_planet_flag=ok} save_event_target_as=future} abort_trigger={from={is_ship_class=shipclass_science_ship} fromfrom={is_planet_class=pc_barren}}}",
    );
    // Both the literal planet class and the receiver are engine contracts;
    // provide a definition so missing references cannot mask scope assertions.
    write(&f.game.join("common/planet_classes/a.txt"), "pc_barren={}");
    let path = f.mod_path.join("events/a.txt");
    for (label, text, expected) in [
        (
            "planet-default",
            "country_event={id=a immediate={capital_scope={enable_special_project={name=P}}}}",
            0,
        ),
        (
            "planet-explicit",
            "country_event={id=a immediate={enable_special_project={name=P location=capital_scope}}}",
            0,
        ),
        (
            "country-wrong-location",
            "country_event={id=a immediate={enable_special_project={name=P}}}",
            1,
        ),
        (
            "ship-carrier-location-is-legal",
            "ship_event={id=a immediate={enable_special_project={name=P owner=owner}}}",
            0,
        ),
        (
            "fleet-wrong-location",
            "fleet_event={id=a immediate={enable_special_project={name=P owner=owner}}}",
            1,
        ),
    ] {
        write(&path, text);
        let (code, r) = invoke(&f, label, &[]);
        assert_eq!(code, expected, "{label}: {r}");
        let diagnostics = r["diagnostics"].as_array().unwrap();
        if expected == 1 {
            assert!(
                diagnostics.iter().any(|d| d["key"] == "scope"
                    && d["severity"] == "error"
                    && d["path"]
                        .as_str()
                        .unwrap()
                        .replace('\\', "/")
                        .ends_with("special_projects/a.txt")),
                "{label}: {r}"
            );
            assert!(
                diagnostics.iter().any(|d| d["call_chain"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|c| c["kind"] == "project-activation")),
                "{r}"
            );
        }
    }
    write(
        &path,
        "country_event={id=a immediate={capital_scope={enable_special_project={name=P}} event_target:future={set_ship_flag=no_leak}}}",
    );
    let (_, r) = invoke(&f, "no-future-target-loan", &[]);
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["reason"]["code"] == "target-binding-unproven"
                && d["path"]
                    .as_str()
                    .unwrap()
                    .replace('\\', "/")
                    .ends_with("events/a.txt")),
        "{r}"
    );
}
#[test]
fn cli_project_callback_cycles_are_bounded_and_keep_engine_unknown_entry() {
    let f = fixture("project-cycle");
    write(
        &f.mod_path.join("common/special_projects/a.txt"),
        "special_project={key=P event_scope=country_event on_success={enable_special_project={name=P owner=this location=this} from={set_planet_flag=wrong_for_country}}}",
    );
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a immediate={enable_special_project={name=P}}}",
    );
    let (code, r) = invoke(&f, "cycle", &[]);
    assert_eq!(code, 1, "{r}");
    let diagnostics = r["diagnostics"].as_array().unwrap();
    assert!(diagnostics.iter().any(|d| d["key"] == "scope"), "{r}");
    assert!(
        diagnostics
            .iter()
            .any(|d| d["reason"]["code"] == "scope-type-unknown"),
        "{r}"
    );
    assert!(
        !diagnostics
            .iter()
            .any(|d| d["reason"]["code"] == "analysis-limit"),
        "{r}"
    );
}
#[test]
fn cli_project_ai_wait_uses_creation_instead_of_location_scope() {
    let f = fixture("project-creation-scope");
    write(
        &f.game.join("common/special_projects/a.txt"),
        "special_project={key=P event_scope=ship_event AI_wait_days={weight=80 modifier={factor=2 from={is_ai=no}}}}",
    );
    let path = f.mod_path.join("events/a.txt");
    write(
        &path,
        "country_event={id=a immediate={enable_special_project={name=P location=capital_scope}}}",
    );
    let (code, r) = invoke(&f, "country-creation-planet-location", &[]);
    assert_eq!(code, 0, "{r}");
    write(
        &path,
        "country_event={id=a immediate={capital_scope={enable_special_project={name=P location=root}}}}",
    );
    let (code, r) = invoke(&f, "planet-creation-country-location", &[]);
    assert_eq!(code, 1, "{r}");
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["key"] == "scope"),
        "{r}"
    );
}
#[test]
fn cli_project_activation_checks_default_owner_and_reference() {
    let f = fixture("project-activation");
    write(
        &f.game.join("common/special_projects/a.txt"),
        "special_project={key=P}",
    );
    let path = f.mod_path.join("events/a.txt");
    for (label, text, code, key) in [
        (
            "country-default",
            "country_event={id=a immediate={enable_special_project={name=P}}}",
            0,
            "",
        ),
        (
            "planet-with-country-root",
            "country_event={id=a immediate={capital_scope={enable_special_project={name=P location=this}}}}",
            0,
            "",
        ),
        (
            "ship-explicit-country",
            "ship_event={id=a immediate={enable_special_project={name=P owner=owner location=this}}}",
            0,
            "",
        ),
        (
            "ship-invalid-default",
            "ship_event={id=a immediate={enable_special_project={name=P}}}",
            1,
            "scope",
        ),
        (
            "missing-project",
            "country_event={id=a immediate={enable_special_project={name=MISSING}}}",
            1,
            "missing-item",
        ),
        (
            "bad-location",
            "country_event={id=a immediate={enable_special_project={name=P location=root.bad_link}}}",
            1,
            "unknown-scope",
        ),
    ] {
        write(&path, text);
        let (actual, r) = invoke(&f, label, &[]);
        assert_eq!(actual, code, "{label}: {r}");
        if !key.is_empty() {
            assert!(
                r["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["key"] == key && d["severity"] == "error"),
                "{label}: {r}"
            );
        }
    }
}
#[test]
fn cli_project_activation_shapes_are_checked_even_when_unreachable() {
    let f = fixture("project-activation-shapes");
    write(
        &f.game.join("common/special_projects/a.txt"),
        "special_project={key=P}",
    );
    let path = f.mod_path.join("events/a.txt");
    for (label, effect, key) in [
        ("scalar", "enable_special_project=yes", "structure"),
        ("missing-name", "enable_special_project={}", "missing-field"),
        (
            "typo",
            "enable_special_project={name=P owenr=root}",
            "unknown-field",
        ),
        (
            "unreachable-reference",
            "if={limit={always=no} enable_special_project={name=MISSING}}",
            "missing-item",
        ),
    ] {
        write(
            &path,
            &format!("country_event={{id=a immediate={{{effect}}}}}"),
        );
        let (code, r) = invoke(&f, label, &[]);
        assert_eq!(code, 1, "{label}: {r}");
        assert!(
            r["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["key"] == key && d["severity"] == "error"),
            "{label}: {r}"
        );
    }
}
#[test]
fn cli_named_target_guard_types_the_successful_branch_and_rejects_wrong_effect() {
    let f = fixture("named-target-type-guard");
    let path = f.mod_path.join("events/a.txt");
    write(
        &path,
        "country_event={id=a immediate={if={limit={exists=global_event_target:t global_event_target:t={is_scope_type=leader}} global_event_target:t={set_leader_flag=ok}}}}",
    );
    let (code, r) = invoke(&f, "positive", &[]);
    assert_eq!(code, 0, "{r}");
    write(
        &path,
        "country_event={id=a immediate={save_event_target_as=t event_target:t?={set_country_flag=ok}}}",
    );
    let (code, r) = invoke(&f, "known-optional-receiver", &[]);
    assert_eq!(code, 0, "{r}");
    write(
        &path,
        "country_event={id=a immediate={if={limit={exists=global_event_target:t global_event_target:t={is_scope_type=leader}} global_event_target:t={set_country_flag=wrong}}}}",
    );
    let (code, r) = invoke(&f, "wrong-effect", &[]);
    assert_eq!(code, 1, "{r}");
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["key"] == "scope"
                && d["message"].as_str().unwrap().contains("set_country_flag")),
        "{r}"
    );
}
#[test]
fn cli_named_target_guard_keeps_optional_and_outside_uses_uncertain() {
    for (label, condition, tail) in [
        (
            "and-entry",
            "AND={event_target:t={is_scope_type=leader}}",
            "",
        ),
        (
            "or-entry",
            "OR={AND={event_target:t={is_scope_type=leader}} AND={event_target:t={is_scope_type=country}}}",
            "",
        ),
        (
            "double-not-entry",
            "NOT={NOT={event_target:t={is_scope_type=leader}}}",
            "",
        ),
        ("optional", "event_target:t? = {is_scope_type=leader}", ""),
        (
            "negative",
            "NOT={event_target:t={is_scope_type=leader}}",
            "",
        ),
        (
            "outside",
            "event_target:t={is_scope_type=leader}",
            "event_target:t={set_leader_flag=outside}",
        ),
    ] {
        let f = fixture(&format!("named-target-{label}"));
        write(
            &f.mod_path.join("events/a.txt"),
            &format!(
                "country_event={{id=a immediate={{if={{limit={{{condition}}} event_target:t={{set_leader_flag=inside}}}} {tail}}}}}"
            ),
        );
        let (code, r) = invoke(&f, label, &[]);
        assert_eq!(code, 2, "{label}: {r}");
        assert!(
            r["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["reason"]["code"] == "target-binding-unproven"),
            "{label}: {r}"
        );
    }
}
fn texture_bytes(payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 128];
    bytes[..4].copy_from_slice(b"DDS ");
    for (offset, value) in [(4, 124u32), (12, 16), (16, 16), (76, 32)] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(payload);
    bytes
}
fn write_texture(path: &Path, payload: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, texture_bytes(payload)).unwrap();
}
#[test]
fn cli_optional_receiver_does_not_guarantee_the_display_target() {
    let f = fixture("display-phase-optional-receiver");
    write(&f.game.join("common/leader_classes/a.txt"), "commander={}");
    write(
        &f.game.join("localisation/english/a_l_english.yml"),
        "l_english:\n TT:0 \"Text\"\n",
    );
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a desc={trigger={event_target:face={has_leader_flag=ready}} text=TT} immediate={random={chance=50 save_event_target_as=source} event_target:source={create_leader={class=commander effect={save_event_target_as=face}}}}}",
    );
    let (code, r) = invoke(&f, "optional-scope", &[]);
    assert_eq!(code, 2, "{r}");
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["reason"]["code"] == "target-binding-unproven"
                && d["message"]
                    .as_str()
                    .unwrap()
                    .ends_with("event_target:face")),
        "{r}"
    );
}
#[test]
fn cli_display_phase_reads_immediate_leader_and_detects_wrong_target_type() {
    let f = fixture("display-phase-immediate-leader");
    write(&f.game.join("common/leader_classes/a.txt"), "commander={}");
    write(
        &f.game.join("localisation/english/a_l_english.yml"),
        "l_english:\n TT:0 \"Text\"\n",
    );
    let path = f.mod_path.join("events/a.txt");
    write(
        &path,
        "country_event={id=a picture_event_data={portrait=event_target:face} desc={trigger={event_target:face={has_leader_flag=ready}} text=TT} immediate={create_leader={class=commander effect={save_event_target_as=face set_leader_flag=ready}}}}",
    );
    let (code, r) = invoke(&f, "positive", &[]);
    assert_eq!(code, 0, "{r}");
    assert_eq!(r["checked_statements"]["transfer_nodes"], 3);
    write(
        &path,
        "country_event={id=a desc={trigger={event_target:face={is_ai=no}} text=TT} immediate={create_leader={class=commander effect={save_event_target_as=face}}}}",
    );
    let (code, r) = invoke(&f, "wrong-leader-trigger", &[]);
    assert_eq!(code, 1, "{r}");
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["key"] == "scope"),
        "{r}"
    );
}
#[test]
fn cli_display_phase_does_not_leak_future_or_conditional_writes() {
    let f = fixture("display-phase-negative-boundaries");
    write(&f.game.join("common/leader_classes/a.txt"), "commander={}");
    write(
        &f.game.join("localisation/english/a_l_english.yml"),
        "l_english:\n TT:0 \"Text\"\n",
    );
    let path = f.mod_path.join("events/a.txt");
    for (label, block) in [
        (
            "option",
            "option={create_leader={class=commander effect={save_event_target_as=face}}}",
        ),
        (
            "after",
            "after={create_leader={class=commander effect={save_event_target_as=face}}}",
        ),
    ] {
        write(
            &path,
            &format!(
                "country_event={{id=a picture_event_data={{portrait=event_target:face}} {block}}}"
            ),
        );
        let (code, r) = invoke(&f, label, &[]);
        assert_eq!(code, 2, "{r}");
        assert!(
            r["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["reason"]["code"] == "target-binding-unproven"),
            "{r}"
        );
        assert!(
            r["saved_target_type_evidence"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["kind"] == "saved-target-type-candidates"
                    && v["target"] == "event_target:face"
                    && v["closed_static_writer_set"] == true
                    && v["candidate_types"]["known"] == serde_json::json!(["leader"])
                    && v["availability_proven"] == false),
            "{r}"
        );
    }
    write(
        &path,
        "country_event={id=a desc={trigger={event_target:face={has_leader_flag=ready}} text=TT} immediate={random={chance=50 create_leader={class=commander effect={save_event_target_as=face}}}}}",
    );
    let (code, r) = invoke(&f, "conditional", &[]);
    assert_eq!(code, 2, "{r}");
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["reason"]["code"] == "target-binding-unproven"),
        "{r}"
    );
}
#[test]
fn cli_guarded_type_arms_match_vanilla_contract_and_keep_static_negatives() {
    let f = fixture("guarded-type-arm-control");
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a trigger={OR={AND={is_scope_type=planet is_star=yes} AND={is_scope_type=country is_ai=no}}}}",
    );
    let (code, r) = invoke(&f, "positive", &[]);
    assert_eq!(code, 0, "{r}");
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a trigger={OR={AND={is_scope_type=planet has_ethic=missing_ethic is_star=maybe} AND={is_scope_type=country relative_power={who=this vlaue=superior}}}}}",
    );
    let (code, r) = invoke(&f, "negative", &[]);
    assert_eq!(code, 1, "{r}");
    for key in ["missing-item", "value", "unknown-field"] {
        assert!(
            r["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["key"] == key),
            "{key}: {r}"
        );
    }
}
#[test]
fn cli_compound_scripted_guards_propagate_across_event_dispatch() {
    let f = fixture("compound-guard-event-dispatch");
    write(
        &f.mod_path.join("common/scripted_triggers/guards.txt"),
        "have_target={exists=event_target:t is_scope_type=country} alias_guard={have_target=yes}",
    );
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a immediate={random={chance=50 save_event_target_as=t} country_event={id=b}}} country_event={id=b is_triggered_only=yes immediate={if={limit={alias_guard=yes is_ai=no} event_target:t={set_country_flag=checked}}}}",
    );
    let (code, r) = invoke(&f, "positive", &[]);
    assert_eq!(code, 0, "{r}");
    assert!(
        r["checked_statements"]["event_evaluations"]
            .as_u64()
            .unwrap()
            >= 2
    );
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a immediate={random_owned_planet={save_event_target_as=t} country_event={id=b}}} country_event={id=b is_triggered_only=yes immediate={if={limit={alias_guard=yes is_ai=no} event_target:t={set_country_flag=wrong}}}}",
    );
    let (code, r) = invoke(&f, "wrong-target-type", &[]);
    assert_eq!(code, 1, "{r}");
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["key"] == "scope"),
        "{r}"
    );
}
#[test]
fn cli_guard_alternatives_keep_optional_and_outside_uses_unproven() {
    let f = fixture("compound-guard-negative-control");
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a immediate={random={chance=50 save_event_target_as=t} if={limit={OR={exists=event_target:t is_ai=no}} event_target:t={set_country_flag=conditional}} event_target:t={set_country_flag=outside}}}",
    );
    let (code, r) = invoke(&f, "optional", &[]);
    assert_eq!(code, 2, "{r}");
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["reason"]["code"] == "target-binding-unproven"),
        "{r}"
    );
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a immediate={if={limit={AND={always=yes always=yes}} set_planet_flag=wrong}}}",
    );
    let (code, r) = invoke(&f, "wrong-scope", &[]);
    assert_eq!(code, 1, "{r}");
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["key"] == "scope"),
        "{r}"
    );
}
#[test]
fn cli_changed_flags_validate_resources_and_keep_two_color_contract() {
    let f = fixture("changed-country-flag");
    write(&f.game.join("flags/colors.txt"), "colors={red={} blue={}}");
    write_texture(&f.game.join("flags/special/emblem.dds"), b"emblem");
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a immediate={change_country_flag={icon={category=special file=emblem.dds} colors={red blue}} change_country_flag={colors={red blue null null}}}} ",
    );
    let (code, r) = invoke(&f, "positive", &[]);
    assert_eq!(code, 0, "{r}");
    assert!(
        r["input_manifest"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["relative_path"] == "flags/special/emblem.dds")
    );
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a immediate={while={count=0 limit={always=yes} change_country_flag={icon={category=special file=missing.dds} colors={red missing}}}}} planet_event={id=b immediate={change_country_flag=random}}",
    );
    let (code, r) = invoke(&f, "negative", &[]);
    assert_eq!(code, 1, "{r}");
    for key in ["missing-item", "scope"] {
        assert!(
            r["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["key"] == key),
            "{key}: {r}"
        );
    }
}
#[test]
fn cli_fleet_settings_and_ambient_ranges_validate_nested_grammar() {
    let f = fixture("fleet-and-ambient-parameters");
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a immediate={create_fleet={settings={can_upgrade=no garrison=yes spawn_debris=no is_boss=yes} effect={set_fleet_settings={can_change_leader=no} set_fleet_flag=created}} create_ambient_object={type=fixture entity_offset={min=-100 max=10000} entity_offset_height=-5 use_3d_location=yes entity_face_object=root}}}",
    );
    let (code, r) = invoke(&f, "positive", &[]);
    assert_eq!(code, 0, "{r}");
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a immediate={create_fleet={settings={garrison=maybe} effect={set_country_flag=wrong}} while={count=0 limit={always=yes} create_ambient_object={type=fixture entity_offset={min=100 max=10} entity_offset_angle={mni=1 max=2}}}}}",
    );
    let (code, r) = invoke(&f, "negative", &[]);
    assert_eq!(code, 1, "{r}");
    for key in ["scope", "value", "range-order", "unknown-field"] {
        assert!(
            r["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["key"] == key),
            "{key}: {r}"
        );
    }
}
#[test]
fn cli_flag_and_room_resources_check_files_colors_and_dds_headers() {
    let f = fixture("flag-room-assets");
    write(&f.game.join("flags/colors.txt"), "colors={red={} blue={}}");
    for rel in [
        "flags/special/emblem.dds",
        "flags/backgrounds/solid.dds",
        "gfx/portraits/city_sets/fixture_room.dds",
    ] {
        write_texture(&f.game.join(rel), b"fixture");
    }
    let event = "country_event={id=a picture_event_data={room=fixture_room} immediate={create_country={flag={icon={category=special file=emblem.dds} background={category=backgrounds file=solid.dds} colors={red blue null null}}}}}";
    write(&f.mod_path.join("events/a.txt"), event);
    let (code, r) = invoke(&f, "positive", &[]);
    assert_eq!(code, 0, "{r}");
    assert_eq!(
        r["input_manifest"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["relative_path"] == "flags/colors.txt")
            .count(),
        1
    );
    std::fs::remove_file(f.game.join("flags/special/emblem.dds")).unwrap();
    let (code, r) = invoke(&f, "missing", &[]);
    assert_eq!(code, 1, "{r}");
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["key"] == "missing-item"),
        "{r}"
    );
    write(&f.game.join("flags/special/emblem.dds"), "DDS truncated");
    let (code, r) = invoke(&f, "bad-header", &[]);
    assert_eq!(code, 1, "{r}");
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["key"] == "dds-header"),
        "{r}"
    );
    write_texture(&f.game.join("flags/special/emblem.dds"), b"fixture");
    write(&f.mod_path.join("flags/colors.txt"), "colors={green={}}");
    let (code, r) = invoke(&f, "colors-overridden", &[]);
    assert_eq!(code, 1, "{r}");
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["key"] == "missing-item"
                && d["message"].as_str().unwrap().contains("flag_colors")),
        "{r}"
    );
    std::fs::remove_file(f.mod_path.join("flags/colors.txt")).unwrap();
    std::fs::remove_file(f.game.join("gfx/portraits/city_sets/fixture_room.dds")).unwrap();
    let (code, r) = invoke(&f, "missing-room", &[]);
    assert_eq!(code, 1, "{r}");
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["message"].as_str().unwrap().contains("fixture_room.dds")),
        "{r}"
    );
}
#[test]
fn cli_graphics_relative_resources_follow_overlay_and_replace_path() {
    let f = fixture("graphics-relative-assets");
    write(
        &f.mod_path.join("gfx/models/pack/model.asset"),
        "pdxmesh={name=fixture_mesh file=local.mesh diffuse=grey.dds} entity={name=fixture_entity pdxmesh=fixture_mesh}",
    );
    write(
        &f.game.join("gfx/models/pack/local.mesh"),
        "base mesh bytes",
    );
    write(
        &f.mod_path.join("gfx/models/pack/local.mesh"),
        "override mesh bytes",
    );
    write_texture(&f.game.join("gfx/models/grey.dds"), b"shared placeholder");
    let (code, r) = invoke(&f, "positive", &[]);
    assert_eq!(code, 2, "{r}");
    assert_eq!(r["errors"], 0);
    let mesh = r["input_manifest"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["relative_path"] == "gfx/models/pack/local.mesh")
        .unwrap();
    assert_eq!(mesh["layer"], 1);
    assert_eq!(mesh["sha256"], sha256(b"override mesh bytes"));
    assert!(
        r["input_manifest"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["relative_path"] == "gfx/models/grey.dds")
    );
    std::fs::remove_file(f.mod_path.join("gfx/models/pack/local.mesh")).unwrap();
    write(
        &f.mod_path.join("descriptor.mod"),
        "replace_path=\"gfx/models/pack\"",
    );
    let (code, r) = invoke(&f, "hidden-sibling", &[]);
    assert_eq!(code, 1, "{r}");
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["key"] == "missing-item"
                && d["message"].as_str().unwrap().contains("local.mesh")),
        "{r}"
    );
    write(
        &f.mod_path.join("gfx/models/pack/local.mesh"),
        "restored mesh bytes",
    );
    write(
        &f.mod_path.join("gfx/models/pack/model.asset"),
        "pdxmesh={name=fixture_mesh file=local.mesh diffuse=missing.dds}",
    );
    let (code, r) = invoke(&f, "missing-texture", &[]);
    assert_eq!(code, 1, "{r}");
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["message"].as_str().unwrap().contains("missing.dds")),
        "{r}"
    );
}
#[test]
fn cli_job_swap_icon_uses_effective_asset_provider_and_records_hash() {
    let f = fixture("job-swap-resource-overlay");
    let rel = "gfx/interface/icons/buildings/legacy_icon.dds";
    // Basic DDS headers exercise static structure, not texture payload/rendering.
    write_texture(&f.game.join(rel), b"base icon bytes");
    write_texture(&f.mod_path.join(rel), b"override icon bytes");
    write(
        &f.mod_path.join("common/pop_jobs/job.txt"),
        "job={possible_pre_triggers={has_owner=yes} swappable_data={default={building_icon=legacy_icon} swap_type={building_icon=legacy_icon trigger={has_planet_flag=eligible owner={is_ai=no}}}}}",
    );
    let (code, r) = invoke(&f, "override", &[]);
    assert_eq!(code, 0, "{r}");
    let assets = r["input_manifest"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| v["relative_path"] == rel)
        .collect::<Vec<_>>();
    assert_eq!(assets.len(), 1, "{r}");
    assert_eq!(assets[0]["layer"], 1);
    assert_eq!(
        assets[0]["sha256"],
        sha256(&texture_bytes(b"override icon bytes"))
    );
    std::fs::remove_file(f.mod_path.join(rel)).unwrap();
    let (code, r) = invoke(&f, "base", &[]);
    assert_eq!(code, 0, "{r}");
    let asset = r["input_manifest"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["relative_path"] == rel)
        .unwrap();
    assert_eq!(asset["layer"], 0);
    assert_eq!(asset["sha256"], sha256(&texture_bytes(b"base icon bytes")));
    write(
        &f.mod_path.join("descriptor.mod"),
        "replace_path=\"gfx/interface/icons/buildings\"",
    );
    let (code, r) = invoke(&f, "hidden", &[]);
    assert_eq!(code, 1, "{r}");
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["key"] == "missing-item"),
        "{r}"
    );
    write_texture(&f.mod_path.join(rel), b"replacement bytes");
    let (code, r) = invoke(&f, "replacement", &[]);
    assert_eq!(code, 0, "{r}");
    assert_eq!(
        r["input_manifest"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["relative_path"] == rel)
            .unwrap()["sha256"],
        sha256(&texture_bytes(b"replacement bytes"))
    );
}
#[test]
fn cli_simulation_declarations_reject_bad_fields_and_scope_stacks() {
    let f = fixture("simulation-declarations");
    write(
        &f.mod_path.join("common/megastructures/mega.txt"),
        "mega={entity_offset={x=-3 y=4} potential={from={if={limit={is_scope_type=megastructure} has_megastructure_flag=upgrade}}} placement_rules={planet_possible={is_star=yes prev={has_star_flag=system} from={is_ai=no}}} on_build_complete={set_star_flag=complete from={set_country_flag=builder} fromfrom={set_megastructure_flag=done} fromfromfrom={set_fleet_flag=constructor}}}",
    );
    let (code, r) = invoke(&f, "positive", &[]);
    assert_eq!(code, 0, "{r}");
    write(
        &f.mod_path.join("common/megastructures/mega.txt"),
        "mega={entity_offset={w=3} placement_rules={planet_possible={prev={is_ai=no}}} on_build_complete={fromfrom={set_country_flag=wrong}}}",
    );
    write(
        &f.mod_path.join("common/pop_jobs/job.txt"),
        "job={possible_pre_triggers={is_robotci=no is_robotic=maybe} swappable_data={swap_type={trigger={is_ai=no} buidling_icon=wrong}}}",
    );
    let (code, r) = invoke(&f, "negative", &[]);
    assert_eq!(code, 1, "{r}");
    for key in ["scope", "unknown-field", "value"] {
        assert!(
            r["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["key"] == key),
            "{key}: {r}"
        );
    }
}
#[test]
fn cli_typed_parameter_contracts_validate_legal_forms_and_reject_errors() {
    let f = fixture("typed-parameter-contracts");
    for (path, text) in [
        ("common/strategic_resources/a.txt", "alloys={} nanites={}"),
        ("common/governments/civics/a.txt", "civic_ok={}"),
        ("common/governments/authorities/a.txt", "auth_ok={}"),
        ("common/traits/a.txt", "leader_trait_ok={} trait_ok={}"),
        ("common/leader_tiers/a.txt", "tier_ok={}"),
        ("common/planet_classes/a.txt", "pc_ok={}"),
        (
            "events/base_known.txt",
            "country_event={id=a is_triggered_only=yes}",
        ),
        (
            "localisation/english/a_l_english.yml",
            "l_english:\n TT:0 \"Text\"\n MSG:0 \"Message\"\n",
        ),
    ] {
        write(&f.game.join(path), text);
    }
    let path = f.mod_path.join("events/a.txt");
    write(
        &path,
        "country_event={id=positive trigger={success_text={text=TT is_ai=no} custom_tooltip_fail={text=TT is_ai=no} calc_true_if={amount>=1 always=yes is_ai=no} has_active_event={a}} immediate={create_message={type=BYPASS_EXPLORED localization=MSG target=this variable={type=name localization=SYSTEM scope=root} variable={key=border value=GFX_border}} add_resource={alloys=100000 nanites=1 mult=2} change_government={authority=auth_ok civics={civic=civic_ok} cooldown=no} create_leader={traits={0=leader_trait_ok non_integer=leader_trait_ok} tier=tier_ok} create_species={traits={trait=trait_ok ideal_planet_class=pc_ok}}}} ",
    );
    let (code, r) = invoke(&f, "positive", &[]);
    assert_eq!(code, 0, "{r}");
    write(
        &path,
        "country_event={id=negative trigger={success_text={text=TT is_star=yes} calc_true_if={is_ai=no} has_active_event={id=a}} immediate={create_message={variable={type=name localization=SYSTEM scpoe=this}} add_resource={allosy=1} change_government={authority=missing} create_leader={traits={0=leader_trait_missing}} create_species={traits={trait=missing}}}}",
    );
    let (code, r) = invoke(&f, "negative", &[]);
    assert_eq!(code, 1, "{r}");
    for key in [
        "scope",
        "missing-field",
        "structure",
        "unknown-field",
        "missing-item",
    ] {
        assert!(
            r["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["key"] == key),
            "{key}: {r}"
        );
    }
}
#[test]
fn cli_parameter_static_shapes_are_checked_in_unreachable_effects() {
    let f = fixture("parameter-static-dead-branch");
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a immediate={while={count=0 limit={always=yes} create_message={variable={key=missing_value}} change_government={cooldown=maybe} add_resource={alloys={bad=yes}}}}}",
    );
    let (code, r) = invoke(&f, "negative", &[]);
    assert_eq!(code, 1, "{r}");
    for key in ["missing-field", "value", "structure"] {
        assert!(
            r["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["key"] == key),
            "{key}: {r}"
        );
    }
}
#[test]
fn cli_ship_declaration_wrong_shapes_are_errors() {
    for body in [
        "section_slots=yes",
        "section_slots={mid={locator={bad=yes}}}",
        "required_component_set={bad=yes}",
        "upgrades_to={bad=yes}",
        "min_upgrade_cost=1",
    ] {
        let f = fixture("ship-declaration-shape");
        write(
            &f.mod_path.join("common/ship_sizes/a.txt"),
            &format!("hull={{{body}}}"),
        );
        let (code, r) = invoke(&f, "negative", &[]);
        assert_eq!(code, 1, "{body}: {r}");
        assert!(
            r["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["key"] == "structure"),
            "{body}: {r}"
        );
    }
}
#[test]
fn cli_required_set_proof_uses_hull_country_condition_and_retains_unknown() {
    let f = fixture("required-set-proof");
    write(
        &f.game.join("common/component_sets/a.txt"),
        "component_set={key=reactor}",
    );
    write(
        &f.mod_path.join("common/ship_sizes/a.txt"),
        "hull={potential_country={is_ai=no} required_component_set=reactor}",
    );
    let path = f.game.join("common/component_templates/a.txt");
    write(
        &path,
        "utility_component_template={key=core component_set=reactor potential={is_ship_size=hull from={is_ai=no}}}",
    );
    let (code, r) = invoke(&f, "covered", &[]);
    assert_eq!(code, 0, "{r}");
    assert_eq!(r["component_potential_proofs"][0]["status"], "covered");
    assert!(
        r["component_potential_proofs"][0]["proof_domain"]
            .as_str()
            .unwrap()
            .contains("not proved")
    );
    write(
        &path,
        "utility_component_template={key=core component_set=reactor potential={is_ship_size=hull from={is_ai=yes}}}",
    );
    let (code, r) = invoke(&f, "rejected-potential", &[]);
    assert_eq!(code, 2, "{r}");
    assert_eq!(r["component_potential_proofs"][0]["status"], "impossible");
    assert_eq!(r["errors"], 0);
    write(
        &path,
        "utility_component_template={key=core component_set=reactor potential={is_ship_size=hull from={has_country_flag=unresolved}}}",
    );
    let (code, r) = invoke(&f, "unknown", &[]);
    assert_eq!(code, 2, "{r}");
    assert_eq!(r["component_potential_proofs"][0]["status"], "unknown");
}
#[test]
fn cli_ship_declarations_check_slots_sets_and_upgrade_cycles_without_design() {
    let f = fixture("ship-declarations");
    write(
        &f.mod_path.join("common/component_sets/a.txt"),
        "component_set={key=reactor required_component_set=yes is_core_component_set=yes}",
    );
    write(&f.game.join("common/ship_sizes/other.txt"), "other={}");
    let component_path = f.game.join("common/component_templates/a.txt");
    write(
        &component_path,
        "utility_component_template={key=core component_set=reactor potential={is_ship_size=hull}}",
    );
    let hull = f.mod_path.join("common/ship_sizes/a.txt");
    let section = f.mod_path.join("common/section_templates/a.txt");
    let slots = f.mod_path.join("common/component_slot_templates/a.txt");
    write(
        &hull,
        "hull={section_slots={mid={template=section locator=mid}} required_component_set=reactor}",
    );
    write(
        &section,
        "ship_section_template={key=section ship_size=hull fits_on_slot=mid component_slot={name=GUN template=small_turret rotation=90 position={x=0 y=1 z=2}}}",
    );
    write(
        &slots,
        "small_turret={size=small component=weapon is_fixed=yes}",
    );
    let (code, r) = invoke(&f, "positive", &[]);
    assert_eq!(code, 0, "{r}");
    assert_eq!(r["checked_statements"]["explicit_ship_designs"], 0);
    write(
        &hull,
        "hull={section_slots={mid={template=section}} required_component_set=reactor upgrades_to=hull}",
    );
    write(
        &section,
        "ship_section_template={key=section ship_size=hull fits_on_slot=stern component_slot={name=GUN template=missing position={q=1}}}",
    );
    write(&slots, "small_turret={size=small component=wepon}");
    write(
        &component_path,
        "utility_component_template={key=core component_set=reactor potential={is_ship_size=other}}",
    );
    let (code, r) = invoke(&f, "negative", &[]);
    assert_eq!(code, 1, "{r}");
    for key in [
        "ship-compatibility",
        "upgrade-cycle",
        "unknown-field",
        "missing-item",
        "value",
    ] {
        assert!(
            r["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["key"] == key),
            "{key}: {r}"
        );
    }
}
#[test]
fn cli_project_requirements_preserve_typed_context_and_references() {
    let f = fixture("project-requirements");
    write(&f.game.join("common/ship_sizes/a.txt"), "science={}");
    write(&f.game.join("common/leader_classes/a.txt"), "scientist={}");
    write(
        &f.game.join("localisation/english/a_l_english.yml"),
        "l_english:\n loc:0 \"Text\"\n",
    );
    let path = f.mod_path.join("common/special_projects/a.txt");
    write(
        &path,
        "special_project={key=P event_scope=ship_event requirements={shipclass_science_ship=1 leader=scientist size_requirement={ship_size=science count=1}} triggered_requirement={text=loc count=1 trigger={is_mobile=yes}}}",
    );
    let (code, r) = invoke(&f, "positive", &[]);
    assert_eq!(code, 0, "{r}");
    write(
        &path,
        "special_project={key=P event_scope=ship_event requirements={size_requirement={ship_size=missing count={x=y}} shipclass_science_shpi=1} triggered_requirement={text=loc trigger={is_star=yes}}}",
    );
    let (code, r) = invoke(&f, "negative", &[]);
    assert_eq!(code, 1, "{r}");
    for key in ["missing-item", "structure", "unknown-field", "scope"] {
        assert!(
            r["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["key"] == key),
            "{key}: {r}"
        );
    }
}
#[test]
fn cli_event_from_override_keeps_fromfrom_and_rejects_wrong_scope() {
    let f = fixture("from-stack-override");
    let path = f.mod_path.join("events/a.txt");
    let text = "planet_event={id=a immediate={every_country={country_event={id=b}}}} country_event={id=b is_triggered_only=yes immediate={country_event={id=c scopes={from=root}}}} country_event={id=c is_triggered_only=yes immediate={from={set_country_flag=ok} fromfrom={set_planet_flag=ok}}}";
    write(&path, text);
    let (code, r) = invoke(&f, "positive", &[]);
    assert_eq!(code, 0, "{r}");
    write(
        &path,
        &text.replace(
            "fromfrom={set_planet_flag=ok}",
            "fromfrom={set_country_flag=bad}",
        ),
    );
    let (code, r) = invoke(&f, "negative", &[]);
    assert_eq!(code, 1, "{r}");
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["key"] == "scope")
    );
}
#[test]
fn cli_event_display_inline_positive_and_negative() {
    let f = fixture("event-display");
    write(
        &f.game.join("localisation/english/test_l_english.yml"),
        "l_english:\n loc:0 \"Text\"\n",
    );
    write(
        &f.game.join("interface/test.gfx"),
        "spriteTypes={spriteType={name=GFX_good texturefile=good.dds}}",
    );
    write(&f.game.join("common/event_chains/a.txt"), "good_chain={}");
    write(&f.game.join("common/specimens/a.txt"), "good_specimen={}");
    write(
        &f.mod_path.join("common/inline_scripts/picture.txt"),
        "trigger={is_ai=$AI$} picture=GFX_good",
    );
    write(
        &f.mod_path.join("common/inline_scripts/desc.txt"),
        "if={limit={is_ai=no} text=loc} else={text=loc}",
    );
    let path = f.mod_path.join("events/a.txt");
    write(
        &path,
        "country_event={id=a event_chain=good_chain specimen=good_specimen force_open=yes picture={inline_script={script=picture AI=no}} desc={inline_script={script=desc}} option={name=loc icon={icon=GFX_good icon_background=GFX_good text=loc}}}",
    );
    let (code, r) = invoke(&f, "positive", &[]);
    assert_eq!(code, 0, "{r}");
    assert!(!r["diagnostics"].as_array().unwrap().iter().any(|d| {
        d["message"]
            .as_str()
            .unwrap()
            .contains("event display subgrammar")
    }));
    write(
        &path,
        "country_event={id=a event_chain=missing picture={trigger={is_star=yes} picture=GFX_missing} desc={if={text=loc} txet=loc} option={icon={icon_background={wrong=yes}}}}",
    );
    let (code, r) = invoke(&f, "negative", &[]);
    assert_eq!(code, 1, "{r}");
    for key in [
        "missing-item",
        "scope",
        "missing-field",
        "unknown-field",
        "structure",
    ] {
        assert!(
            r["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["key"] == key),
            "{key}: {r}"
        );
    }
}
struct Fixture {
    root: PathBuf,
    game: PathBuf,
    mod_path: PathBuf,
}
fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}
fn fixture(name: &str) -> Fixture {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../verification/rust-core-01/cli-fixtures");
    let root = base.join(format!(
        "{name}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::SeqCst)
    ));
    let game = root.join("game");
    let mod_path = root.join("mod");
    write(
        &game.join("launcher-settings.json"),
        r#"{"gameId":"stellaris","rawVersion":"v4.5.2"}"#,
    );
    std::fs::create_dir_all(&mod_path).unwrap();
    Fixture {
        root,
        game,
        mod_path,
    }
}
fn invoke(f: &Fixture, label: &str, extra: &[&str]) -> (i32, Value) {
    let exe = std::env::var_os("TIGER_STELLAR_TEST_EXE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_tiger-stellar")));
    let output = f.root.join(format!("{label}.json"));
    let mut command = Command::new(&exe);
    command
        .args(["--game"])
        .arg(&f.game)
        .arg("--mod")
        .arg(&f.mod_path)
        .arg("--output")
        .arg(&output)
        .args(extra);
    command.env("PATH", "");
    let started = std::time::Instant::now();
    let result = command.output().unwrap();
    let wall = started.elapsed().as_secs_f64();
    std::fs::write(f.root.join(format!("{label}.stdout.txt")), &result.stdout).unwrap();
    std::fs::write(f.root.join(format!("{label}.stderr.txt")), &result.stderr).unwrap();
    let report: Value = serde_json::from_slice(&std::fs::read(&output).unwrap()).unwrap();
    let binary = sha256(&std::fs::read(&exe).unwrap());
    if report["status"] != "infrastructure_error" {
        assert_eq!(report["binding"]["implementation"]["binary_sha256"], binary);
        assert_eq!(
            report["binding"]["implementation"]["source_sha256"],
            env!("TIGER_STELLAR_SOURCE_SHA256")
        );
    }
    let record = json!({"exe":exe,"binary_sha256":binary,"args":command.get_args().map(|a|a.to_string_lossy().to_string()).collect::<Vec<_>>(),"exit_code":result.status.code(),"wall_seconds":wall,"report_sha256":sha256(&std::fs::read(&output).unwrap()),"report":output,"engine_not_run":true});
    std::fs::write(
        f.root.join(format!("{label}.invocation.json")),
        serde_json::to_vec_pretty(&record).unwrap(),
    )
    .unwrap();
    (result.status.code().unwrap_or(-1), report)
}
#[test]
fn cli_clean_without_python_or_path() {
    let f = fixture("clean-中文");
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a immediate={save_event_target_as=t event_target:t={set_country_flag=ok}}}",
    );
    let (code, r) = invoke(&f, "native", &[]);
    assert_eq!(code, 0, "{r}");
    assert_eq!(r["errors"], 0);
    assert_eq!(r["runtime"], "native Rust; no Python semantic runtime");
}
#[test]
fn cli_wrong_scope_after_forty_branches() {
    let f = fixture("invalid");
    let noise = (0..40)
        .map(|i| format!("random={{chance=50 save_event_target_as=x{i}}} "))
        .collect::<String>();
    write(
        &f.mod_path.join("events/a.txt"),
        &format!(
            "country_event={{id=a immediate={{random={{chance=50 save_event_target_as=t}} {noise} if={{limit={{exists=event_target:t}} event_target:t={{set_planet_flag=bad}}}}}}}}"
        ),
    );
    let (code, r) = invoke(&f, "native", &[]);
    assert_eq!(code, 1);
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["key"] == "scope" && d["severity"] == "error")
    );
    assert_eq!(r["checked_statements"]["enumerated_binding_paths"], 0);
}
#[test]
fn cli_repeated_stable_guard_is_clean() {
    let f = fixture("guard");
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a immediate={if={limit={is_ai=yes} capital_scope={save_event_target_as=t}} if={limit={is_ai=yes} event_target:t={set_planet_flag=ok}}}}",
    );
    let (code, r) = invoke(&f, "native", &[]);
    assert_eq!(code, 0, "{r}");
}
#[test]
fn cli_version_mismatch_warns_and_continues() {
    let f = fixture("version");
    write(
        &f.game.join("launcher-settings.json"),
        r#"{"gameId":"stellaris","rawVersion":"v99.0"}"#,
    );
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a immediate={set_planet_flag=bad}}",
    );
    let (code, r) = invoke(&f, "native", &[]);
    assert_eq!(code, 1);
    let d = r["diagnostics"].as_array().unwrap();
    assert!(
        d.iter()
            .any(|d| d["key"] == "version-mismatch" && d["severity"] == "warning")
    );
    assert!(
        d.iter()
            .any(|d| d["key"] == "scope" && d["severity"] == "error")
    );
}
#[test]
fn cli_empty_input_cannot_pass() {
    let f = fixture("empty");
    let (code, r) = invoke(&f, "native", &[]);
    assert_eq!(code, 2);
    assert_eq!(r["target_files"], 0);
    assert_eq!(r["status"], "empty_input_not_validation");
}
#[test]
fn cli_wrong_game_rejected() {
    let f = fixture("wrong-game");
    write(
        &f.game.join("launcher-settings.json"),
        r#"{"gameId":"ck3","rawVersion":"1.20.1"}"#,
    );
    let (code, r) = invoke(&f, "native", &[]);
    assert_eq!(code, 3);
    assert!(r["message"].as_str().unwrap().contains("wrong-game"));
}
#[test]
fn cli_layer_order_changes_final_provider() {
    let f = fixture("overlays");
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a immediate={country_event={id=b}}}",
    );
    let a = f.root.join("depA");
    let b = f.root.join("depB");
    write(
        &a.join("events/b.txt"),
        "country_event={id=b is_triggered_only=yes immediate={set_country_flag=ok}}",
    );
    write(
        &b.join("events/b.txt"),
        "country_event={id=b is_triggered_only=yes immediate={set_planet_flag=bad}}",
    );
    let a = a.to_str().unwrap();
    let b = b.to_str().unwrap();
    let (bad, r) = invoke(&f, "a-then-b", &["--dependency", a, "--dependency", b]);
    assert_eq!(bad, 1, "{r}");
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["path"].as_str().is_some_and(|p| p.contains("depB")))
    );
    let (good, r) = invoke(&f, "b-then-a", &["--dependency", b, "--dependency", a]);
    assert_eq!(good, 0, "{r}");
}
#[test]
fn cli_replace_path_removes_hidden_definition() {
    let f = fixture("replace");
    write(
        &f.game.join("events/hidden.txt"),
        "country_event={id=hidden is_triggered_only=yes}",
    );
    write(
        &f.mod_path.join("descriptor.mod"),
        "replace_path=\"events\"",
    );
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a immediate={country_event={id=hidden}}}",
    );
    let (code, r) = invoke(&f, "native", &[]);
    assert_eq!(code, 1);
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["key"] == "missing-event")
    );
}
#[test]
fn cli_local_reader_constant_reaches_inline() {
    let f = fixture("reader");
    write(&f.game.join("common/scripted_variables/a.txt"), "@amount=1");
    write(
        &f.mod_path.join("events/a.txt"),
        "@amount=7\ncountry_event={id=a immediate={inline_script=a}}",
    );
    write(
        &f.mod_path.join("common/inline_scripts/a.txt"),
        "set_variable={which=x value=@amount}",
    );
    let (code, r) = invoke(&f, "native", &[]);
    assert_eq!(code, 0, "{r}");
}
#[test]
fn cli_cache_and_repeat_stability() {
    let f = fixture("cache");
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a immediate={set_planet_flag=bad}}",
    );
    let cache = f.root.join("cache");
    let flags = ["--cache", cache.to_str().unwrap()];
    let (_, first) = invoke(&f, "cold", &flags);
    let (_, second) = invoke(&f, "warm", &flags);
    assert_eq!(first["diagnostics"], second["diagnostics"]);
    assert_eq!(first["input_manifest"], second["input_manifest"]);
    assert_eq!(first["checked_statements"], second["checked_statements"]);
    assert!(second["parser"]["cache_hits"].as_u64().unwrap() > 0);
    let cp = std::fs::read_dir(&cache)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|s| s == "json"))
        .unwrap();
    std::fs::write(cp, b"{}").unwrap();
    let (_, third) = invoke(&f, "corrupt-recovered", &flags);
    assert_eq!(first["diagnostics"], third["diagnostics"]);
    assert!(third["parser"]["uncached_parses"].as_u64().unwrap() > 0);
}
#[test]
fn cli_worker_recycles_without_losing_late_file() {
    let f = fixture("worker-recycle");
    for i in 0..103 {
        write(
            &f.mod_path.join(format!("events/{i:03}.txt")),
            &format!("country_event={{id=a{i} immediate={{set_country_flag=ok}}}}"),
        );
    }
    write(
        &f.mod_path.join("events/zzz.txt"),
        "country_event={id=last immediate={set_planet_flag=bad}}",
    );
    let (code, r) = invoke(&f, "native", &[]);
    assert_eq!(code, 1);
    assert_eq!(r["target_files"], 104);
    assert_eq!(r["parser"]["worker_starts"], 2);
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["path"].as_str().unwrap().ends_with("zzz.txt"))
    );
}

#[test]
fn cli_macro_reader_headers_survive_worker_recycle_and_keep_local_shadow() {
    let f = fixture("macro-header-worker-recycle");
    write(
        &f.game.join("common/scripted_variables/global.txt"),
        "@ZERO=9",
    );
    write(
        &f.mod_path.join("common/scripted_effects/header.txt"),
        "@ZERO=0\nset_number={set_variable={which=x value=$N$} set_variable={which=divisor value=@ZERO}}",
    );
    let mut calls = String::new();
    for value in 0..106 {
        calls.push_str(&format!("set_number={{N={value}}} "));
    }
    write(
        &f.mod_path.join("events/a.txt"),
        &format!(
            "country_event={{id=a immediate={{{calls} divide_variable={{which=x value=divisor}}}}}}"
        ),
    );
    let (code, r) = invoke(&f, "recycled-header", &[]);
    assert_eq!(code, 1, "{r}");
    assert_eq!(r["warnings"], 0, "{r}");
    assert_eq!(r["errors"], 1, "{r}");
    assert!(r["parser"]["worker_starts"].as_u64().unwrap() >= 2, "{r}");
    assert!(
        r["checked_statements"]["script_body_compilations"]
            .as_u64()
            .unwrap()
            >= 106,
        "{r}"
    );
    assert!(
        r["diagnostics"].as_array().unwrap().iter().any(|d| {
            d["key"] == "value" && d["message"].as_str().unwrap().contains("must not be zero")
        }),
        "{r}"
    );
}
#[test]
fn cli_selected_language_is_required() {
    let f = fixture("language");
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a title=only_english}",
    );
    write(
        &f.mod_path.join("localisation/english/a.yml"),
        "l_english:\n only_english:0 \"English\"\n",
    );
    let (code, r) = invoke(&f, "native", &["--language", "simp_chinese"]);
    assert_eq!(code, 2);
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["key"] == "localization")
    );
}

#[test]
fn cli_root_template_is_not_zero_work_success() {
    let f = fixture("root-template");
    write(&f.mod_path.join("events/a.txt"), "inline_script=event");
    write(
        &f.mod_path.join("common/inline_scripts/event.txt"),
        "country_event={id=a immediate={set_planet_flag=bad}}",
    );
    let (code, r) = invoke(&f, "native", &[]);
    assert_eq!(code, 1, "{r}");
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["key"] == "scope")
    );
}

#[test]
fn cli_nested_inline_keeps_caller_reader_context() {
    let f = fixture("nested-reader");
    write(&f.game.join("common/scripted_variables/a.txt"), "@amount=1");
    write(
        &f.mod_path.join("events/a.txt"),
        "@amount=7\ncountry_event={id=a immediate={inline_script=a}}",
    );
    write(
        &f.mod_path.join("common/inline_scripts/a.txt"),
        "if={limit={always=yes} inline_script=b}",
    );
    write(
        &f.mod_path.join("common/inline_scripts/b.txt"),
        "set_variable={which=x value=@amount}",
    );
    let (code, r) = invoke(&f, "native", &[]);
    assert_eq!(code, 0, "{r}");
}

const NATIVE_CSV: &str = include_str!("fixtures/vanilla-red-laser.csv");
#[test]
fn cli_shared_reader_headers_keep_distinct_callers_and_nested_overrides() {
    let f = fixture("shared-reader-callers");
    write(&f.game.join("common/scripted_variables/a.txt"), "@amount=1");
    write(
        &f.mod_path.join("common/component_templates/a.txt"),
        "@amount=7\nweapon_component_template={key=A inline_script=a}",
    );
    write(
        &f.mod_path.join("common/component_templates/b.txt"),
        "@amount=11\nweapon_component_template={key=B inline_script=a}",
    );
    write(
        &f.mod_path.join("common/inline_scripts/a.txt"),
        "inline_script=b\ntotal_fire_time=1",
    );
    write(
        &f.mod_path.join("common/inline_scripts/b.txt"),
        "damage={min=@amount max=10}",
    );
    let (code, r) = invoke(&f, "different-local-values", &[]);
    assert_eq!(code, 1, "{r}");
    assert_eq!(r["errors"], 1, "{r}");
    let row = r["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["key"] == "range-order")
        .unwrap();
    for occurrence in row["occurrences"].as_array().unwrap() {
        let caller = occurrence["call_chain"]
            .as_array()
            .unwrap()
            .iter()
            .find(|frame| {
                frame["kind"] == "definition" && frame["category"] == "component_templates"
            })
            .unwrap()["path"]
            .as_str()
            .unwrap();
        assert!(
            caller.ends_with("b.txt"),
            "the valid caller must not borrow the failing caller's constant: {r}"
        );
    }
}
fn csv_fixture(name: &str, csv: &str) -> Fixture {
    let f = fixture(name);
    write(
        &f.game
            .join("common/component_templates/weapon_components.csv"),
        csv,
    );
    write(
        &f.mod_path.join("common/component_templates/a.txt"),
        "weapon_component_template={key=SMALL_RED_LASER}",
    );
    f
}
#[test]
fn cli_native_csv_contract_slice() {
    let f = csv_fixture("native-csv", NATIVE_CSV);
    let (code, r) = invoke(&f, "native", &[]);
    assert_eq!(code, 0, "{r}");
    assert!(
        r["input_manifest"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["relative_path"] == "common/component_templates/weapon_components.csv")
    );
}
#[test]
fn cli_csv_reversed_damage_is_error() {
    assert!(NATIVE_CSV.contains(";6.00;16.00;"));
    let data = NATIVE_CSV.replacen(";6.00;16.00;", ";20.00;10.00;", 1);
    let f = csv_fixture("csv-damage-invalid", &data);
    let (code, r) = invoke(&f, "native", &[]);
    assert_eq!(code, 1);
    assert!(r["diagnostics"].as_array().unwrap().iter().any(|d| {
        d["key"] == "range-order"
            && d["path"]
                .as_str()
                .unwrap()
                .ends_with("weapon_components.csv")
    }));
}
#[test]
fn cli_csv_override_checks_effective_cooldown() {
    let rows = tiger_stellar::csv::rows(NATIVE_CSV).unwrap();
    let header = rows[0].1.clone();
    let mut row = rows[1].1.clone();
    let index = header.iter().position(|s| s == "cooldown").unwrap();
    row[index] = "0".into();
    let csv = format!("{}\n{}\n", header.join(";"), row.join(";"));
    let f = csv_fixture("csv-override", &csv);
    write(
        &f.mod_path.join("common/component_templates/a.txt"),
        "weapon_component_template={key=SMALL_RED_LASER damage={min=1 max=2} total_fire_time=10}",
    );
    let (code, r) = invoke(&f, "native", &[]);
    assert_eq!(code, 1);
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["message"].as_str().unwrap().contains("CSV cooldown"))
    );
}
#[test]
fn cli_csv_replace_path_hides_lower_layer_stats() {
    let f = csv_fixture("csv-replace", NATIVE_CSV);
    write(
        &f.mod_path.join("descriptor.mod"),
        "replace_path=\"common/component_templates\"",
    );
    let (code, r) = invoke(&f, "native", &[]);
    assert_eq!(code, 2, "{r}");
    assert!(r["diagnostics"].as_array().unwrap().iter().any(|d| {
        d["message"]
            .as_str()
            .unwrap()
            .contains("no inline definition or resolved CSV")
    }));
}

#[test]
fn cli_uppercase_logic_root_and_from() {
    let f = fixture("logical-case");
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a trigger={OR={always=yes always=no} if={limit={is_ai=yes} has_country_flag=a} else={has_country_flag=b}} immediate={IF={limit={is_ai=yes} ROOT={set_country_flag=ok}} ELSE={set_country_flag=ok}}}",
    );
    write(
        &f.mod_path.join("common/component_templates/a.txt"),
        "utility_component_template={key=TEST potential={OR={FROM={is_ai=no} is_scope_type=design}}}",
    );
    let (code, r) = invoke(&f, "native", &[]);
    assert_eq!(code, 0, "{r}");
    assert_eq!(r["errors"], 0);
    assert_eq!(r["warnings"], 0);
}

#[test]
fn cli_cached_macro_rechecks_flow_and_retains_call_sites() {
    let f = fixture("macro-cache");
    write(
        &f.mod_path.join("common/scripted_effects/a.txt"),
        "check={event_target:$NAME$={set_country_flag=ok}}",
    );
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a immediate={save_event_target_as=t check={NAME=t}\ncapital_scope={save_event_target_as=t} check={NAME=t}\ncheck={NAME=t}}}",
    );
    let (code, r) = invoke(&f, "native", &[]);
    assert_eq!(code, 1, "{r}");
    assert_eq!(r["checked_statements"]["script_body_compilations"], 1);
    assert_eq!(r["checked_statements"]["script_body_cache_hits"], 2);
    let row = r["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["key"] == "scope")
        .unwrap();
    assert_eq!(row["occurrences"].as_array().unwrap().len(), 2);
    assert_eq!(row["occurrences_truncated"], false);
    let (_, repeated) = invoke(&f, "repeat", &[]);
    assert_eq!(r["diagnostics"], repeated["diagnostics"]);
}

#[test]
fn cli_crisis_and_name_list_legal_contracts() {
    let f = fixture("ported-contracts-good");
    write(
        &f.mod_path.join("common/crisis_levels/a.txt"),
        "level={allow={is_ai=no} on_unlock={set_country_flag=ok}}",
    );
    write(
        &f.mod_path.join("common/name_lists/a.txt"),
        "bio={trigger={is_ai=no} randomized=no should_name_home_system_planets=no}",
    );
    let (code, r) = invoke(&f, "native", &[]);
    assert_eq!(code, 0, "{r}");
}
#[test]
fn cli_crisis_and_name_list_invalid_contracts() {
    let f = fixture("ported-contracts-bad");
    write(
        &f.mod_path.join("common/crisis_levels/a.txt"),
        "level={on_unlock={set_planet_flag=bad} perks={missing}}",
    );
    write(
        &f.mod_path.join("common/name_lists/a.txt"),
        "bio={selectable={is_star=yes} randomized=maybe}",
    );
    let (code, r) = invoke(&f, "native", &[]);
    assert_eq!(code, 1, "{r}");
    for key in ["scope", "missing-item", "value"] {
        assert!(
            r["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["key"] == key)
        );
    }
    let (_, repeat) = invoke(&f, "repeat", &[]);
    assert_eq!(r["diagnostics"], repeat["diagnostics"]);
}
#[test]
fn cli_on_action_references_and_weights() {
    let f = fixture("onaction-contracts");
    write(
        &f.mod_path.join("common/on_actions/a.txt"),
        "on_test={events={missing.1} random_events={-1=a 0=missing.2}}",
    );
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a is_triggered_only=yes}",
    );
    let (code, r) = invoke(&f, "native", &[]);
    assert_eq!(code, 1, "{r}");
    for key in ["missing-event", "value"] {
        assert!(
            r["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["key"] == key)
        );
    }
}
#[test]
fn cli_graphics_index_survives_template_reindex() {
    let f = fixture("graphics-contracts");
    write(
        &f.game.join("gfx/models/test.asset"),
        "pdxmesh={name=mesh} entity={name=circle pdxmesh=mesh}",
    );
    write(
        &f.mod_path.join("common/component_templates/a.txt"),
        "utility_component_template={key=A friendly_aura={name=aura graphics={area_effect={entity=circle dynamic_scale=yes}}}}",
    );
    let (code, r) = invoke(&f, "good", &[]);
    assert_eq!(code, 0, "{r}");
    write(
        &f.mod_path.join("common/component_templates/a.txt"),
        "utility_component_template={key=A friendly_aura={name=aura graphics={area_effect={entity=absent}}}}",
    );
    let (code, r) = invoke(&f, "bad", &[]);
    assert_eq!(code, 1, "{r}");
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["key"] == "missing-item")
    );
}
#[test]
fn cli_explicit_design_power_and_partial_unknown() {
    let f = fixture("design-power-contract");
    for (path, text) in [
        (
            "common/ship_sizes/a.txt",
            "hull={section_slots={mid={locator=mid}} required_component_set=reactor}",
        ),
        (
            "common/section_templates/a.txt",
            "ship_section_template={key=section ship_size=hull fits_on_slot=mid component_slot={name=GUN template=small_turret}}",
        ),
        (
            "common/component_slot_templates/a.txt",
            "small_turret={size=small component=weapon}",
        ),
        (
            "common/component_templates/a.txt",
            "weapon_component_template={key=gun size=small power=-5 potential={is_ship_size=hull}} utility_component_template={key=core size=small component_set=reactor power=10 potential={is_ship_size=hull}}",
        ),
    ] {
        write(&f.game.join(path), text);
    }
    let design = "ship_design={name=design ship_size=hull section={slot=mid template=section component={slot=GUN template=gun}} required_component=core}";
    let path = f.mod_path.join("common/global_ship_designs/a.txt");
    write(&path, design);
    let (code, r) = invoke(&f, "complete", &[]);
    assert_eq!(code, 0, "{r}");
    assert_eq!(r["ship_feasibility"][0]["net_component_power"], 5.0);
    assert_eq!(
        r["ship_feasibility"][0]["automatic_design_validated"],
        false
    );
    write(&path, &design.replace("required_component=core", ""));
    let (code, r) = invoke(&f, "partial", &[]);
    assert_eq!(code, 2, "{r}");
    assert_eq!(r["ship_feasibility"][0]["status"], "unknown");
    assert!(
        !r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["key"] == "ship-power")
    );
    write(&path, design);
    let component_path = f.game.join("common/component_templates/a.txt");
    let components = std::fs::read_to_string(&component_path).unwrap();
    write(&component_path, &components.replace("power=10", "power=1"));
    let (code, r) = invoke(&f, "deficit", &[]);
    assert_eq!(code, 1, "{r}");
    assert_eq!(r["ship_feasibility"][0]["status"], "infeasible");
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["key"] == "ship-power")
    );
}
#[test]
fn cli_native_directive_opinion_weight_and_conditional_contracts() {
    let f = fixture("native-contracts-good");
    write(
        &f.game.join("common/opinion_modifiers/a.txt"),
        "known={opinion=10}",
    );
    write(
        &f.mod_path.join("common/scripted_effects/a.txt"),
        "helper={optimize_memory set_country_flag=ok}",
    );
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a trigger={OR={if={limit={is_ai=yes} has_country_flag=a} else={has_country_flag=b} always=yes}} immediate={helper=yes add_opinion_modifier={who=this modifier=known} random_owned_colony={weights={base=10 modifier={factor=0.1 owner={is_ai=no}}} set_planet_flag=ok}}}",
    );
    let (code, r) = invoke(&f, "good", &[]);
    assert_eq!(code, 0, "{r}");
    let (_, repeat) = invoke(&f, "repeat", &[]);
    assert_eq!(r["diagnostics"], repeat["diagnostics"]);
    assert!(r["phase_seconds"]["input_load_parse"].as_f64().unwrap() >= 0.);
}
#[test]
fn cli_native_contract_negative_controls_remain_errors() {
    let f = fixture("native-contracts-bad");
    let path = f.mod_path.join("events/a.txt");
    for (label, text, key) in [
        (
            "opinion",
            "country_event={id=a immediate={add_opinion_modifier={who=this modifier=missing}}}",
            "missing-item",
        ),
        (
            "weights",
            "country_event={id=a immediate={random_owned_colony={weights={modifier={factor=1 is_ai=no}} set_planet_flag=ok}}}",
            "scope",
        ),
        (
            "directive",
            "country_event={id=a immediate={optmize_memory}}",
            "unknown-effect",
        ),
    ] {
        write(&path, text);
        let (code, r) = invoke(&f, label, &[]);
        assert_eq!(code, 1, "{r}");
        assert!(
            r["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["key"] == key)
        );
    }
}
#[test]
fn cli_native_count_script_and_fleet_action_contracts() {
    let f = fixture("count-and-fleet-good");
    write(
        &f.game.join("common/scripted_triggers/a.txt"),
        "pop_amount={count_owned_pop_amount={}}",
    );
    write(
        &f.mod_path.join("events/a.txt"),
        "planet_event={id=a trigger={pop_amount>2000}} fleet_event={id=b immediate={queue_actions={effect={id=local set_fleet_flag=ok} move_to=solar_system}}}",
    );
    let (code, r) = invoke(&f, "native", &[]);
    assert_eq!(code, 0, "{r}");
    let (_, repeat) = invoke(&f, "repeat", &[]);
    assert_eq!(r["diagnostics"], repeat["diagnostics"]);
}
#[test]
fn cli_count_and_fleet_negative_controls_are_not_hidden() {
    let f = fixture("count-and-fleet-bad");
    write(
        &f.game.join("common/scripted_triggers/a.txt"),
        "truth={is_ai=no}",
    );
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a trigger={truth>5}} fleet_event={id=b immediate={queue_actions={effect={id=local set_country_flag=bad}} id=not_metadata}} ",
    );
    let (code, r) = invoke(&f, "native", &[]);
    assert_eq!(code, 1, "{r}");
    for key in ["scope", "value", "unknown-effect"] {
        assert!(
            r["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["key"] == key)
        );
    }
}
#[test]
fn cli_unproven_prev_and_component_weight_contexts() {
    let f = fixture("entry-contexts");
    write(
        &f.mod_path.join("common/scripted_effects/a.txt"),
        "helper={prev={set_country_flag=ok}}",
    );
    write(
        &f.mod_path.join("common/component_templates/a.txt"),
        "utility_component_template={key=A potential={FROM={is_ai=no}} ai_weight={modifier={factor=1 is_ai=no}}}",
    );
    let (code, r) = invoke(&f, "uninvoked", &[]);
    assert_eq!(code, 2, "{r}");
    assert_eq!(r["errors"], 0);
    write(
        &f.mod_path.join("events/a.txt"),
        "country_event={id=a immediate={helper=yes}}",
    );
    let (code, r) = invoke(&f, "known-empty", &[]);
    assert_eq!(code, 1, "{r}");
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["key"] == "scope"
                && d["message"].as_str().unwrap().contains("does not exist"))
    );
}
#[test]
fn cli_generated_base_event_is_indexed_without_validating_unrelated_body() {
    let f = fixture("base-template-laziness");
    write(
        &f.game.join("events/base.txt"),
        "inline_script={script=generator}",
    );
    write(
        &f.game.join("common/inline_scripts/generator.txt"),
        "country_event={id=generated is_triggered_only=yes immediate={inline_script={script=bad_inner}}}",
    );
    write(
        &f.game.join("common/inline_scripts/bad_inner.txt"),
        "set_country_flag=$MISSING$",
    );
    let path = f.mod_path.join("events/a.txt");
    write(
        &path,
        "country_event={id=mod immediate={set_country_flag=ok}}",
    );
    let (code, r) = invoke(&f, "unrelated", &[]);
    assert_eq!(code, 0, "{r}");
    write(
        &path,
        "country_event={id=mod immediate={country_event={id=generated}}}",
    );
    let (code, r) = invoke(&f, "reached", &[]);
    assert_eq!(code, 1, "{r}");
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["key"] == "macro-argument")
    );
    write(&path, "inline_script={script=generator}");
    let (code, r) = invoke(&f, "target-template", &[]);
    assert_eq!(code, 1, "{r}");
    assert!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["key"] == "macro-argument")
    );
}
