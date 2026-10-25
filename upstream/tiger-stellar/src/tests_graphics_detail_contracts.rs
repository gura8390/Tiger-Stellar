use super::*;

fn detail_graphics_file(e: &mut Engine, path: &str, text: &str, target: bool) -> Vec<Arc<Node>> {
    let parsed = e.parser.parse(path, text, "").unwrap();
    assert!(parsed.fatal.is_none() && parsed.diagnostics.is_empty());
    let mut nodes = parsed.ast;
    for node in &mut nodes {
        node.locate(path, &mut e.program.next_id);
    }
    let nodes = nodes.into_iter().map(Arc::new).collect::<Vec<_>>();
    e.program.index(File {
        path: path.into(),
        category: "gfx".into(),
        ast: nodes.clone(),
        target,
    });
    nodes
}

fn graphics_details_case(providers: &str, target: &str) -> Engine {
    let mut e = make();
    detail_graphics_file(&mut e, "providers.asset", providers, false);
    let nodes = detail_graphics_file(&mut e, "target.asset", target, true);
    for node in nodes {
        e.graphics_detail_contracts(&node);
    }
    e
}

#[test]
fn graphics_native_project_entity_fields_are_accepted() {
    let e = graphics_details_case(
        "",
        "entity={name=sp_planet_generator_entity pdxmesh=sp_planet_generator_mesh scale=1.2 cull_radius=110 default_state=idle state={name=idle state_time=10} state={name=construction state_time=10} state={name=death state_time=1 looping=no} game_data={shader_type=ship emissive_recolor_crunch=0.0}}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(e.diagnostics.rows.is_empty());
}

#[test]
fn graphics_probabilistic_duplicate_state_names_are_not_duplicate_errors() {
    let e = graphics_details_case(
        "",
        "entity={name=ship default_state=idle state={name=idle animation=idle} state={name=death animation=death chance=1 looping=no} state={name=death animation=death2 chance=1 looping=no} state={name=attack next_state=idle looping=no}}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
}

#[test]
fn graphics_contract_does_not_invent_required_entity_or_animation_fields() {
    let e = graphics_details_case("", "entity={} objectTypes={pdxmesh={}} animation={}");
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
}

#[test]
fn graphics_entity_state_and_mesh_declarations_require_blocks() {
    for text in [
        "entity=yes",
        "entity={state=idle}",
        "objectTypes={pdxmesh=mesh}",
    ] {
        let e = graphics_details_case("", text);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "structure"),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn graphics_known_selector_fields_require_scalars() {
    for text in [
        "entity={name={bad=1}}",
        "entity={pdxmesh={bad=1}}",
        "entity={default_state={bad=1}}",
        "entity={state={name={bad=1}}}",
        "entity={state={animation={bad=1}}}",
        "entity={state={next_state={bad=1}}}",
        "animation={name=clip file={bad=1}}",
    ] {
        let e = graphics_details_case("", text);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "structure"),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn graphics_numeric_fields_check_shape_and_finiteness() {
    for text in [
        "entity={scale=NaN}",
        "entity={cull_radius=1e999}",
        "entity={state={state_time=NaN}}",
        "objectTypes={pdxmesh={scale=1e999}}",
    ] {
        let e = graphics_details_case("", text);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "value"),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
    let e = graphics_details_case("", "entity={scale={min=1 max=2}}");
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "structure"));
}

#[test]
fn graphics_large_finite_numbers_are_not_policy_errors() {
    let e = graphics_details_case(
        "",
        "entity={scale=1e30 cull_radius=1e30 state={state_time=1e30}} objectTypes={pdxmesh={scale=1e30}}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
}

#[test]
fn graphics_looping_uses_a_boolean_contract() {
    for field in ["looping=maybe", "looping={yes}"] {
        let e = graphics_details_case("", &format!("entity={{state={{{field}}}}}"));
        assert!(errors(&e) > 0, "{field}: {:?}", e.diagnostics.rows);
    }
}

#[test]
fn graphics_known_assignment_operators_are_checked() {
    for text in [
        "entity>={name=e}",
        "entity={scale>=1}",
        "entity={state>={name=idle}}",
        "entity={state={looping!=no}}",
        "objectTypes={pdxmesh={animation={id>=idle type=clip}}}",
        "objectTypes={pdxmesh={animation={id=idle type>=clip}}}",
    ] {
        let e = graphics_details_case("animation={name=clip}", text);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "operator"),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn graphics_numeric_expressions_are_not_evaluated_in_a_script_frame() {
    let e = graphics_details_case(
        "",
        "entity={scale=unknown_scale cull_radius=value:missing state={state_time=trigger:num_pops}}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "numeric-information-unresolved"));
    assert!(!e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
}

#[test]
fn graphics_unexpanded_declaration_is_unknown_instead_of_a_fake_bad_shape() {
    let e = graphics_details_case("", "entity={name=e scale=$SCALE$}");
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "graphics-expansion-unverified"));
}

#[test]
fn graphics_clone_inheritance_remains_unknown() {
    let e = graphics_details_case(
        "entity={name=base}",
        "entity={name=child clone=base default_state=idle}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "graphics-inheritance-unverified"));
}

#[test]
fn graphics_nested_entity_references_are_not_treated_as_declarations() {
    let e = graphics_details_case(
        "",
        "entity={name=e state={name=idle event={entity=particle_entity}} game_data={entity=game_entity}}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
}

#[test]
fn graphics_mesh_animation_type_resolves_a_global_definition_across_files() {
    let e = graphics_details_case(
        "animation={name=cruiser_idle_animation file=cruiser_idle.anim}",
        "objectTypes={pdxmesh={name=cruiser_mesh animation={id=idle type=cruiser_idle_animation}}}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
}

#[test]
fn graphics_missing_global_animation_is_detected() {
    let e = graphics_details_case(
        "animation={name=real_clip}",
        "objectTypes={pdxmesh={animation={id=idle type=misspelled_clip}}}",
    );
    assert!(
        e.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "missing-item" && d.message.contains("misspelled_clip"))
    );
}

#[test]
fn graphics_local_animation_id_and_state_clip_are_not_global_names() {
    let e = graphics_details_case(
        "animation={name=long_global_animation_name}",
        "objectTypes={pdxmesh={name=mesh animation={id=idle type=long_global_animation_name}}} entity={name=e pdxmesh=mesh state={name=idle animation=idle}}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
}

#[test]
fn graphics_nested_aliases_and_states_cannot_register_global_animations() {
    for provider in [
        "objectTypes={pdxmesh={animation={name=ghost id=idle type=other}}}",
        "entity={name=e state={name=ghost animation=ghost}}",
    ] {
        let e = graphics_details_case(
            provider,
            "objectTypes={pdxmesh={animation={id=idle type=ghost}}}",
        );
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "missing-item"),
            "{provider}: {:?}",
            e.diagnostics.rows
        );
    }
}

#[test]
fn graphics_unrecognized_animation_wrappers_keep_missing_references_unknown() {
    for provider in [
        "animations={animation={name=wrapped_clip}}",
        "animationTypes={animation={name=wrapped_clip}}",
        "future_container={nested={animation={name=wrapped_clip}}}",
    ] {
        let e = graphics_details_case(
            provider,
            "objectTypes={pdxmesh={animation={id=idle type=wrapped_clip}}}",
        );
        assert_eq!(errors(&e), 0, "{provider}: {:?}", e.diagnostics.rows);
        assert!(has(&e, "graphics-animation-wrapper-unverified"));
        assert!(!e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
    }
}

#[test]
fn graphics_unverified_wrapped_names_are_not_proven_global_registrations() {
    for reference in ["wrapped_clip", "unseen_clip"] {
        let e = graphics_details_case(
            "animations={animation={name=wrapped_clip}}",
            &format!("objectTypes={{pdxmesh={{animation={{id=idle type={reference}}}}}}}"),
        );
        assert_eq!(errors(&e), 0, "{reference}: {:?}", e.diagnostics.rows);
        assert!(has(&e, "graphics-animation-wrapper-unverified"));
    }
}

#[test]
fn graphics_known_global_exists_despite_an_unverified_wrapper() {
    let e = graphics_details_case(
        "animation={name=known_clip} animations={animation={name=wrapped_clip}}",
        "objectTypes={pdxmesh={animation={id=idle type=known_clip}}}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(!has(&e, "graphics-animation-wrapper-unverified"));
}

#[test]
fn graphics_local_namespaces_inside_unknown_wrappers_do_not_open_global_names() {
    for provider in [
        "future_wrapper={objectTypes={pdxmesh={animation={name=ghost id=idle type=other}}}}",
        "future_wrapper={entity={name=e state={animation={name=ghost}}}}",
    ] {
        let e = graphics_details_case(
            provider,
            "objectTypes={pdxmesh={animation={id=idle type=ghost}}}",
        );
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "missing-item"),
            "{provider}: {:?}",
            e.diagnostics.rows
        );
        assert!(!has(&e, "graphics-animation-wrapper-unverified"));
    }
}

#[test]
fn graphics_unknown_wrapper_walk_budget_does_not_create_missing_errors() {
    for reference in ["known_clip", "unseen_clip"] {
        let mut e = make();
        detail_graphics_file(
            &mut e,
            "providers.asset",
            "animation={name=known_clip} unknown={a={b={c={d={name=maybe_clip}}}}}",
            false,
        );
        let nodes = detail_graphics_file(
            &mut e,
            "target.gfx",
            &format!("objectTypes={{pdxmesh={{animation={{id=idle type={reference}}}}}}}"),
            true,
        );
        for node in nodes {
            e.graphics_detail_with_animation_budget(&node, 3);
        }
        assert_eq!(errors(&e), 0, "{reference}: {:?}", e.diagnostics.rows);
        assert!(!e.complete);
        assert!(has(&e, "analysis-limit"));
        assert!(!e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
    }
}

#[test]
fn graphics_animation_alias_shape_is_checked_without_invented_required_fields() {
    for text in [
        "objectTypes={pdxmesh={animation=clip}}",
        "objectTypes={pdxmesh={animation={id={idle} type=clip}}}",
        "objectTypes={pdxmesh={animation={id=idle type={clip}}}}",
    ] {
        let e = graphics_details_case("animation={name=clip}", text);
        assert!(
            e.diagnostics.rows.iter().any(|d| d.key == "structure"),
            "{text}: {:?}",
            e.diagnostics.rows
        );
    }
    for text in [
        "objectTypes={pdxmesh={animation={id=idle}}}",
        "objectTypes={pdxmesh={animation={type=clip}}}",
    ] {
        let e = graphics_details_case("animation={name=clip}", text);
        assert_eq!(errors(&e), 0, "{text}: {:?}", e.diagnostics.rows);
        assert!(has(&e, "graphics-animation-alias-incomplete"));
    }
}

#[test]
fn graphics_unresolved_global_names_keep_the_registry_open() {
    for providers in [
        "animation={name=$CLIP$}",
        "animation={name={bad=1}}",
        "animation={}",
    ] {
        let e = graphics_details_case(
            providers,
            "objectTypes={pdxmesh={animation={id=idle type=unknown_clip}}}",
        );
        assert_eq!(errors(&e), 0, "{providers}: {:?}", e.diagnostics.rows);
        assert!(has(&e, "graphics-animation-reference-unverified"));
    }
}

#[test]
fn graphics_dynamic_animation_reference_does_not_become_missing() {
    let e = graphics_details_case(
        "",
        "objectTypes={pdxmesh={animation={id=idle type=runtime:clip}}}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "dynamic-reference"));
}

#[test]
fn graphics_animation_registration_preserves_identity_case() {
    let good = graphics_details_case(
        "animation={NAME=Clip}",
        "objectTypes={pdxmesh={animation={ID=idle TYPE=Clip}}}",
    );
    assert_eq!(errors(&good), 0, "{:?}", good.diagnostics.rows);
    let bad = graphics_details_case(
        "animation={name=Clip}",
        "objectTypes={pdxmesh={animation={id=idle type=clip}}}",
    );
    assert!(bad.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
}

#[test]
fn graphics_same_name_global_definitions_do_not_create_fake_uniqueness_errors() {
    let e = graphics_details_case(
        "animation={name=clip file=one.anim} animation={name=clip file=two.anim}",
        "objectTypes={pdxmesh={animation={id=idle type=clip}}}",
    );
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
}

fn graphics_small_budget_case(reference: &str, limit: usize) -> Engine {
    let mut e = make();
    detail_graphics_file(
        &mut e,
        "providers.asset",
        "animation={name=seen_clip} animation={name=remote_clip}",
        false,
    );
    let nodes = detail_graphics_file(
        &mut e,
        "target.gfx",
        &format!("objectTypes={{pdxmesh={{animation={{id=idle type={reference}}}}}}}"),
        true,
    );
    for node in nodes {
        e.graphics_detail_with_animation_budget(&node, limit);
    }
    e
}

#[test]
fn graphics_animation_budget_does_not_turn_an_unvisited_provider_into_missing() {
    let e = graphics_small_budget_case("remote_clip", 1);
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(!e.complete);
    assert!(has(&e, "analysis-limit"));
    assert!(has(&e, "graphics-animation-reference-unverified"));
    assert!(!e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
}

#[test]
fn graphics_animation_budget_is_reported_even_for_an_already_seen_name() {
    let e = graphics_small_budget_case("seen_clip", 1);
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(!e.complete);
    assert!(has(&e, "analysis-limit"));
    assert!(!e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
}

#[test]
fn graphics_complete_small_index_preserves_good_and_missing_reference_results() {
    let good = graphics_small_budget_case("remote_clip", 3);
    assert_eq!(errors(&good), 0, "{:?}", good.diagnostics.rows);
    assert!(good.complete);
    assert!(!has(&good, "analysis-limit"));
    let bad = graphics_small_budget_case("misspelled_clip", 3);
    assert!(bad.complete);
    assert!(!has(&bad, "analysis-limit"));
    assert!(bad.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
}

#[test]
fn graphics_whole_dsl_warning_is_preserved_after_detail_checks() {
    let mut e = make();
    detail_graphics_file(
        &mut e,
        "providers.gfx",
        "objectTypes={pdxmesh={name=mesh}}",
        false,
    );
    detail_graphics_file(
        &mut e,
        "target.asset",
        "entity={name=e pdxmesh=mesh state={name=idle state_time=10}}",
        true,
    );
    e.analyze();
    assert_eq!(errors(&e), 0, "{:?}", e.diagnostics.rows);
    assert!(has(&e, "rule-unimplemented"));
}

struct GraphicsOverlayFixture {
    root: std::path::PathBuf,
}
impl GraphicsOverlayFixture {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "tiger-graphics-overlay-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(root.join("game")).unwrap();
        std::fs::create_dir(root.join("mod")).unwrap();
        Self { root }
    }
    fn write(&self, relative: &str, text: &str) {
        let path = self.root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    fn engine(&self) -> Engine {
        let mut e = make();
        e.program = Program::load(
            &self.root.join("game"),
            &[self.root.join("mod")],
            &mut e.parser,
            &mut e.diagnostics,
        )
        .unwrap();
        let nodes = e
            .program
            .files
            .iter()
            .filter(|f| f.target)
            .flat_map(|f| f.ast.iter().cloned())
            .collect::<Vec<_>>();
        for n in nodes {
            e.graphics_detail_contracts(&n);
        }
        e
    }
}
impl Drop for GraphicsOverlayFixture {
    fn drop(&mut self) {
        if self.root.parent() == Some(std::env::temp_dir().as_path())
            && self
                .root
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("tiger-graphics-overlay-"))
        {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

#[test]
fn graphics_same_path_override_removes_the_old_animation_provider() {
    let fixture = GraphicsOverlayFixture::new();
    fixture.write("game/gfx/animations/a.asset", "animation={name=old_clip}");
    fixture.write("mod/gfx/animations/a.asset", "animation={name=new_clip}");
    fixture.write(
        "mod/gfx/models/mesh.gfx",
        "objectTypes={pdxmesh={animation={id=idle type=old_clip}}}",
    );
    let bad = fixture.engine();
    assert!(
        bad.diagnostics
            .rows
            .iter()
            .any(|d| d.key == "missing-item" && d.message.contains("old_clip"))
    );
    assert!(
        bad.program
            .inputs
            .iter()
            .any(|i| i.relative_path == "gfx/animations/a.asset" && i.layer == 1)
    );
    assert!(
        !bad.program
            .inputs
            .iter()
            .any(|i| i.relative_path == "gfx/animations/a.asset" && i.layer == 0)
    );
    fixture.write(
        "mod/gfx/models/mesh.gfx",
        "objectTypes={pdxmesh={animation={id=idle type=new_clip}}}",
    );
    let good = fixture.engine();
    assert_eq!(errors(&good), 0, "{:?}", good.diagnostics.rows);
}

#[test]
fn graphics_replace_path_excludes_old_global_animation_definitions() {
    let fixture = GraphicsOverlayFixture::new();
    fixture.write("game/gfx/animations/a.asset", "animation={name=old_clip}");
    fixture.write("mod/descriptor.mod", "replace_path=\"gfx/animations\"");
    fixture.write(
        "mod/gfx/models/mesh.gfx",
        "objectTypes={pdxmesh={animation={id=idle type=old_clip}}}",
    );
    let e = fixture.engine();
    assert!(e.diagnostics.rows.iter().any(|d| d.key == "missing-item"));
    assert!(
        !e.program
            .inputs
            .iter()
            .any(|i| i.relative_path == "gfx/animations/a.asset")
    );
}
