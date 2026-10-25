//! These roots deliberately have no event/script summary of their own. A
//! target event would hide an on-action demand-key mismatch through event:id.
use super::*;

#[derive(Clone, Copy, Debug)]
enum DemandRoot {
    Decision,
    ActionEvents,
}

fn demand_file(e: &mut Engine, path: &str, category: &str, text: &str, target: bool) {
    let parsed = e.parser.parse(path, text, "").unwrap();
    assert!(parsed.fatal.is_none() && parsed.diagnostics.is_empty());
    let mut nodes = parsed.ast;
    for node in &mut nodes {
        node.locate(path, &mut e.program.next_id);
    }
    e.program.index(File {
        path: path.into(),
        category: category.into(),
        ast: nodes.into_iter().map(Arc::new).collect(),
        target,
    });
}

fn demand_fixture(root: DemandRoot, body: &str, dynamic_writer: bool, multihop: bool) -> Engine {
    let mut e = make();
    let dynamic = if dynamic_writer {
        " unknown_writer={save_global_event_target_as=$TARGET$}"
    } else {
        ""
    };
    demand_file(
        &mut e,
        "upstream_writers.txt",
        "scripted_effects",
        &format!(
            "producer={{create_country={{effect={{save_global_event_target_as=shared}}}}}}{dynamic}"
        ),
        false,
    );
    demand_file(
        &mut e,
        "upstream_consumer.txt",
        "events",
        &format!(
            "country_event={{id=upstream.consumer is_triggered_only=yes immediate={{{body}}}}}
             country_event={{id=upstream.unrelated is_triggered_only=yes immediate={{global_event_target:unrelated={{set_country_flag=unused}}}}}}"
        ),
        false,
    );
    if multihop {
        demand_file(
            &mut e,
            "upstream_bridge.txt",
            "events",
            "country_event={id=upstream.bridge is_triggered_only=yes immediate={fire_on_action={on_action=on_second}}}",
            false,
        );
        demand_file(
            &mut e,
            "upstream_second_action.txt",
            "on_actions",
            "on_second={events={upstream.consumer}}",
            false,
        );
    }
    let first_event = if multihop {
        "upstream.bridge"
    } else {
        "upstream.consumer"
    };
    match root {
        DemandRoot::Decision => {
            demand_file(
                &mut e,
                "upstream_first_action.txt",
                "on_actions",
                &format!("on_first={{events={{{first_event}}}}}"),
                false,
            );
            demand_file(
                &mut e,
                "target_decision.txt",
                "decisions",
                "test_decision={effect={fire_on_action={on_action=on_first}}}",
                true,
            );
        }
        DemandRoot::ActionEvents => demand_file(
            &mut e,
            "target_action.txt",
            "on_actions",
            &format!("on_first={{events={{{first_event}}}}}"),
            true,
        ),
    }
    e
}

fn run_demand(root: DemandRoot, body: &str, dynamic: bool, multihop: bool) -> Engine {
    let mut e = demand_fixture(root, body, dynamic, multihop);
    e.analyze();
    let candidates = e
        .saved_target_type_candidates("global_event_target:shared")
        .unwrap_or_else(|| panic!("{root:?}: reachable action reader was not demanded"));
    assert!(
        candidates.known.contains("country"),
        "{root:?}: {candidates:?}"
    );
    assert_eq!(candidates.unknown, dynamic, "{root:?}: {candidates:?}");
    let summary = e
        .saved_target_type_evidence()
        .into_iter()
        .find(|v| v["kind"] == "writer-type-discovery-summary")
        .unwrap();
    let wanted = summary["demanded_names"].as_array().unwrap();
    assert!(wanted.iter().any(|v| v == "shared"));
    assert!(!wanted.iter().any(|v| v == "unrelated"));
    assert!(
        e.event_inputs
            .keys()
            .any(|k| k.starts_with("upstream.consumer\n")),
        "{root:?}: consumer was not dispatched"
    );
    if multihop {
        assert!(
            e.event_inputs
                .keys()
                .any(|k| k.starts_with("upstream.bridge\n"))
        );
    }
    e
}

fn positive(root: DemandRoot) {
    let e = run_demand(
        root,
        "global_event_target:shared={set_country_flag=ok}",
        false,
        false,
    );
    assert_eq!(errors(&e), 0, "{root:?}: {:?}", e.diagnostics.rows);
    assert!(has(&e, "target-binding-unproven"));
}

