//! Creation callbacks retained by the 0.1.8 contracts and current shipped
//! scripts. An arrival callback is future execution, not a caller continuation.
//!
//! Evidence: effects.log create_random_fleet; machine_age_crisis_events.txt
//! start_situation.effect and scalar auto_move_to_planet.arrival_effect;
//! 00_scripted_effects.txt spawn_megastructure.init_effect. The latter two
//! callbacks are absent from the engine's abbreviated parameter descriptions.
use crate::{
    ast::Node,
    domain::{Availability, Context, State, Types},
    engine::Engine,
};

#[derive(Clone, Copy)]
enum CreationField {
    Item(&'static str),
    Scope(Option<&'static str>),
    Number,
    Boolean,
    Scalar,
}

fn parent_field(parent: &str, child: &str) -> Option<CreationField> {
    match (parent, child) {
        ("start_situation", "type") => Some(CreationField::Item("situations")),
        ("start_situation", "target") | ("spawn_megastructure", "coords_from") => {
            Some(CreationField::Scope(None))
        }
        ("spawn_megastructure", "type") => Some(CreationField::Item("megastructures")),
        ("spawn_megastructure", "owner") => Some(CreationField::Scope(Some("country"))),
        ("spawn_megastructure", "planet") | ("auto_move_to_planet", "target") => {
            Some(CreationField::Scope(Some("planet")))
        }
        ("spawn_megastructure", "name" | "graphical_culture") => Some(CreationField::Scalar),
        ("spawn_megastructure", "orbit_distance" | "orbit_angle") => Some(CreationField::Number),
        ("auto_move_to_planet", "clear_auto_move_on_arrival") => Some(CreationField::Boolean),
        _ => None,
    }
}

impl Engine {
    /// Context-independent callback grammar must also be visited on paths
    /// whose guards make their execution impossible.
    pub(crate) fn creation_callback_structure(&mut self, parent: &Node) {
        if matches!(
            parent.key.as_str(),
            "start_situation" | "spawn_megastructure" | "auto_move_to_planet"
        ) {
            if parent.op.as_str() != "=" {
                self.error(
                    parent,
                    "operator",
                    "creation or movement effect requires =",
                    None,
                );
            }
            if !self.block(parent) {
                return;
            }
            let required = if parent.key == "auto_move_to_planet" {
                "target"
            } else {
                "type"
            };
            if parent.field(required).is_none() {
                self.error(
                    parent,
                    "missing-field",
                    format!("{} requires {required}", parent.key),
                    None,
                );
            }
            for child in parent.body() {
                if let Some(contract) = parent_field(&parent.key, &child.key) {
                    if child.op.as_str() != "=" {
                        self.error(
                            child,
                            "operator",
                            "creation or movement parameter requires =",
                            None,
                        );
                    }
                    if self.scalar_field(child) {
                        match contract {
                            CreationField::Item(category)
                                if !child.scalar().contains(['$', '@', ':', '|', '[', ']']) =>
                            {
                                self.symbol_reference(category, child.scalar(), child);
                            }
                            CreationField::Boolean => self.boolean(child),
                            CreationField::Number => {
                                if let Ok(number) = child.scalar().parse::<f64>()
                                    && !number.is_finite()
                                {
                                    self.error(
                                        child,
                                        "value",
                                        "numeric value must be finite",
                                        None,
                                    );
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        for child in parent.body() {
            let creation = matches!(
                (parent.key.as_str(), child.key.as_str()),
                ("start_situation", "effect")
                    | ("spawn_megastructure", "init_effect")
                    | ("create_random_fleet", "effect")
            );
            let arrival = parent.key == "auto_move_to_planet" && child.key == "arrival_effect";
            if !creation && !arrival {
                continue;
            }
            if child.op.as_str() != "=" {
                self.error(
                    child,
                    "operator",
                    if creation {
                        "creation callback requires ="
                    } else {
                        "arrival callback requires ="
                    },
                    None,
                );
            }
            if creation {
                if self.block(child) {
                    self.structure_script(child.body(), true);
                }
            } else if child.children.is_some() {
                // The form remains unverified at semantic analysis, but a
                // misspelled effects token does not become valid syntax.
                self.structure_script(child.body(), true);
            } else if self.scalar_field(child) {
                let name = child.scalar();
                if !name.contains(['$', '@', '[', ']'])
                    && !self.program.scripts.contains_key(&(true, name.into()))
                {
                    self.error(
                        child,
                        "missing-item",
                        format!("unknown arrival scripted effect {name}"),
                        None,
                    );
                }
            }
        }
    }

    /// Only fields with evidenced contracts are consumed; new or otherwise
    /// unknown engine fields still reach the generic coverage diagnostic.
    pub(crate) fn creation_parent_field(
        &mut self,
        parent: &Node,
        child: &Node,
        ctx: &Context,
        state: &mut State,
        inputs: &State,
    ) -> bool {
        let Some(contract) = parent_field(&parent.key, &child.key) else {
            return false;
        };
        if child.op.as_str() != "=" {
            self.error(
                child,
                "operator",
                "creation or movement parameter requires =",
                None,
            );
        }
        if !self.scalar_field(child) {
            return true;
        }
        match contract {
            CreationField::Item(category) => {
                self.symbol_reference(category, child.scalar(), child);
            }
            CreationField::Scope(accepted) => {
                let target = self.resolve(child.scalar(), ctx, state, child, false);
                if let Some(accepted) = accepted {
                    self.compatible(
                        &target.this,
                        &[accepted.into()],
                        child,
                        &format!("{} {}", parent.key, child.key),
                        ctx,
                    );
                }
            }
            CreationField::Number => {
                self.numeric_in_state(child, ctx, inputs);
            }
            CreationField::Boolean => self.boolean(child),
            CreationField::Scalar => {}
        }
        true
    }

    /// Called before the generic API-parameter whitelist: shipped callbacks
    /// omitted by that whitelist must still be checked in their own context.
    pub(crate) fn creation_callback(
        &mut self,
        parent: &Node,
        child: &Node,
        ctx: &Context,
        state: &mut State,
    ) -> bool {
        let output = match (parent.key.as_str(), child.key.as_str()) {
            ("start_situation", "effect") => "situation",
            ("spawn_megastructure", "init_effect") => "megastructure",
            ("create_random_fleet", "effect") => "fleet",
            ("auto_move_to_planet", "arrival_effect") => {
                self.arrival_callback(parent, child, ctx, state);
                return true;
            }
            _ => return false,
        };
        let frame = ctx.push(
            Types::new(output),
            &format!("created-callback:{}:{}", parent.id, child.key),
        );
        let depth = self.chain.len();
        self.chain.push(
            serde_json::json!({
                "kind":"creation-callback", "effect":parent.key,
                "callback":child.key, "path":child.source, "line":child.line,
                "parent_line":parent.line, "deferred":false,
                "input_context":ctx, "callback_context":frame
            })
            .into(),
        );
        if child.op.as_str() != "=" {
            self.error(child, "operator", "creation callback requires =", Some(ctx));
        }
        if self.block(child) {
            let mut selected = state.clone();
            // A new receiver cannot inherit numbers from an earlier creation
            // at the same lexical statement or from an owner lookup cache.
            selected.retain_number_anchors();
            selected.forget_number_object(&frame.object);
            self.effects(child.body(), &frame, &mut selected);
            // Static analysis does not prove construction succeeds. Join all
            // facts, including bindings and guard correlations, with the path
            // on which the callback did not execute.
            *state = state.join(&selected);
        }
        self.chain.truncate(depth);
        true
    }

    fn arrival_callback(&mut self, parent: &Node, child: &Node, ctx: &Context, state: &State) {
        // Current native call sites establish a fleet receiver. The effect
        // also accepts ships, but that is not proof of its callback's THIS.
        let fleet = !ctx.this.unknown
            && !ctx.this.known.is_empty()
            && ctx.this.known.iter().all(|kind| kind == "fleet");
        let mut frame = Context::unknown_caller();
        frame.object = format!("arrival:{}:{}", parent.id, child.id);
        frame.root_object = format!("unknown-arrival-root:{}", parent.id);
        if fleet && child.children.is_none() {
            frame.this = Types::new("fleet");
        }
        let depth = self.chain.len();
        self.chain.push(
            serde_json::json!({
                "kind":"arrival-callback", "effect":parent.key,
                "callback":child.key, "path":child.source, "line":child.line,
                "parent_line":parent.line, "deferred":true,
                "input_context":ctx, "callback_context":frame
            })
            .into(),
        );
        if child.op.as_str() != "=" {
            self.error(child, "operator", "arrival callback requires =", Some(ctx));
        }
        self.gap(
            child,
            "arrival callback is future execution; ROOT, FROM, PREV and captured target lifetime are not established",
            "arrival-callback-context-unverified",
            Some(&frame),
        );
        if !fleet {
            self.gap(
                child,
                "arrival callback THIS contract is established only for the shipped fleet entry",
                "arrival-callback-input-unverified",
                Some(&frame),
            );
        }
        let mut future = state.clone();
        future.forget_numbers();
        future.clear_correlations();
        // Between registration and arrival a named object can disappear or be
        // rebound, including a previously absent target. Retain candidates for
        // provenance, but never use them as an exhaustive future type proof.
        for binding in future.bindings.values_mut() {
            binding.availability = Availability::Maybe;
            binding.types.unknown = true;
        }
        if child.children.is_some() {
            self.gap(
                child,
                "block-valued arrival callback is not evidenced by current shipped call sites; body is checked with unknown THIS",
                "arrival-callback-form-unverified",
                Some(&frame),
            );
            self.effects(child.body(), &frame, &mut future);
        } else if self.scalar_field(child) {
            let name = child.scalar();
            if name.contains(['$', '@', '[', ']']) {
                self.gap(
                    child,
                    "dynamic arrival scripted-effect identity requires expansion",
                    "dynamic-callback-identity",
                    Some(&frame),
                );
            } else if !self.program.scripts.contains_key(&(true, name.into())) {
                self.error(
                    child,
                    "missing-item",
                    format!("unknown arrival scripted effect {name}"),
                    Some(&frame),
                );
            } else {
                let call = Node {
                    key: name.into(),
                    value: Some("yes".into()),
                    ..child.clone_header()
                };
                self.call(&call, true, &frame, &mut future);
            }
        }
        // Neither the arrival receiver nor its targets/numbers are guaranteed
        // to be changed before the next statement at the registration site.
        self.chain.truncate(depth);
    }
}
