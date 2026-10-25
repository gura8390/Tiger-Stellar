//! create_random_fleet grammar from current effects.log:320-331. This checks
//! declarative choices, not AI design selection or the resulting runtime fleet.
use crate::{
    ast::Node,
    domain::{Context, State},
    engine::Engine,
};
use std::collections::BTreeSet;

impl Engine {
    fn fleet_number_shape(&mut self, n: &Node, integer: bool) {
        if !self.scalar_field(n) {
            return;
        }
        if let Ok(value) = n.scalar().parse::<f64>() {
            if !value.is_finite() {
                self.error(n, "value", "numeric value must be finite", None);
            } else if integer && value.fract() != 0.0 {
                self.error(
                    n,
                    "value",
                    "fleet size and design counts require integers",
                    None,
                );
            }
        }
    }
    fn fleet_design_reference(&mut self, n: &Node) {
        if self.scalar_field(n) {
            self.symbol_reference("global_ship_designs", n.scalar(), n);
        }
    }
    pub(crate) fn random_fleet_shape(&mut self, n: &Node) {
        if n.op.as_str() != "=" {
            self.error(n, "operator", "create_random_fleet requires =", None);
        }
        if !self.block(n) {
            return;
        }
        if n.field("size").is_none() {
            self.error(
                n,
                "missing-field",
                "create_random_fleet requires size",
                None,
            );
        }
        let mut seen = BTreeSet::new();
        for field in n.body() {
            if !seen.insert(field.key.as_str()) {
                self.error(
                    field,
                    "duplicate-field",
                    format!("duplicate create_random_fleet parameter {}", field.key),
                    None,
                );
            }
            if field.op.as_str() != "=" {
                self.error(
                    field,
                    "operator",
                    "create_random_fleet parameters require =",
                    None,
                );
            }
            match field.key.as_str() {
                "name" => {
                    self.scalar_field(field);
                }
                "size" => self.fleet_number_shape(field, true),
                "can_overflow" => self.boolean(field),
                "effect" => {
                    if self.block(field) {
                        self.structure_script(field.body(), true);
                    }
                }
                "ship_designs" => {
                    if !self.block(field) {
                        continue;
                    }
                    for entry in field.body() {
                        if !entry.key.is_empty() {
                            self.error(
                                entry,
                                "structure",
                                "ship_designs expects bare names or anonymous design blocks",
                                None,
                            );
                        } else if entry.children.is_none() {
                            self.fleet_design_reference(entry);
                        } else {
                            if entry.field("design").is_none() {
                                self.error(
                                    entry,
                                    "missing-field",
                                    "fleet design entry requires design",
                                    None,
                                );
                            }
                            let mut entry_seen = BTreeSet::new();
                            for child in entry.body() {
                                if !entry_seen.insert(child.key.as_str()) {
                                    self.error(
                                        child,
                                        "duplicate-field",
                                        format!("duplicate fleet design field {}", child.key),
                                        None,
                                    );
                                }
                                if child.op.as_str() != "=" {
                                    self.error(
                                        child,
                                        "operator",
                                        "fleet design entry fields require =",
                                        None,
                                    );
                                }
                                match child.key.as_str() {
                                    "design" => self.fleet_design_reference(child),
                                    "weight" => self.fleet_number_shape(child, false),
                                    "min" | "max" => self.fleet_number_shape(child, true),
                                    _ => self.error(
                                        child,
                                        "unknown-field",
                                        format!("unknown fleet design field {}", child.key),
                                        None,
                                    ),
                                }
                            }
                            if let (Some(a), Some(b)) = (
                                entry.val("min").and_then(|s| s.parse::<f64>().ok()),
                                entry.val("max").and_then(|s| s.parse::<f64>().ok()),
                            ) && a.is_finite()
                                && b.is_finite()
                                && a > b
                            {
                                self.error(
                                    entry,
                                    "range-order",
                                    "fleet design min count exceeds max count",
                                    None,
                                );
                            }
                        }
                    }
                }
                _ => self.error(
                    field,
                    "unknown-field",
                    format!("unknown create_random_fleet parameter {}", field.key),
                    None,
                ),
            }
        }
    }
    pub(crate) fn random_fleet_parameters(
        &mut self,
        n: &Node,
        ctx: &Context,
        state: &mut State,
        inputs: &State,
    ) {
        self.random_fleet_shape(n);
        let depth = self.chain.len();
        self.chain.push(serde_json::json!({"kind":"random-fleet","path":n.source,"line":n.line,"automatic_design_validated":false}).into());
        if let Some(size) = n.field("size") {
            let value = self.numeric_in_state(size, ctx, inputs);
            self.fleet_numeric_domain(size, value, ctx);
        }
        if let Some(list) = n.field("ship_designs")
            && list.children.is_some()
        {
            if list.body().is_empty() {
                self.gap(
                    list,
                    "explicit empty fleet design list fallback behavior requires verification",
                    "random-fleet-empty-list-unverified",
                    Some(ctx),
                );
            }
            for entry in list.body().iter().filter(|entry| entry.children.is_some()) {
                let mut bounds = [None, None];
                for child in entry.body() {
                    if matches!(child.key.as_str(), "weight" | "min" | "max") {
                        let value = self.numeric_in_state(child, ctx, inputs);
                        self.fleet_numeric_domain(child, value, ctx);
                        match child.key.as_str() {
                            "min" => bounds[0] = value,
                            "max" => bounds[1] = value,
                            _ => {}
                        }
                    }
                }
                if let (Some(min), Some(max)) = (bounds[0], bounds[1])
                    && min > max
                {
                    self.error(
                        entry,
                        "range-order",
                        "fleet design min count exceeds max count",
                        Some(ctx),
                    );
                } else if entry.field("min").is_some()
                    && entry.field("max").is_some()
                    && (bounds[0].is_none() || bounds[1].is_none())
                {
                    self.gap(
                        entry,
                        "fleet design min/max relation depends on runtime quantities",
                        "numeric-range-unproven",
                        Some(ctx),
                    );
                }
            }
        }
        for field in n.body().iter().filter(|field| field.key == "effect") {
            self.creation_callback(n, field, ctx, state);
        }
        self.chain.truncate(depth);
    }
    fn fleet_numeric_domain(&mut self, n: &Node, value: Option<f64>, ctx: &Context) {
        if matches!(n.key.as_str(), "size" | "min" | "max") {
            match value {
                Some(value) if value.is_finite() && value.fract() != 0.0 => self.error(
                    n,
                    "value",
                    "fleet size and design counts require integers",
                    Some(ctx),
                ),
                None => self.gap(
                    n,
                    "fleet size or design count is a runtime quantity whose integer constraint is not established",
                    "numeric-integer-unproven",
                    Some(ctx),
                ),
                _ => {}
            }
        }
        if value.is_some_and(|v| v < 0.0) {
            self.gap(
                n,
                "negative fleet size, weight or count sentinel/clamp behavior is not documented",
                "random-fleet-domain-unverified",
                Some(ctx),
            );
        }
    }
}