fn negative(root: DemandRoot, exists: bool, multihop: bool) {
    let body = if exists {
        "if={limit={exists=global_event_target:shared} global_event_target:shared={set_planet_flag=wrong}}"
    } else {
        "global_event_target:shared={set_planet_flag=wrong}"
    };
    let e = run_demand(root, body, false, multihop);
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| { d.key == "scope" && d.message.contains("set_planet_flag") }),
        "{root:?}: {:?}",
        e.diagnostics.rows
    );
}

fn unknown(root: DemandRoot) {
    let e = run_demand(
        root,
        "global_event_target:shared={set_planet_flag=uncertain}",
        true,
        false,
    );
    assert_eq!(errors(&e), 0, "{root:?}: {:?}", e.diagnostics.rows);
    assert!(has(&e, "scope-type-unknown"));
    assert!(has(&e, "target-binding-unproven"));
}

#[test]
fn decision_fire_action_demands_allow_a_legal_upstream_reader() {
    positive(DemandRoot::Decision);
}
#[test]
fn action_event_list_demands_allow_a_legal_upstream_reader() {
    positive(DemandRoot::ActionEvents);
}
#[test]
fn decision_fire_action_demands_detect_a_wrong_upstream_reader_scope() {
    negative(DemandRoot::Decision, false, false);
}
#[test]
fn action_event_list_demands_detect_a_wrong_upstream_reader_scope() {
    negative(DemandRoot::ActionEvents, false, false);
}
#[test]
fn decision_fire_action_exists_guard_retains_the_closed_writer_type() {
    negative(DemandRoot::Decision, true, false);
}
#[test]
fn action_event_list_exists_guard_retains_the_closed_writer_type() {
    negative(DemandRoot::ActionEvents, true, false);
}
#[test]
fn decision_fire_action_dynamic_writer_remains_unknown() {
    unknown(DemandRoot::Decision);
}
#[test]
fn action_event_list_dynamic_writer_remains_unknown() {
    unknown(DemandRoot::ActionEvents);
}
#[test]
fn decision_fire_action_multihop_reader_demands_are_transitive() {
    negative(DemandRoot::Decision, false, true);
}
#[test]
fn action_event_list_multihop_reader_demands_are_transitive() {
    negative(DemandRoot::ActionEvents, false, true);
}

fn selector_fixture(selector: &str, name: &str) -> Engine {
    let mut e = make();
    demand_file(
        &mut e,
        "selector_writer.txt",
        "scripted_effects",
        &format!(
            "producer={{create_ship={{effect={{{selector}={{save_global_event_target_as={name}}}}}}}}}"
        ),
        false,
    );
    demand_file(
        &mut e,
        "selector_consumer.txt",
        "events",
        &format!(
            "country_event={{id=consumer is_triggered_only=yes immediate={{global_event_target:{name}={{set_planet_flag=probe}}}}}}"
        ),
        true,
    );
    e.prepare_reads();
    e.prepare_saved_target_types();
    e
}

fn selector_case(selector: &str, expected: &str, wrong: bool, exists: bool) {
    let flag = match expected {
        "ship" => "set_ship_flag",
        "country" => "set_country_flag",
        _ => panic!("unexpected fixture scope"),
    };
    // Establish that the semantic scope resolver already accepts this spelling
    // on a ship. No country.owner expression is used as a supposed positive.
    let mut semantic = make();
    let scope_body = ast(
        &mut semantic,
        &format!("{selector}={{save_global_event_target_as=semantic_probe {flag}=ok}}"),
    );
    let mut state = State::default();
    semantic.effects(&scope_body, &Context::new("ship"), &mut state);
    assert_eq!(
        errors(&semantic),
        0,
        "{selector}: {:?}",
        semantic.diagnostics.rows
    );
    assert_eq!(
        state.get("global_event_target:semantic_probe").types,
        Types::new(expected),
        "{selector}: scope resolver fixture did not produce the expected type"
    );

    let mut e = selector_fixture(selector, "shared");
    assert_eq!(
        e.saved_target_type_candidates("global_event_target:shared"),
        Some(Types::new(expected)),
        "{selector}: index disagrees with semantic scope normalization"
    );
    // Native set_planet_flag accepts Ship/Colony as well as Planet.
    let wrong_command = if expected == "ship" {
        "set_country_flag"
    } else {
        "set_planet_flag"
    };
    let command = if wrong { wrong_command } else { flag };
    let receiver = format!("global_event_target:shared={{{command}=checked}}");
    let body = if exists {
        format!("if={{limit={{exists=global_event_target:shared}} {receiver}}}")
    } else {
        receiver
    };
    let body = ast(&mut e, &body);
    e.effects(&body, &Context::new("country"), &mut State::default());
    if wrong {
        assert!(
            e.diagnostics
                .rows
                .iter()
                .any(|d| { d.key == "scope" && d.message.contains(wrong_command) }),
            "{selector}: {:?}",
            e.diagnostics.rows
        );
    } else {
        assert_eq!(errors(&e), 0, "{selector}: {:?}", e.diagnostics.rows);
        assert!(has(&e, "target-binding-unproven"));
    }
}

