use super::*;
use crate::documented_on_actions::parse_annotations;

fn annotated(comments: &str) -> crate::documented_on_actions::DocumentedAction {
    let e = make();
    let text = format!("{comments}\non_fixture={{}}\n");
    parse_annotations(
        &text,
        comments.lines().count() as u32 + 1,
        "on_fixture",
        &e.rules,
    )
    .unwrap()
}
#[test]
fn annotation_this_does_not_lend_root_or_prev() {
    let entry = annotated("# This = planet being targeted\n# From = fleet that fired");
    assert_eq!(entry.context.this.label(), "planet");
    assert_eq!(entry.context.froms[0].label(), "fleet");
    assert!(entry.context.root.unknown);
    assert!(entry.context.prevs.is_empty());
    assert!(entry.context.prev_tail_unknown);
}
#[test]
fn annotation_root_only_does_not_invent_this() {
    let entry = annotated("# Root = Country\n# From = system");
    assert!(entry.context.this.unknown);
    assert_eq!(entry.context.root.label(), "country");
    assert_eq!(entry.context.froms[0].label(), "galactic_object");
}
#[test]
fn annotation_explicit_shared_root_has_shared_identity() {
    let entry = annotated("# THIS/ROOT = planet that have been fired upon");
    assert_eq!(entry.context.this.label(), "planet");
    assert_eq!(entry.context.root.label(), "planet");
    assert_eq!(entry.context.object, entry.context.root_object);
}
#[test]
fn annotation_same_type_is_not_same_object_without_explicit_alias() {
    let entry = annotated("# This = Country\n# Root = Country");
    assert_ne!(entry.context.object, entry.context.root_object);
}
#[test]
fn annotation_shared_label_in_prose_does_not_prove_object_identity() {
    let entry = annotated("# This = Country # unlike this/root example\n# Root = Country");
    assert_ne!(entry.context.object, entry.context.root_object);
}
#[test]
fn annotation_optional_sparse_from_is_not_definite_scope() {
    let entry = annotated("# Root = Planet\n# FromFrom = Fleet of the science ship, if any");
    assert!(entry.context.froms[0].unknown);
    assert!(entry.context.froms[1].unknown);
    assert!(entry.context.froms[1].known.contains("fleet"));
    assert!(entry.optional.contains("fromfrom"));
}
#[test]
fn annotation_native_invalid_ship_slot_retains_unknown() {
    let entry = annotated(
        "# This = Country\n# FromFromFrom = ship 2 # can be invalid if damage is done via other methods (like scripted action)",
    );
    assert!(entry.context.froms[2].known.contains("ship"));
    assert!(entry.context.froms[2].unknown);
    assert!(entry.optional.contains("fromfromfrom"));
}
#[test]
fn annotation_owner_contract_uses_exported_scope_rule() {
    let entry = annotated("# This = owner of ship 1 (destroyed)\n# From = owner of fleet 2");
    assert_eq!(entry.context.this.label(), "country");
    assert_eq!(entry.context.froms[0].label(), "country");
    let mut e = make();
    e.rules.scopes.get_mut("owner").unwrap().output = Some("planet".into());
    e.rules
        .scopes
        .get_mut("owner")
        .unwrap()
        .output_by_input
        .clear();
    assert!(
        parse_annotations(
            "# This = owner of ship 1\non_fixture={}",
            2,
            "on_fixture",
            &e.rules
        )
        .is_none()
    );
}
#[test]
fn annotation_ambiguous_natural_language_is_not_a_contract() {
    let e = make();
    for phrase in [
        "planet or ship",
        "receiver",
        "Empire 1",
        "owner of unknown_object",
        "owner of ship that died",
    ] {
        let text = format!("# This = {phrase}\non_fixture={{}}");
        assert!(
            parse_annotations(&text, 2, "on_fixture", &e.rules).is_none(),
            "{phrase}"
        );
    }
}
#[test]
fn annotation_conflicting_slot_does_not_take_last_word() {
    let entry = annotated("# This = Planet\n# This = Country\n# From = Fleet");
    assert!(entry.context.this.unknown);
    assert!(entry.context.this.known.is_empty());
}
#[test]
fn annotation_ambiguous_duplicate_revokes_type_in_either_order() {
    for comments in [
        "# This = Planet\n# This = Planet or Ship",
        "# This = Planet or Ship\n# This = Planet",
    ] {
        let entry = annotated(comments);
        assert!(entry.context.this.unknown);
        assert!(entry.context.this.known.is_empty());
    }
}
#[test]
fn annotation_only_immediately_preceding_comment_block_counts() {
    let e = make();
    let text = "# This = Planet\non_previous={}\non_fixture={}";
    assert!(parse_annotations(text, 3, "on_fixture", &e.rules).is_none());
    assert!(parse_annotations(text, 2, "on_fixture", &e.rules).is_none());
}
#[test]
fn annotation_engine_types_remain_stellaris_only() {
    let e = make();
    assert!(
        parse_annotations(
            "# This = character\non_fixture={}",
            2,
            "on_fixture",
            &e.rules
        )
        .is_none()
    );
    assert_eq!(
        annotated("# Scope = Astral Rift").context.this.label(),
        "astral_rift"
    );
    assert_eq!(
        annotated("# Scope = pop group").context.this.label(),
        "pop_group"
    );
}
fn annotated_run(comments: &str, effects: &str) -> Engine {
    let mut e = make();
    let action = ast(&mut e, "on_fixture={events={receiver}}").remove(0);
    e.program.index(File {
        path: "actions.txt".into(),
        category: "on_actions".into(),
        ast: vec![Arc::new(action.clone())],
        target: false,
    });
    let event = ast(
        &mut e,
        &format!("planet_event={{id=receiver is_triggered_only=yes immediate={{{effects}}}}}"),
    );
    e.program.index(File {
        path: "events.txt".into(),
        category: "events".into(),
        ast: event.into_iter().map(Arc::new).collect(),
        target: false,
    });
    e.documented_action_entries
        .insert("on_fixture".into(), annotated(comments));
    e.on_action_fallback(&action);
    e.analyze();
    e
}
#[test]
fn annotation_action_event_preserves_documented_from_and_root() {
    let e = annotated_run(
        "# THIS/ROOT = Planet\n# From = Fleet",
        "set_planet_flag=ok root={set_planet_flag=ok} from={set_fleet_flag=ok}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(!has(&e, "on-action-caller-context-unknown"));
    assert_eq!(e.engine_entries.len(), 1);
    assert_eq!(e.engine_entries[0]["runtime_invocation_proven"], false);
    assert_eq!(e.engine_entries[0]["runtime_target_validity_proven"], false);
}
#[test]
fn annotation_action_event_wrong_documented_from_is_detected() {
    let e = annotated_run(
        "# THIS/ROOT = Planet\n# From = Fleet",
        "from={set_country_flag=wrong}",
    );
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "scope"));
}
#[test]
fn annotation_event_unknown_root_still_has_boundary() {
    let e = annotated_run(
        "# THIS = Planet\n# From = Fleet",
        "root={set_country_flag=unknown}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "scope-type-unknown"));
}
fn doc_source_case(layer: usize, drift: bool) -> Engine {
    let mut e = make();
    let text = "# THIS/ROOT = planet\n# From = fleet\non_fixture={}";
    let path = std::env::temp_dir().join(format!(
        "tiger-doc-source-{}-{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, text).unwrap();
    let mut nodes = ast(&mut e, text);
    let source = path.to_string_lossy().into_owned();
    for n in &mut nodes {
        n.locate(&source, &mut e.program.next_id);
    }
    e.program.index(File {
        path: source.clone(),
        category: "on_actions".into(),
        ast: nodes.into_iter().map(Arc::new).collect(),
        target: false,
    });
    e.program.inputs.push(crate::loader::Input {
        relative_path: "common/on_actions/native.txt".into(),
        path: source,
        layer,
        sha256: if drift {
            "wrong".into()
        } else {
            crate::hash::sha256(text.as_bytes())
        },
    });
    e.prepare_documented_actions();
    std::fs::remove_file(path).unwrap();
    e
}
#[test]
fn annotation_loader_requires_hash_bound_layer_zero_source() {
    assert!(
        doc_source_case(0, false)
            .documented_action_entries
            .contains_key("on_fixture")
    );
    assert!(
        doc_source_case(1, false)
            .documented_action_entries
            .is_empty()
    );
    assert!(
        doc_source_case(0, true)
            .documented_action_entries
            .is_empty()
    );
}
