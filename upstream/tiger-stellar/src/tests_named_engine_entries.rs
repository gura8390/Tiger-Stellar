use super::*;

const PK_TRIGGER_DOC: &str = "# this = planet being targeted\n# from = fleet\n#can_destroy_planet_with_PLANET_DESTRUCTION_COMPONENT = {\n";
const PK_ACTION_DOC: &str = "# this/root = planet that have been fired upon\n# from = fleet that fired\n#on_destroy_planet_with_PLANET_DESTRUCTION_COMPONENT = {\n# They use the same scopes as the standard events\n";

struct PkSourceFixture {
    root: std::path::PathBuf,
}
impl PkSourceFixture {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("tiger-pk-sources-{}-{stamp}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        Self { root }
    }
    fn write(&self, relative: &str, text: &str) -> std::path::PathBuf {
        let path = self.root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }
    fn engine(&self) -> Engine {
        let mut e = pk_engine("weapon_component_template={key=PRIVATE type=planet_killer}");
        let launcher = self.write(
            "game/launcher-settings.json",
            "{\"gameId\":\"stellaris\",\"rawVersion\":\"v4.5.2\"}",
        );
        e.program
            .record(&launcher, "launcher-settings.json", 0)
            .unwrap();
        e
    }
}
impl Drop for PkSourceFixture {
    fn drop(&mut self) {
        // Delete only this dedicated test directory directly under temp_dir.
        if self.root.parent() == Some(std::env::temp_dir().as_path())
            && self
                .root
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("tiger-pk-sources-"))
        {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

#[test]
fn planet_killer_registration_lookup_does_not_change_execution_trace() {
    let mut e = pk_engine("weapon_component_template={key=PRIVATE type=planet_killer}");
    let node = ast(&mut e, "can_destroy_planet_with_PRIVATE={}").remove(0);
    e.chain
        .push(serde_json::json!({"kind":"existing-caller"}).into());
    let before = e.chain.clone();
    for _ in 0..2 {
        e.planet_killer_test_entry(&node.key, "triggers", &node, PK_TRIGGER_DOC)
            .unwrap();
    }
    assert_eq!(e.chain, before);
    assert_eq!(e.engine_entries.len(), 1);
}

#[test]
fn planet_killer_runtime_source_is_layer_zero_and_hash_bound() {
    let fixture = PkSourceFixture::new();
    let mut e = fixture.engine();
    let relative = "common/scripted_triggers/01_scripted_triggers_planet_killers.txt";
    let path = fixture.write(&format!("game/{relative}"), PK_TRIGGER_DOC);
    e.program
        .record(&path.canonicalize().unwrap(), relative, 0)
        .unwrap();
    let count = e.program.inputs.len();
    let node = ast(&mut e, "can_destroy_planet_with_PRIVATE={}").remove(0);
    e.planet_killer_entry(&node.key, "triggers", &node).unwrap();
    assert_eq!(
        e.program.inputs.len(),
        count,
        "a canonical manifest path and a launcher-relative spelling identify one physical source"
    );
    assert_eq!(
        e.engine_entries[0]["evidence"]["sha256"],
        crate::hash::sha256(PK_TRIGGER_DOC.as_bytes())
    );
    assert_eq!(
        std::path::Path::new(e.engine_entries[0]["evidence"]["path"].as_str().unwrap()),
        path.canonicalize().unwrap().as_path()
    );
    std::fs::write(
        &path,
        format!("{PK_TRIGGER_DOC}# changed after input snapshot\n"),
    )
    .unwrap();
    assert!(
        e.planet_killer_entry(&node.key, "triggers", &node)
            .is_none()
    );
}

#[test]
fn planet_killer_mod_comments_cannot_authorize_missing_or_changed_vanilla_contract() {
    let fixture = PkSourceFixture::new();
    let mut e = fixture.engine();
    let relative = "common/scripted_triggers/01_scripted_triggers_planet_killers.txt";
    let mod_path = fixture.write(&format!("mod/{relative}"), PK_TRIGGER_DOC);
    e.program.record(&mod_path, relative, 1).unwrap();
    let node = ast(&mut e, "can_destroy_planet_with_PRIVATE={}").remove(0);
    assert!(
        e.planet_killer_entry(&node.key, "triggers", &node)
            .is_none()
    );
    let original = fixture.write(&format!("game/{relative}"), PK_TRIGGER_DOC);
    e.planet_killer_entry(&node.key, "triggers", &node).unwrap();
    assert!(e.program.inputs.iter().any(|i| i.layer == 0
        && i.relative_path == relative
        && std::path::Path::new(&i.path).canonicalize().unwrap()
            == original.canonicalize().unwrap()));
    let mut changed = fixture.engine();
    changed.program.record(&mod_path, relative, 1).unwrap();
    std::fs::write(&original, "# unrelated vanilla annotation\n").unwrap();
    let node = ast(&mut changed, "can_destroy_planet_with_PRIVATE={}").remove(0);
    assert!(
        changed
            .planet_killer_entry(&node.key, "triggers", &node)
            .is_none()
    );
    assert!(changed.engine_entries.is_empty());
}

fn pk_engine(components: &str) -> Engine {
    let mut e = make();
    let nodes = ast(&mut e, components);
    e.program.index(File {
        path: "components.txt".into(),
        category: "component_templates".into(),
        ast: nodes.into_iter().map(Arc::new).collect(),
        target: false,
    });
    e
}

#[test]
fn planet_killer_registered_trigger_has_documented_this_from_but_unknown_root() {
    let mut e = pk_engine("weapon_component_template={key=PRIVATE type=planet_killer}");
    let node = ast(
        &mut e,
        "can_destroy_planet_with_PRIVATE={is_star=yes from={has_fleet_flag=ready}}",
    )
    .remove(0);
    let ctx = e
        .planet_killer_test_entry(&node.key, "triggers", &node, PK_TRIGGER_DOC)
        .unwrap();
    assert_eq!(ctx.this.label(), "planet");
    assert_eq!(ctx.froms[0].label(), "fleet");
    assert!(ctx.root.unknown);
    assert!(ctx.prev_tail_unknown);
    e.triggers(node.body(), &ctx, &State::default());
    assert!(e.diagnostics.rows.is_empty(), "{:?}", e.diagnostics.rows);
    let record = &e.engine_entries[0];
    assert_eq!(record["engine_entry_registered"], true);
    for field in [
        "ordinary_script_call_proven",
        "runtime_invocation_proven",
        "runtime_availability_proven",
        "runtime_target_validity_proven",
    ] {
        assert_eq!(record[field], false);
    }
    assert_eq!(record["evidence"]["markers"][0]["line"], 1);
}

#[test]
fn planet_killer_registered_trigger_wrong_this_and_from_still_fail() {
    let mut e = pk_engine("weapon_component_template={key=PRIVATE type=planet_killer}");
    let node = ast(
        &mut e,
        "can_destroy_planet_with_PRIVATE={is_ai=yes from={is_star=yes}}",
    )
    .remove(0);
    let ctx = e
        .planet_killer_test_entry(&node.key, "triggers", &node, PK_TRIGGER_DOC)
        .unwrap();
    e.triggers(node.body(), &ctx, &State::default());
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "scope" && d.message.contains("is_ai"))
    );
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "scope" && d.message.contains("is_star"))
    );
}

