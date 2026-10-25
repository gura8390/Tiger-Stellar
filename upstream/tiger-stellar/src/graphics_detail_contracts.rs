//! Additional graphics declaration contracts, without claiming rendering or a
//! complete asset DSL. The existing whole-graphics coverage warning remains.
//!
//! Installed 4.5.2 witnesses (relative to the game root):
//! gfx/models/ships/mammalian_01/_mammalian_01_ships_entities.asset:30-47,
//! SHA256 d350cb9494c297d9b91a8809a317d618e30ea6df5fb511dd80482a78d90c2491;
//! _mammalian_01_ships_meshes.gfx:254-262,
//! SHA256 e0c930f540c3e135b7f81ccf3de47f2882351b051afe8b301504d68d419ca63b;
//! _mammalian_01_ships_animations.asset:32-49,
//! SHA256 fec83f5ca519505315949eac460639285bae323b664db40210b5158e3e00566d.
//! The mesh's animation.id is a local clip alias; animation.type names a global
//! top-level animation declaration. entity.state.animation uses the local alias.
use crate::{ast::Node, engine::Engine, loader::Program};
use std::collections::BTreeSet;

const ANIMATION_INDEX_ROOT_LIMIT: usize = 32_768;

#[derive(Default)]
struct AnimationIndex {
    names: BTreeSet<String>,
    open: bool,
    wrapper_unverified: bool,
    exhausted: bool,
}

fn dynamic(value: &str) -> bool {
    value.contains(['$', '@', ':', '|', '[', ']'])
}

impl AnimationIndex {
    fn collect(program: &Program) -> Self {
        Self::collect_bounded(program, ANIMATION_INDEX_ROOT_LIMIT)
    }

    fn collect_bounded(program: &Program, limit: usize) -> Self {
        let mut result = Self::default();
        let mut work = 0;
        let mut wrapper_work = 0;
        // Program::load has already applied same-path replacement and
        // replace_path. Never index old source providers from asset_files or
        // raw game directories. Same-name declarations need only existence
        // here; their composition and playback content are not interpreted.
        'files: for file in &program.files {
            if !matches!(file.category.as_str(), "gfx" | "interface") {
                continue;
            }
            for node in &file.ast {
                work += 1;
                if work > limit {
                    result.open = true;
                    result.exhausted = true;
                    break 'files;
                }
                if node.macro_parts.is_some() {
                    result.open = true;
                    continue;
                }
                if !node.key.eq_ignore_ascii_case("animation") {
                    result.inspect_unknown_wrapper(node, &mut wrapper_work, limit);
                    if result.exhausted {
                        break 'files;
                    }
                    continue;
                }
                let names = node
                    .body()
                    .iter()
                    .filter(|field| field.key.eq_ignore_ascii_case("name"))
                    .collect::<Vec<_>>();
                if node.children.is_none() || node.op.as_str() != "=" || names.len() != 1 {
                    result.open = true;
                    continue;
                }
                let name = names[0];
                if name.children.is_some()
                    || name.value.is_none()
                    || name.op.as_str() != "="
                    || name.scalar().is_empty()
                    || dynamic(name.scalar())
                {
                    result.open = true;
                } else {
                    result.names.insert(name.scalar().into());
                }
            }
        }
        result
    }

    fn inspect_unknown_wrapper(&mut self, root: &Node, work: &mut usize, limit: usize) {
        // Native evidence establishes these as local object namespaces. A
        // state clip or mesh alias cannot define a global animation, even if
        // an extra local field happens to be called name.
        if root.key.eq_ignore_ascii_case("entity") || root.key.eq_ignore_ascii_case("pdxmesh") {
            return;
        }
        // Other containers are not a proven closed grammar. A possible
        // animation declaration under an unfamiliar wrapper prevents a hard
        // missing-reference conclusion; it is not itself registered as a
        // confirmed global provider. Iterate without recursive call depth.
        let mut levels = vec![root.body().iter()];
        while let Some(level) = levels.last_mut() {
            let Some(node) = level.next() else {
                levels.pop();
                continue;
            };
            if *work >= limit {
                self.open = true;
                self.exhausted = true;
                return;
            }
            *work += 1;
            if node.key.eq_ignore_ascii_case("entity") || node.key.eq_ignore_ascii_case("pdxmesh") {
                continue;
            }
            if node.macro_parts.is_some()
                || (node.key.eq_ignore_ascii_case("animation")
                    && node
                        .body()
                        .iter()
                        .any(|field| field.key.eq_ignore_ascii_case("name")))
            {
                self.open = true;
                self.wrapper_unverified = true;
                return;
            }
            if !node.body().is_empty() {
                levels.push(node.body().iter());
            }
        }
    }
}

impl Engine {
    fn graphics_assignment(&mut self, n: &Node) {
        if n.op.as_str() != "=" {
            self.error(n, "operator", "graphics declaration fields require =", None);
        }
    }

    fn graphics_detail_scalar(&mut self, n: &Node) {
        self.graphics_assignment(n);
        if self.scalar_field(n) && dynamic(n.scalar()) {
            self.gap(
                n,
                "graphics field requires runtime or reader expansion",
                "graphics-expression-unverified",
                None,
            );
        }
    }

    fn graphics_detail_number(&mut self, n: &Node) {
        self.graphics_assignment(n);
        // No Context/State: graphics quantities do not borrow script variables,
        // evaluate value: queries, or acquire an arbitrary engine receiver.
        self.numeric(n, None);
    }

