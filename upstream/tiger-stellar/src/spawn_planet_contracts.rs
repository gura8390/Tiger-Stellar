//! spawn_planet: the exported API only establishes a system receiver. Field
//! contracts below come from the retained 0.1.8 contract and bounded shipped
//! examples, not from the field names alone.
//!
//! 00_scripted_effects:6961-6974: class/location(megastructure)/angle offsets;
//! shroud_shadows_scripted_effects:2921-2929: random_colonizable, location=none,
//! numeric orbit ranges and init_effect; first_contact_dlc_effects:589-598:
//! orbit_location, size, distance offset and has_ring;
//! federations_event_effects:1803-1811: none deposits/modifiers and flag list.
use crate::{
    ast::Node,
    domain::{Context, State, Types},
    engine::Engine,
};
use std::collections::BTreeSet;

fn number_field(name: &str) -> bool {
    matches!(
        name,
        "size" | "orbit_angle" | "orbit_distance" | "orbit_angle_offset" | "orbit_distance_offset"
    )
}
fn bool_field(name: &str) -> bool {
    matches!(
        name,
        "has_ring"
            | "generate_random_name"
            | "spawn_beyond_gravity_well"
            | "check_overlap"
            | "orbit_location"
    )
}
fn numeric_range(name: &str) -> bool {
    matches!(name, "orbit_angle" | "orbit_distance")
}
fn callback(name: &str) -> bool {
    matches!(name, "init_effect" | "init_effects")
}

impl Engine {
    fn spawn_class_reference(&mut self, n: &Node, ctx: Option<&Context>, state: Option<&State>) {
        if !self.scalar_field(n) {
            return;
        }
        let value = n.scalar();
        if self
            .program
            .entities
            .get("planet_classes")
            .is_some_and(|classes| classes.contains(value))
        {
            self.symbol_reference("planet_classes", value, n);
            return;
        }
        if value == "random_colonizable" {
            return;
        }
        if value.starts_with("random") {
            self.gap(
                n,
                "spawn_planet random class selector is not evidenced by the current contract",
                "spawn-planet-class-selector-unverified",
                ctx,
            );
        } else if self.is_scope(value) {
            if let (Some(ctx), Some(state)) = (ctx, state) {
                self.resolve(value, ctx, state, n, false);
            }
            self.gap(n, "spawn_planet scope-valued class source requires verification; it is not a literal class identity", "spawn-planet-class-source-unverified", ctx);
        } else {
            self.symbol_reference("planet_classes", value, n);
        }
    }

    fn spawn_number_shape(&mut self, n: &Node) {
        if n.children.is_none() {
            if self.scalar_field(n)
                && let Ok(value) = n.scalar().parse::<f64>()
                && !value.is_finite()
            {
                self.error(n, "value", "numeric value must be finite", None);
            }
            return;
        }
        if !numeric_range(&n.key) {
            self.scalar_field(n);
            return;
        }
        let mut seen = BTreeSet::new();
        for field in n.body() {
            if !matches!(field.key.as_str(), "min" | "max") {
                self.error(
                    field,
                    "unknown-field",
                    "spawn_planet orbit range expects min/max fields",
                    None,
                );
                continue;
            }
            if !seen.insert(field.key.as_str()) {
                self.error(
                    field,
                    "duplicate-field",
                    "duplicate spawn_planet orbit range endpoint",
                    None,
                );
            }
            if field.op.as_str() != "=" {
                self.error(
                    field,
                    "operator",
                    "spawn_planet range endpoint requires =",
                    None,
                );
            }
            if self.scalar_field(field)
                && let Ok(value) = field.scalar().parse::<f64>()
                && !value.is_finite()
            {
                self.error(field, "value", "numeric value must be finite", None);
            }
        }
        if let (Some(min), Some(max)) = (
            n.val("min").and_then(|v| v.parse::<f64>().ok()),
            n.val("max").and_then(|v| v.parse::<f64>().ok()),
        ) && min.is_finite()
            && max.is_finite()
            && min > max
        {
            self.error(
                n,
                "range-order",
                "spawn_planet orbit range min exceeds max",
                None,
            );
        }
    }

    fn spawn_flags_shape(&mut self, n: &Node) {
        if !self.block(n) {
            return;
        }
        for entry in n.body() {
            if !entry.key.is_empty() || entry.children.is_some() || entry.value.is_none() {
                self.error(
                    entry,
                    "structure",
                    "spawn_planet flags expects bare scalar flag names",
                    None,
                );
            }
        }
    }

    pub(crate) fn spawn_planet_structure(&mut self, n: &Node) {
        if n.op.as_str() != "=" {
            self.error(n, "operator", "spawn_planet requires =", None);
        }
        if !self.block(n) {
            return;
        }
        if n.field("class").is_none() {
            self.error(n, "missing-field", "spawn_planet requires class", None);
        }
        for field in n.body() {
            let known = number_field(&field.key)
                || bool_field(&field.key)
                || callback(&field.key)
                || matches!(
                    field.key.as_str(),
                    "class" | "location" | "name" | "flags" | "deposit_blockers" | "modifiers"
                );
            if known && field.op.as_str() != "=" {
                self.error(field, "operator", "spawn_planet parameter requires =", None);
            }
            if number_field(&field.key) {
                self.spawn_number_shape(field);
            } else if bool_field(&field.key) {
                if self.scalar_field(field) {
                    self.boolean(field);
                }
            } else if callback(&field.key) {
                if self.block(field) {
                    self.structure_script(field.body(), true);
                }
            } else {
                match field.key.as_str() {
                    "class" => self.spawn_class_reference(field, None, None),
                    "location" | "name" => {
                        self.scalar_field(field);
                    }
                    "flags" => self.spawn_flags_shape(field),
                    _ => {}
                }
            }
        }
    }

