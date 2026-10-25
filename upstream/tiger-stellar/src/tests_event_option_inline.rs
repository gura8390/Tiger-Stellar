use super::*;

// Exact shipped template text (newline normalization only); source SHA256
// 4af3814916c851e63b4544b209cdca5a5cfd0cd1ec093f224ae52684442e9e02.
const NATIVE_DYNAMIC_COST: &str = r###"allow = {
	$OWNER_SCOPE$ = {
		if = {
			limit = {
				has_monthly_income = { resource = $RESOURCE$ value < $INCOME1$ }
			}
			has_resource = { type = $RESOURCE$ amount >= $COST1$ }
		}
		else_if = {
			limit = {
				has_monthly_income = { resource = $RESOURCE$ value > $INCOME1$ }
				has_monthly_income = { resource = $RESOURCE$ value <= $INCOME2$ }
			}
			has_resource = { type = $RESOURCE$ amount >= $COST2$ }
		}
		else_if = {
			limit = {
				has_monthly_income = { resource = $RESOURCE$ value > $INCOME2$ }
				has_monthly_income = { resource = $RESOURCE$ value <= $INCOME3$ }
			}
			has_resource = { type = $RESOURCE$ amount >= $COST3$ }
		}
		else_if = {
			limit = {
				has_monthly_income = { resource = $RESOURCE$ value > $INCOME3$ }
				has_monthly_income = { resource = $RESOURCE$ value <= $INCOME4$ }
			}
			has_resource = { type = $RESOURCE$ amount >= $COST4$ }
		}
		else = {
			has_resource = { type = $RESOURCE$ amount >= $COST5$ }
		}
	}
}

$OWNER_SCOPE$ = {
	if = {
		limit = {
			has_monthly_income = { resource = $RESOURCE$ value <= $INCOME1$ }
		}
		add_resource = {
			$RESOURCE$ = $COST1$
			mult = -1
		}
	}
	else_if = {
		limit = {
			has_monthly_income = { resource = $RESOURCE$ value > $INCOME1$ }
			has_monthly_income = { resource = $RESOURCE$ value <= $INCOME2$ }
		}
		add_resource = {
			$RESOURCE$ = $COST2$
			mult = -1
		}
	}
	else_if = {
		limit = {
			has_monthly_income = { resource = $RESOURCE$ value > $INCOME2$ }
			has_monthly_income = { resource = $RESOURCE$ value <= $INCOME3$ }
		}
		add_resource = {
			$RESOURCE$ = $COST3$
			mult = -1
		}
	}
	else_if = {
		limit = {
			has_monthly_income = { resource = $RESOURCE$ value > $INCOME3$ }
			has_monthly_income = { resource = $RESOURCE$ value <= $INCOME4$ }
		}
		add_resource = {
			$RESOURCE$ = $COST4$
			mult = -1
		}
	}
	else_if = {
		limit = {
			has_monthly_income = { resource = $RESOURCE$ value > $INCOME4$ }
		}
		add_resource = {
			$RESOURCE$ = $COST5$
			mult = -1
		}
	}
}"###;

fn option_inline_engine(text: &str) -> Engine {
    let mut e = make();
    e.program.inline.insert(
        "option/body".into(),
        ("option_body.txt".into(), text.into()),
    );
    e.program.entities.insert(
        "strategic_resources".into(),
        ["alloys".into()].into_iter().collect(),
    );
    e.program.entities.insert(
        "gfx_spritetype".into(),
        ["GFX_option".into()].into_iter().collect(),
    );
    e.program.loc.insert(
        "l_english".into(),
        ["OPTION".into(), "INLINED_NAME".into()]
            .into_iter()
            .collect(),
    );
    e
}
fn run_inline_option(e: &mut Engine, option: &str, scope: &str) -> State {
    let event_type = match scope {
        "country" => "country_event",
        "planet" => "planet_event",
        "ship" => "ship_event",
        "planet|ship" => "carrier_event",
        _ => "event",
    };
    let n = ast(
        e,
        &format!("{event_type}={{id=inline.1 option={{name=OPTION {option}}}}}"),
    )
    .remove(0);
    let mut state = State::default();
    e.event_body(&n, &Context::new(scope), &mut state);
    state
}
fn option_inline_case(text: &str, scope: &str) -> (Engine, State) {
    let mut e = option_inline_engine(text);
    let state = run_inline_option(&mut e, "inline_script=option/body", scope);
    (e, state)
}

#[test]
fn option_inline_native_dynamic_cost_allow_is_not_an_effect() {
    // nomads_arkship_events.txt:470-497 supplies this option inline call.
    let mut e = option_inline_engine("");
    e.program.inline.insert(
        "events/dynamic_resource_cost".into(),
        (
            "common/inline_scripts/events/dynamic_resource_cost.txt".into(),
            NATIVE_DYNAMIC_COST.into(),
        ),
    );
    run_inline_option(
        &mut e,
        "inline_script={script=events/dynamic_resource_cost RESOURCE=alloys COST1=200 INCOME1=10 COST2=500 INCOME2=50 COST3=1000 INCOME3=100 COST4=2000 INCOME4=200 COST5=5000 INCOME5=500 OWNER_SCOPE=owner}",
        "planet|ship",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(
        !e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "unknown-effect" && d.message.contains("allow"))
    );
}