    fn graphics_entity_state(&mut self, n: &Node) {
        self.graphics_assignment(n);
        if !self.block(n) {
            return;
        }
        for field in n.body() {
            match field.key.to_ascii_lowercase().as_str() {
                "name" | "animation" | "next_state" => self.graphics_detail_scalar(field),
                "state_time" => self.graphics_detail_number(field),
                "looping" => {
                    self.graphics_assignment(field);
                    if !self.scalar_field(field) {
                        continue;
                    }
                    if dynamic(field.scalar()) {
                        self.gap(
                            field,
                            "graphics looping flag requires expansion",
                            "graphics-expression-unverified",
                            None,
                        );
                    } else {
                        self.boolean(field);
                    }
                }
                // Events, probabilities, blend policies and other selectors
                // retain the whole-graphics coverage boundary. Repeated named
                // death states are valid native probability alternatives.
                _ => {}
            }
        }
    }

    fn graphics_entity_details(&mut self, n: &Node) {
        self.graphics_assignment(n);
        if !self.block(n) {
            return;
        }
        for field in n.body() {
            match field.key.to_ascii_lowercase().as_str() {
                "name" | "pdxmesh" | "default_state" => self.graphics_detail_scalar(field),
                "scale" | "cull_radius" => self.graphics_detail_number(field),
                "state" => self.graphics_entity_state(field),
                "clone" => {
                    self.graphics_detail_scalar(field);
                    self.gap(
                        field,
                        "entity clone state and animation inheritance is not resolved",
                        "graphics-inheritance-unverified",
                        None,
                    );
                }
                _ => {}
            }
        }
    }

    fn graphics_animation_type(&mut self, n: &Node, index: &mut Option<AnimationIndex>) {
        self.graphics_assignment(n);
        if !self.scalar_field(n) {
            return;
        }
        if dynamic(n.scalar()) {
            self.gap(
                n,
                "global animation reference requires expansion",
                "dynamic-reference",
                None,
            );
            return;
        }
        if index.is_none() {
            *index = Some(AnimationIndex::collect(&self.program));
        }
        let registry = index.as_ref().unwrap();
        if registry.exhausted {
            self.complete = false;
            self.gap(
                n,
                "global animation declaration index budget reached",
                "analysis-limit",
                None,
            );
        }
        if registry.names.contains(n.scalar()) {
            return;
        }
        if registry.open || matches!(n.scalar(), "none" | "default" | "random") {
            if registry.wrapper_unverified {
                self.gap(
                    n,
                    "an unmodeled graphics wrapper may provide a global animation declaration",
                    "graphics-animation-wrapper-unverified",
                    None,
                );
            } else {
                self.gap(
                    n,
                    "global animation registration cannot be closed for this reference",
                    "graphics-animation-reference-unverified",
                    None,
                );
            }
        } else {
            self.error(
                n,
                "missing-item",
                format!("unknown global animation reference `{}`", n.scalar()),
                None,
            );
        }
    }

    fn graphics_mesh_details(&mut self, n: &Node, index: &mut Option<AnimationIndex>) {
        self.graphics_assignment(n);
        if !self.block(n) {
            return;
        }
        for field in n.body() {
            match field.key.to_ascii_lowercase().as_str() {
                "name" | "file" => self.graphics_detail_scalar(field),
                "scale" => self.graphics_detail_number(field),
                "animation" => {
                    self.graphics_assignment(field);
                    if !self.block(field) {
                        continue;
                    }
                    if !field
                        .body()
                        .iter()
                        .any(|c| c.key.eq_ignore_ascii_case("id"))
                        || !field
                            .body()
                            .iter()
                            .any(|c| c.key.eq_ignore_ascii_case("type"))
                    {
                        self.gap(
                            field,
                            "mesh animation alias has an unestablished id/type contract",
                            "graphics-animation-alias-incomplete",
                            None,
                        );
                    }
                    for item in field.body() {
                        if item.key.eq_ignore_ascii_case("id") {
                            self.graphics_detail_scalar(item);
                        } else if item.key.eq_ignore_ascii_case("type") {
                            self.graphics_animation_type(item, index);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn graphics_details_root(&mut self, n: &Node, index: &mut Option<AnimationIndex>) {
        if n.macro_parts.is_some() {
            self.gap(
                n,
                "graphics declaration needs macro expansion",
                "graphics-expansion-unverified",
                None,
            );
            return;
        }
        match n.key.to_ascii_lowercase().as_str() {
            "entity" => self.graphics_entity_details(n),
            "pdxmesh" => self.graphics_mesh_details(n, index),
            "animation" => {
                self.graphics_assignment(n);
                if self.block(n) {
                    for field in n.body() {
                        if matches!(field.key.to_ascii_lowercase().as_str(), "name" | "file") {
                            self.graphics_detail_scalar(field);
                        }
                    }
                }
            }
            "objecttypes" => {
                self.graphics_assignment(n);
                if self.block(n) {
                    for child in n.body() {
                        if matches!(
                            child.key.to_ascii_lowercase().as_str(),
                            "pdxmesh" | "entity"
                        ) {
                            self.graphics_details_root(child, index);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Call once per graphics/interface top-level node, alongside the existing
    /// path/reference checks and whole-graphics coverage warning. The animation
    /// registry is lazy and local to this one node walk, never a payload memo or
    /// an engine-level lifetime/type/availability cache.
    pub(crate) fn graphics_detail_contracts(&mut self, n: &Node) {
        self.graphics_details_root(n, &mut None);
    }

    #[cfg(test)]
    pub(crate) fn graphics_detail_with_animation_budget(&mut self, n: &Node, limit: usize) {
        let mut index = Some(AnimationIndex::collect_bounded(&self.program, limit));
        self.graphics_details_root(n, &mut index);
    }
}