    fn spawn_numeric_value(&mut self, n: &Node, ctx: &Context, inputs: &State) -> Option<f64> {
        if n.scalar().contains("trigger:")
            || n.scalar().contains("modifier:")
            || n.scalar().contains("value:")
        {
            // The general variable guide establishes numeric variable reads,
            // but current spawn examples do not establish query input scopes.
            // Unknown evaluation objects must not borrow the caller's stored
            // constants merely because their scope types were made unknown.
            // Explicit target bindings remain available through inputs.
            let mut frame = Context::unknown_caller();
            frame.object = format!("spawn-expression:{}.this", n.id);
            frame.root_object = format!("spawn-expression:{}.unknown-root", n.id);
            self.gap(n, "spawn_planet numeric query evaluation scope is not established by current examples", "spawn-planet-expression-context-unverified", Some(&frame));
            self.numeric_in_state(n, &frame, inputs)
        } else {
            self.numeric_in_state(n, ctx, inputs)
        }
    }

    fn spawn_planet_number(&mut self, n: &Node, ctx: &Context, inputs: &State) {
        if n.children.is_none() {
            self.spawn_numeric_value(n, ctx, inputs);
            return;
        }
        if !numeric_range(&n.key) {
            return;
        }
        let mut bounds = [None, None];
        for (index, name) in ["min", "max"].into_iter().enumerate() {
            if let Some(field) = n.field(name) {
                bounds[index] = self.spawn_numeric_value(field, ctx, inputs);
            }
        }
        match bounds {
            [Some(min), Some(max)] if min > max => self.error(n, "range-order", "spawn_planet orbit range min exceeds max", Some(ctx)),
            [Some(_), Some(_)] => {}
            _ => self.gap(n, "spawn_planet orbit range relation or omitted endpoint defaults are not established", "numeric-range-unproven", Some(ctx)),
        }
    }

    pub(crate) fn spawn_planet_parameters(
        &mut self,
        n: &Node,
        ctx: &Context,
        state: &mut State,
        inputs: &State,
    ) {
        self.spawn_planet_structure(n);
        let depth = self.chain.len();
        self.chain.push(
            serde_json::json!({"kind":"spawn-planet","path":n.source,"line":n.line,"context":ctx})
                .into(),
        );
        // Parameter references are read before any init callback changes a
        // target. This also makes their interpretation independent of ordering.
        for field in n.body() {
            if callback(&field.key) {
                continue;
            }
            if number_field(&field.key) {
                self.spawn_planet_number(field, ctx, inputs);
            } else if bool_field(&field.key) {
                if self.scalar_field(field) {
                    self.boolean(field);
                }
            } else {
                match field.key.as_str() {
                    "class" => self.spawn_class_reference(field, Some(ctx), Some(inputs)),
                    "location" if self.scalar_field(field) => {
                        if field.scalar() != "none" {
                            let receiver = self.resolve(field.scalar(), ctx, inputs, field, false);
                            if receiver.this.unknown
                                || receiver.this.known.is_empty()
                                || receiver.this.known.iter().any(|kind| {
                                    !matches!(kind.as_str(), "megastructure" | "planet")
                                })
                            {
                                self.gap(field, "spawn_planet location input other than the shipped megastructure or planet receiver requires verification", "spawn-planet-location-unverified", Some(&receiver));
                            }
                        }
                    }
                    "name" => {
                        self.scalar_field(field);
                    }
                    "flags" => self.spawn_flags_shape(field),
                    "deposit_blockers" | "modifiers"
                        if field.scalar() == "none" && field.children.is_none() => {}
                    _ => {
                        self.migration_gaps
                            .insert(format!("spawn_planet/{}", field.key));
                        self.gap(
                            field,
                            format!(
                                "spawn_planet parameter subgrammar is not established: {}",
                                field.key
                            ),
                            "spawn-planet-field-unverified",
                            Some(ctx),
                        );
                    }
                }
            }
        }
        for field in n.body().iter().filter(|field| callback(&field.key)) {
            if field.children.is_none() {
                continue;
            }
            let frame = ctx.push(Types::new("planet"), &format!("spawned-planet:{}", n.id));
            self.chain.push(serde_json::json!({"kind":"spawn-planet-callback","callback":field.key,"path":field.source,"line":field.line,"context":frame,"creation_success_proven":false}).into());
            if field.key == "init_effects" {
                self.gap(field, "plural spawn_planet init_effects is retained from the legacy contract but lacks a current shipped callback example", "spawn-planet-callback-form-unverified", Some(&frame));
            }
            let mut selected = state.clone();
            selected.retain_number_anchors();
            selected.forget_number_object(&frame.object);
            self.effects(field.body(), &frame, &mut selected);
            *state = state.join(&selected);
            self.chain.pop();
        }
        self.chain.truncate(depth);
    }
}