#[test]
fn planet_killer_trigger_root_and_prev_are_not_invented() {
    let mut e = pk_engine("weapon_component_template={key=PRIVATE type=planet_killer}");
    let node = ast(
        &mut e,
        "can_destroy_planet_with_PRIVATE={root={is_ai=yes} prev={is_ai=yes}}",
    )
    .remove(0);
    let ctx = e
        .planet_killer_test_entry(&node.key, "triggers", &node, PK_TRIGGER_DOC)
        .unwrap();
    e.triggers(node.body(), &ctx, &State::default());
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "scope-type-unknown"));
    assert!(has(&e, "caller-context-unknown"));
}

#[test]
fn planet_killer_action_and_queued_variants_share_documented_context() {
    for suffix in ["", "_queued", "_unqueued"] {
        let mut e = pk_engine("weapon_component_template={key=PRIVATE type=planet_killer}");
        let name = format!("on_destroy_planet_with_PRIVATE{suffix}");
        let node = ast(&mut e, &format!("{name}={{}}")).remove(0);
        let ctx = e
            .planet_killer_test_entry(&name, "on_actions", &node, PK_ACTION_DOC)
            .unwrap();
        assert_eq!(ctx.this.label(), "planet");
        assert_eq!(ctx.root.label(), "planet");
        assert_eq!(ctx.object, ctx.root_object);
        assert_eq!(ctx.froms[0].label(), "fleet");
    }
}

