use crate::{ast::Node, domain::Context, engine::Engine};

impl Engine {
    pub(crate) fn fleet_settings(&mut self, n: &Node) {
        if !self.block(n) {
            return;
        }
        for field in n.body() {
            match field.key.as_str() {
                // Native export documents the first two; installed creation
                // and fleet-setting callers establish garrison/spawn_debris.
                "can_upgrade" | "can_change_leader" | "garrison" => self.boolean(field),
                "spawn_debris" | "is_boss" => {
                    self.boolean(field);
                    if n.key == "set_fleet_settings" {
                        self.gap(field,format!("{} setter contract unverified; native creation settings are verified",field.key),"engine-contract-unverified",None);
                    }
                }
                "" => self.error(
                    field,
                    "structure",
                    "fleet settings require named entries",
                    None,
                ),
                _ => self.gap(
                    field,
                    format!("fleet setting contract unverified: {}", field.key),
                    "engine-contract-unverified",
                    None,
                ),
            }
        }
    }
    pub(crate) fn parameter_numeric_range(&mut self, n: &Node, ctx: Option<&Context>) {
        if n.children.is_none() {
            self.numeric(n, ctx);
            return;
        }
        for field in n.body() {
            if matches!(field.key.as_str(), "min" | "max") {
                self.numeric(field, ctx);
            } else {
                self.error(field, "unknown-field", "numeric range expects min/max", ctx);
            }
        }
        let low = n.val("min").and_then(|v| v.parse::<f64>().ok());
        let high = n.val("max").and_then(|v| v.parse::<f64>().ok());
        if low
            .zip(high)
            .is_some_and(|(a, b)| a.is_finite() && b.is_finite() && a > b)
        {
            self.error(n, "range-order", "numeric range min exceeds max", ctx);
        }
    }
}
