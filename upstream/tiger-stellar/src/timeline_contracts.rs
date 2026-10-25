//! add_timeline_event: current effects.log:3810-3818 and the shipped
//! common/timeline_events/00_timeline_defines.txt:5-24, 34-76.
//! The latter documents override_types and its positional target contract,
//! which the exported effect parameter list does not capture completely.
use crate::{
    ast::Node,
    domain::{Context, State},
    engine::Engine,
};
use std::collections::BTreeSet;

fn dynamic(value: &str) -> bool {
    value.contains(['$', '@', '[', ']'])
}

impl Engine {
    /// Caller-independent grammar/references, also used for unreachable code.
    pub(crate) fn timeline_structure(&mut self, n: &Node) -> bool {
        if n.key != "add_timeline_event" {
            return false;
        }
        if !self.block(n) {
            return true;
        }
        if n.op.as_str() != "=" {
            self.error(n, "operator", "add_timeline_event requires =", None);
        }
        if n.field("type").is_none() {
            self.error(n, "missing-field", "add_timeline_event requires type", None);
        }
        if n.body()
            .iter()
            .any(|f| f.key.starts_with("override_") && f.key != "override_id")
            && n.field("override_id").is_none()
        {
            self.error(
                n,
                "missing-field",
                "timeline overrides require override_id",
                None,
            );
        }
        let mut seen = BTreeSet::new();
        for field in n.body() {
            if !seen.insert(field.key.as_str()) {
                self.error(
                    field,
                    "duplicate-field",
                    "duplicate timeline parameter",
                    None,
                );
            }
            if field.op.as_str() != "=" {
                self.error(field, "operator", "timeline parameter requires =", None);
            }
            match field.key.as_str() {
                "type" => {
                    if self.scalar_field(field) {
                        self.symbol_reference("timeline_events", field.scalar(), field);
                    }
                }
                "override_id" => {
                    if self.scalar_field(field) && field.scalar().is_empty() {
                        self.error(
                            field,
                            "value",
                            "timeline override_id must not be empty",
                            None,
                        );
                    }
                }
                "override_tooltip" | "override_tooltip_delayed" => {
                    if self.scalar_field(field) {
                        self.localisation(field);
                        if !field.scalar().is_empty() {
                            self.timeline_gui_binding_gap(field);
                        }
                    }
                }
                "override_text" | "override_texture" => self.timeline_overrides(field),
                "override_types" => {
                    self.timeline_type_list(field);
                }
                "targets" => {
                    if let Some(targets) = self.timeline_list(field)
                        && targets.len() > 10
                    {
                        self.error(
                            field,
                            "value",
                            "timeline events support at most ten targets",
                            None,
                        );
                    }
                }
                "date" => {
                    if self.scalar_field(field) {
                        let parts = field.scalar().split('.').collect::<Vec<_>>();
                        if parts.len() != 3 || parts.iter().any(|p| p.parse::<u32>().is_err()) {
                            self.gap(
                                field,
                                "timeline date expression/calendar requires verification",
                                "timeline-date-unverified",
                                None,
                            );
                        }
                    }
                }
                _ => self.gap(
                    field,
                    format!(
                        "timeline parameter has no established contract: {}",
                        field.key
                    ),
                    "timeline-parameter-unverified",
                    None,
                ),
            }
        }
        let types = self.timeline_target_types(n);
        let targets = n.field("targets").and_then(|f| self.timeline_list(f));
        let target_count = targets.as_ref().map_or(0, Vec::len);
        if let Some(types) = types
            && types.len() != target_count
            && (n.field("targets").is_none() || targets.is_some())
        {
            self.error(
                n,
                "timeline-target-count",
                format!(
                    "timeline target types require {} targets, got {target_count}",
                    types.len()
                ),
                None,
            );
        }
        true
    }

    /// Resolve target selectors in the effect's country frame. No facts from
    /// reading a target or its metadata are exported into the input state.
    pub(crate) fn timeline_parameters(&mut self, n: &Node, ctx: &Context, state: &State) {
        self.timeline_structure(n);
        self.compatible(&ctx.this, &["country".into()], n, "add_timeline_event", ctx);
        let types = self.timeline_target_types(n);
        let targets = n
            .field("targets")
            .and_then(|field| self.timeline_list(field));
        let Some(targets) = targets else {
            return;
        };
        if types.is_none() && !targets.is_empty() {
            self.gap(
                n,
                "timeline target type list is not established by an override or database entry",
                "timeline-target-types-unverified",
                Some(ctx),
            );
        }
        for (index, target) in targets.iter().enumerate() {
            if !self.is_scope(target.scalar()) {
                self.gap(
                    target,
                    "timeline target selector contract requires verification",
                    "timeline-target-selector-unverified",
                    Some(ctx),
                );
                continue;
            }
            let frame = self.resolve(target.scalar(), ctx, state, target, false);
            if let Some(kind) = types.as_ref().and_then(|kinds| kinds.get(index)) {
                self.compatible(
                    &frame.this,
                    std::slice::from_ref(kind),
                    target,
                    "timeline target",
                    &frame,
                );
            }
        }
    }