#[test]
fn uppercase_this_writer_matches_existing_semantic_scope_positive() {
    selector_case("THIS", "ship", false, false);
}
#[test]
fn uppercase_this_writer_detects_a_wrong_consumer() {
    selector_case("THIS", "ship", true, false);
}
#[test]
fn uppercase_this_writer_exists_guard_keeps_the_type() {
    selector_case("THIS", "ship", true, true);
}

#[test]
fn uppercase_this_ship_writer_retains_the_native_planet_flag_permission() {
    let mut e = selector_fixture("THIS", "shared");
    let body = ast(
        &mut e,
        "global_event_target:shared={set_planet_flag=carrier}",
    );
    e.effects(&body, &Context::new("country"), &mut State::default());
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "target-binding-unproven"));
}
#[test]
fn dotted_uppercase_optional_owner_matches_existing_scope_positive() {
    selector_case("this.OWNER?", "country", false, false);
}
#[test]
fn dotted_uppercase_optional_owner_detects_a_wrong_consumer() {
    selector_case("this.OWNER?", "country", true, false);
}
#[test]
fn dotted_uppercase_optional_owner_exists_guard_keeps_the_type() {
    selector_case("this.OWNER?", "country", true, true);
}
#[test]
fn optional_owner_writer_matches_existing_semantic_scope_positive() {
    selector_case("owner?", "country", false, false);
}
#[test]
fn optional_owner_writer_detects_a_wrong_consumer() {
    selector_case("owner?", "country", true, false);
}
#[test]
fn optional_owner_writer_exists_guard_keeps_the_type() {
    selector_case("owner?", "country", true, true);
}
#[test]
fn mixed_case_owner_writer_matches_existing_semantic_scope_positive() {
    selector_case("OwNeR", "country", false, false);
}
#[test]
fn mixed_case_owner_writer_detects_a_wrong_consumer() {
    selector_case("OwNeR", "country", true, false);
}
#[test]
fn mixed_case_owner_writer_exists_guard_keeps_the_type() {
    selector_case("OwNeR", "country", true, true);
}
#[test]
fn normalized_root_from_prev_still_do_not_borrow_created_this() {
    for selector in ["ROOT", "FROM?", "PREV?"] {
        let e = selector_fixture(selector, "shared");
        assert!(
            e.saved_target_type_candidates("global_event_target:shared")
                .unwrap()
                .unknown,
            "{selector}: unknown caller context became a created ship"
        );
    }
}
#[test]
fn dynamic_selector_writer_keeps_the_candidate_set_open() {
    let e = selector_fixture("$SELECTOR$", "shared");
    assert!(
        e.saved_target_type_candidates("global_event_target:shared")
            .unwrap()
            .unknown
    );
}
#[test]
fn selector_normalization_preserves_target_identity_case_and_namespace() {
    let e = selector_fixture("THIS", "Shared");
    assert_eq!(
        e.saved_target_type_candidates("global_event_target:Shared"),
        Some(Types::new("ship"))
    );
    assert!(
        e.saved_target_type_candidates("global_event_target:shared")
            .is_none()
    );
    // Global targets can be read through the local-looking alias, never the
    // reverse. This spelling change must not manufacture a second identity.
    assert_eq!(
        e.saved_target_type_candidates("event_target:Shared"),
        Some(Types::new("ship"))
    );
}