#[test]
fn option_inline_allow_is_checked_in_event_scope() {
    let (e, _) = option_inline_case("allow={is_ai=no}", "planet");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "scope"));
    assert!(
        !e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "unknown-effect" && d.message.contains("allow"))
    );
}

#[test]
fn option_inline_missing_resource_reference_is_detected() {
    let (e, _) = option_inline_case(
        "allow={owner={has_resource={type=missing_resource amount>=1}}}",
        "planet",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
}

#[test]
fn option_inline_unknown_trigger_is_not_swallowed_as_metadata() {
    let (e, _) = option_inline_case("allow={owner={unknown_option_predicate=yes}}", "planet");
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "unknown-trigger")
    );
}

#[test]
fn option_inline_unknown_effect_remains_an_error() {
    let (e, _) = option_inline_case("allow={always=yes} unknown_option_effect=yes", "country");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "unknown-effect"));
}

#[test]
fn option_inline_metadata_fields_are_classified_before_effects() {
    let (e, _) = option_inline_case(
        "name=INLINED_NAME trigger={always=yes} allow={always=yes} highlight={always=yes} exclusive_trigger={always=yes} ai_chance={base=1} icon=GFX_option default=no is_dialog_only=no default_hide_option=no hide_option_if_not_allowed=no response_text=ok sound=ok show_sound=ok tag=ok",
        "country",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn option_inline_false_allow_does_not_execute_effects() {
    let (e, state) =
        option_inline_case("allow={always=no} save_event_target_as=blocked", "country");
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert_eq!(
        state.get("event_target:blocked").availability,
        Availability::Absent
    );
}

#[test]
fn option_inline_false_trigger_still_checks_effect_structure() {
    let (e, state) = option_inline_case(
        "trigger={always=no} unknown_option_effect=yes save_event_target_as=blocked",
        "country",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "unknown-effect"));
    assert_eq!(
        state.get("event_target:blocked").availability,
        Availability::Absent
    );
}

#[test]
fn option_inline_false_allow_still_checks_fixed_references() {
    let (e, _) = option_inline_case(
        "allow={always=no} add_resource={missing_resource=1}",
        "country",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
}

#[test]
fn option_inline_native_trigger_and_expanded_allow_both_apply() {
    let mut e = option_inline_engine("allow={always=no} save_event_target_as=blocked");
    let state = run_inline_option(
        &mut e,
        "trigger={always=yes} inline_script=option/body",
        "country",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert_eq!(
        state.get("event_target:blocked").availability,
        Availability::Absent
    );
}

#[test]
fn option_inline_trigger_refines_effect_receiver() {
    let mut e = option_inline_engine("trigger={is_scope_type=country} set_country_flag=ok");
    run_inline_option(&mut e, "inline_script=option/body", "country|planet");
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(!has(&e, "scope-type-ambiguous"), "{:?}", e.diagnostics.rows);
}

#[test]
fn option_inline_nested_allow_is_materialized_before_classification() {
    let mut e = option_inline_engine("inline_script=option/nested");
    e.program.inline.insert(
        "option/nested".into(),
        (
            "nested_option.txt".into(),
            "allow={owner={is_ai=no}} owner={set_country_flag=paid}".into(),
        ),
    );
    run_inline_option(&mut e, "inline_script=option/body", "planet");
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn option_inline_cache_does_not_borrow_a_previous_caller_scope() {
    let mut e = option_inline_engine("allow={is_star=yes}");
    run_inline_option(&mut e, "inline_script=option/body", "planet");
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    run_inline_option(&mut e, "inline_script=option/body", "country");
    assert!(e.diagnostics.rows.iter().any(|d| {
        d.key == "scope"
            && d.context
                .as_ref()
                .is_some_and(|c| c.this.label() == "country")
    }));
}

#[test]
fn option_inline_root_and_prev_are_preserved_in_allow() {
    let (e, _) = option_inline_case(
        "allow={owner={is_ai=no prev={is_star=no} root={is_star=no}}} owner={set_country_flag=paid}",
        "planet",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn option_inline_wrong_prev_scope_is_detected() {
    let (e, _) = option_inline_case("allow={owner={prev={is_ai=no}}}", "planet");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "scope"));
}

#[test]
fn option_inline_missing_inline_and_arguments_remain_errors() {
    let mut e = option_inline_engine("allow={$PREDICATE$=yes}");
    run_inline_option(&mut e, "inline_script=option/body", "country");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "macro-argument"));
    let mut e = option_inline_engine("");
    run_inline_option(&mut e, "inline_script=option/missing", "country");
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "missing-inline-script")
    );
}

#[test]
fn option_inline_does_not_execute_metadata_blocks() {
    let (e, _) = option_inline_case(
        "allow={owner={is_ai=no}} ai_chance={weight=1 modifier={factor=2 owner={is_ai=no}}}",
        "planet",
    );
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
}

#[test]
fn option_inline_recursive_body_retains_analysis_limit() {
    let mut e = option_inline_engine("inline_script=option/body");
    run_inline_option(&mut e, "inline_script=option/body", "country");
    assert!(has(&e, "analysis-limit"));
    assert!(!e.complete);
}

#[test]
fn option_inline_repeated_allow_has_no_assumed_composition_or_pruning() {
    let mut e = option_inline_engine("allow={always=yes} set_country_flag=wrong_receiver");
    run_inline_option(
        &mut e,
        "allow={always=no} inline_script=option/body",
        "planet",
    );
    assert!(has(&e, "event-option-condition-composition-unverified"));
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "scope"));
}
