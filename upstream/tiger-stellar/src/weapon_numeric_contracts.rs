//! Additional numeric weapon fields from the formal 0.1.8 `numeric_range`
//! contract. Current game evidence: component_templates/000_documentation.txt
//! lines 137-178; weapon_components.csv header line 6 and MISSILE_1 line 98.
//! Status power sp_independent_components.txt:1687-1693 supplies inline scalars.
//! These sources establish the fields and scalar numbers, not a balance cap or
//! an engine contract for inline ranges. Keep the latter boundary explicit.
use crate::{
    ast::Node,
    domain::{Context, State},
    engine::Engine,
};
use std::collections::BTreeSet;

impl Engine {
    pub(crate) fn weapon_numeric_contract(&mut self, n: &Node, ctx: &Context) -> bool {
        if !matches!(
            n.key.as_str(),
            "military_power_multiplier"
                | "missile_speed"
                | "missile_evasion"
                | "missile_shield"
                | "missile_armor"
                | "missile_health"
                | "missile_retarget_range"
        ) {
            // windup / total_fire_time already have dedicated contracts with
            // weapon-vs-strike-craft rules; do not shadow those handlers.
            return false;
        }
        if n.op.as_str() != "=" {
            self.error(n, "operator", "numeric weapon field requires =", Some(ctx));
        }
        if n.children.is_none() {
            self.weapon_numeric_operand(n, ctx);
            return true;
        }
        self.gap(
            n,
            "numeric weapon range structure is checked; engine support for this inline min/max form is not established by the current game sources",
            "weapon-numeric-range-engine-unverified",
            Some(ctx),
        );
        let mut seen = BTreeSet::new();
        let mut minimum = None;
        let mut maximum = None;
        for endpoint in n.body() {
            if !matches!(endpoint.key.as_str(), "min" | "max") {
                self.error(
                    endpoint,
                    "unknown-field",
                    format!("{} numeric range expects min/max", n.key),
                    Some(ctx),
                );
                continue;
            }
            if !seen.insert(endpoint.key.as_str()) {
                self.error(
                    endpoint,
                    "duplicate-field",
                    "duplicate numeric weapon range endpoint",
                    Some(ctx),
                );
            }
            if endpoint.op.as_str() != "=" {
                self.error(
                    endpoint,
                    "operator",
                    "numeric weapon range endpoint requires =",
                    Some(ctx),
                );
            }
            let value = self.weapon_numeric_operand(endpoint, ctx);
            if endpoint.key == "min" {
                minimum = value;
            } else {
                maximum = value;
            }
        }
        // Neither the legacy structural contract nor current engine evidence
        // establishes mandatory endpoints or their defaults. Check only an
        // explicitly supplied pair, retaining the range boundary above.
        if minimum.zip(maximum).is_some_and(|(min, max)| min > max) {
            self.error(
                n,
                "range-order",
                format!("{} min exceeds max", n.key),
                Some(ctx),
            );
        }
        true
    }

    /// Return only a finite literal. A runtime quantity or an expanded script
    /// value must not become evidence of endpoint order in a static declaration.
    fn weapon_numeric_operand(&mut self, n: &Node, ctx: &Context) -> Option<f64> {
        if !self.scalar_field(n) {
            return None;
        }
        match n.scalar().parse::<f64>() {
            Ok(value) if value.is_finite() => Some(value),
            Ok(_) => {
                self.error(n, "value", "numeric weapon value must be finite", Some(ctx));
                None
            }
            Err(_) => {
                let value = n.scalar();
                if value.is_empty()
                    || (!crate::variables::variable_name(value)
                        && !value.contains(['.', ':', '@', '$', '[', ']']))
                {
                    self.error(
                        n,
                        "value",
                        "expected a numeric weapon value or unresolved numeric expression",
                        Some(ctx),
                    );
                    return None;
                }
                // The declaration's general ship context is not evidence for
                // numeric stat evaluation. The shipped ship/FROM documentation
                // belongs to on_hit, not these values. Preserve fixed reference
                // checks, but never turn this unknown entry into a ship proof.
                let evaluation = Context::unknown_caller();
                if !matches!(value, "yes" | "no") {
                    self.numeric_in_state(n, &evaluation, &State::default());
                }
                self.gap(
                    n,
                    "weapon numeric expression requires runtime/declaration evaluation; no literal bound is established",
                    "weapon-numeric-expression-unverified",
                    Some(&evaluation),
                );
                None
            }
        }
    }
}