#[test]
fn planet_killer_full_component_key_wins_over_queued_suffix() {
    let mut e = pk_engine("weapon_component_template={key=PRIVATE_queued type=planet_killer}");
    let node = ast(&mut e, "on_destroy_planet_with_PRIVATE_queued={}").remove(0);
    e.planet_killer_test_entry(&node.key, "on_actions", &node, PK_ACTION_DOC)
        .unwrap();
    assert_eq!(e.engine_entries[0]["component"], "PRIVATE_queued");
    assert_eq!(e.engine_entries[0]["variant"], serde_json::Value::Null);
}

#[test]
fn planet_killer_wrong_or_missing_component_and_name_do_not_register() {
    for components in ["", "weapon_component_template={key=PRIVATE type=missile}"] {
        let mut e = pk_engine(components);
        let node = ast(&mut e, "can_destroy_planet_with_PRIVATE={}").remove(0);
        assert!(
            e.planet_killer_test_entry(&node.key, "triggers", &node, PK_TRIGGER_DOC)
                .is_none()
        );
        assert!(e.engine_entries.is_empty());
    }
    let mut e = pk_engine("weapon_component_template={key=PRIVATE type=planet_killer}");
    let node = ast(&mut e, "almost_can_destroy_planet_with_PRIVATE={}").remove(0);
    assert!(
        e.planet_killer_test_entry(&node.key, "triggers", &node, PK_TRIGGER_DOC)
            .is_none()
    );
}

#[test]
fn planet_killer_markers_must_be_complete_comment_annotations() {
    for docs in [
        "",
        "# this = planet being targeted\n# from = fleet\n",
        "this = planet being targeted\nfrom = fleet\ncan_destroy_planet_with_PLANET_DESTRUCTION_COMPONENT = {\n",
        "# this = country being targeted\n# from = fleet\n#can_destroy_planet_with_PLANET_DESTRUCTION_COMPONENT = {\n",
    ] {
        let mut e = pk_engine("weapon_component_template={key=PRIVATE type=planet_killer}");
        let node = ast(&mut e, "can_destroy_planet_with_PRIVATE={}").remove(0);
        assert!(
            e.planet_killer_test_entry(&node.key, "triggers", &node, docs)
                .is_none()
        );
    }
}

#[test]
fn planet_killer_queued_variant_requires_explicit_shared_scope_marker() {
    let mut e = pk_engine("weapon_component_template={key=PRIVATE type=planet_killer}");
    let node = ast(&mut e, "on_destroy_planet_with_PRIVATE_queued={}").remove(0);
    let docs = PK_ACTION_DOC.lines().take(3).collect::<Vec<_>>().join("\n");
    assert!(
        e.planet_killer_test_entry(&node.key, "on_actions", &node, &docs)
            .is_none()
    );
}

#[test]
fn planet_killer_effective_component_override_controls_registration() {
    let mut e = pk_engine("weapon_component_template={key=PRIVATE type=planet_killer}");
    let overridden = ast(
        &mut e,
        "weapon_component_template={key=PRIVATE type=missile}",
    );
    e.program.index(File {
        path: "override.txt".into(),
        category: "component_templates".into(),
        ast: overridden.into_iter().map(Arc::new).collect(),
        target: true,
    });
    let node = ast(&mut e, "can_destroy_planet_with_PRIVATE={}").remove(0);
    assert!(
        e.planet_killer_test_entry(&node.key, "triggers", &node, PK_TRIGGER_DOC)
            .is_none()
    );
}

#[test]
fn planet_killer_ordinary_script_call_does_not_take_engine_entry_context() {
    let mut e = pk_engine("weapon_component_template={key=PRIVATE type=planet_killer}");
    let node = ast(&mut e, "can_destroy_planet_with_PRIVATE={is_star=yes}").remove(0);
    e.program
        .scripts
        .insert((false, node.key.clone()), Arc::new(node));
    let body = ast(&mut e, "can_destroy_planet_with_PRIVATE=yes");
    e.triggers(&body, &Context::new("country"), &State::default());
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "scope"));
    assert!(e.engine_entries.is_empty());
}

#[test]
fn planet_killer_registration_does_not_create_named_target_availability() {
    let mut e = pk_engine("weapon_component_template={key=PRIVATE type=planet_killer}");
    let node = ast(
        &mut e,
        "can_destroy_planet_with_PRIVATE={event_target:runtime_only={is_ai=yes}}",
    )
    .remove(0);
    let ctx = e
        .planet_killer_test_entry(&node.key, "triggers", &node, PK_TRIGGER_DOC)
        .unwrap();
    e.triggers(node.body(), &ctx, &State::default());
    assert_eq!(errors(&e), 0);
    assert!(has(&e, "target-binding-unproven"));
}