    fn timeline_list(&mut self, n: &Node) -> Option<Vec<Node>> {
        if !self.block(n) {
            return None;
        }
        let mut entries = Vec::new();
        let mut valid = true;
        for child in n.body() {
            if !child.key.is_empty() || child.children.is_some() || child.value.is_none() {
                self.error(
                    child,
                    "structure",
                    "timeline list expects anonymous scalar entries",
                    None,
                );
                valid = false;
            } else {
                entries.push(child.clone());
            }
        }
        valid.then_some(entries)
    }

    fn timeline_type_list(&mut self, n: &Node) -> Option<Vec<String>> {
        let entries = self.timeline_list(n)?;
        let mut kinds = Vec::new();
        let mut valid = true;
        for entry in entries {
            let kind = entry.scalar();
            if dynamic(kind) {
                self.gap(
                    &entry,
                    "dynamic timeline target type requires expansion",
                    "timeline-target-types-unverified",
                    None,
                );
                valid = false;
            } else if !crate::rules::SCOPES
                .split_whitespace()
                .any(|scope| scope == kind)
            {
                self.error(
                    &entry,
                    "value",
                    format!("unknown timeline target scope type {kind}"),
                    None,
                );
                valid = false;
            } else {
                if !matches!(kind, "country" | "planet") {
                    // The guide documents country/planet and explicitly says
                    // other persistent/dead types require game code support.
                    // A general script scope enum is not a storage registry.
                    self.gap(&entry, "timeline persistent/dead storage support for this scope type is not established", "timeline-target-storage-unverified", None);
                }
                kinds.push(kind.into());
            }
        }
        valid.then_some(kinds)
    }

    fn timeline_target_types(&mut self, n: &Node) -> Option<Vec<String>> {
        if let Some(types) = n.field("override_types") {
            return self.timeline_type_list(types);
        }
        let definition = self
            .program
            .definitions
            .get("timeline_events")?
            .get(n.val("type")?)?
            .clone();
        // An absent database field is not evidence of an empty type contract.
        // Empty target arrays need no receiver assumption; nonempty arrays
        // without either type source retain an explicit unknown boundary.
        self.timeline_type_list(definition.field("types")?)
    }

    fn timeline_overrides(&mut self, n: &Node) {
        let Some(entries) = self.timeline_list(n) else {
            return;
        };
        let mut parsed_binding = false;
        for entry in entries {
            let raw = entry.scalar();
            if dynamic(raw) {
                self.gap(
                    &entry,
                    "dynamic timeline GUI override requires expansion",
                    "timeline-override-entry-unverified",
                    None,
                );
                continue;
            }
            // The exported example also puts two GUI:GFX pairs inside one
            // quoted scalar. Accept that documented variant without treating
            // each word as a trigger or an effect.
            let parts = raw.split_whitespace().collect::<Vec<_>>();
            if parts.is_empty() {
                self.error(
                    &entry,
                    "value",
                    "timeline override expects GUI_element:reference",
                    None,
                );
                continue;
            }
            if parts.len() > 1 && parts.iter().any(|part| !part.contains(':')) {
                self.gap(
                    &entry,
                    "timeline GUI override contains unverified literal text syntax",
                    "timeline-override-entry-unverified",
                    None,
                );
                continue;
            }
            for part in parts {
                let Some((gui, value)) = part.split_once(':') else {
                    self.error(
                        &entry,
                        "value",
                        "timeline override expects GUI_element:reference",
                        None,
                    );
                    continue;
                };
                if gui.is_empty() || value.is_empty() {
                    self.error(
                        &entry,
                        "value",
                        "timeline override requires nonempty GUI element and reference",
                        None,
                    );
                    continue;
                }
                parsed_binding = true;
                let probe = Node {
                    value: Some(value.into()),
                    ..entry.clone()
                };
                if n.key == "override_text" {
                    self.localisation(&probe);
                } else if value.contains(['/', '\\'])
                    || value.ends_with(".dds")
                    || value.ends_with(".tga")
                {
                    self.gap(
                        &probe,
                        "timeline texture override is not a documented GFX sprite reference",
                        "timeline-texture-reference-unverified",
                        None,
                    );
                } else {
                    // GFX_ is a naming convention, not a required prefix for a
                    // registered sprite. Fixed symbol names can be checked.
                    self.symbol_reference("gfx_spritetype", value, &probe);
                }
            }
        }
        if parsed_binding {
            self.timeline_gui_binding_gap(n);
        }
    }

    fn timeline_gui_binding_gap(&mut self, n: &Node) {
        self.gap(
            n,
            "timeline GUI control names, hierarchy and display binding are not validated; localization and sprite references are checked separately",
            "timeline-gui-binding-unverified",
            None,
        );
    }
}
