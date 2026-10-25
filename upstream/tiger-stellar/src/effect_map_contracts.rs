//! Bounded effect fields from Stellaris 4.5.2 exports and shipped examples.
//! Only code flags are a complete parent grammar. War target/name fields and
//! Vivarium owners leave their parent effects' remaining checks active.
use crate::{
    ast::Node,
    domain::{Context, State},
    engine::Engine,
};
use std::collections::BTreeSet;

const CODE_FLAGS: &[&str] = &[
    "colonizer",
    "rebel",
    "track_all_situations",
    "custom_name",
    "is_in_breach_of_any",
    "has_advisor",
    "autopick_leader_traits",
    "crisis_fighter",
    "is_under_crisis_attack",
    "initialized",
    "is_affected_by_war_exhaustion",
    "is_nomadic",
    "killed",
];
const NATIVE_FAUNA_CATEGORIES: &[&str] = &[
    "space_amoeba",
    "tiyanki",
    "voidworm",
    "cutholoid",
    "crystalline_entity",
];

fn dynamic(value: &str) -> bool {
    value.contains(['$', '@', ':', '|', '[', ']'])
}

fn unresolved_owner_value(value: &str) -> bool {
    // ':' is deliberately absent: an ordinary event_target:country must still
    // resolve and satisfy the Country contract, including any fixed link typo.
    value.contains(['$', '@', '[', ']'])
}

impl Engine {
    fn code_flag_structure(&mut self, n: &Node) {
        if !self.block(n) {
            return;
        }
        if n.body().is_empty() {
            self.gap(
                n,
                "empty code-flag map behavior is not established by the export",
                "code-flag-empty-map-unverified",
                None,
            );
        }
        let mut seen = BTreeSet::new();
        for field in n.body() {
            if field.key.is_empty() {
                self.error(
                    field,
                    "structure",
                    "code flags expects named boolean assignments",
                    None,
                );
                continue;
            }
            if !seen.insert(field.key.as_str()) {
                self.gap(
                    field,
                    "repeated code-flag assignment composition is not established by the export",
                    "code-flag-composition-unverified",
                    None,
                );
            }
            if field.op.as_str() != "=" {
                self.error(field, "operator", "code-flag assignments require =", None);
            }
            if dynamic(&field.key) {
                self.gap(
                    field,
                    "dynamic engine code-flag identity requires expansion",
                    "dynamic-reference",
                    None,
                );
            } else if !CODE_FLAGS.contains(&field.key.as_str()) {
                self.error(
                    field,
                    "unknown-field",
                    format!("unknown country code flag {}", field.key),
                    None,
                );
            }
            if self.scalar_field(field) {
                if dynamic(field.scalar()) {
                    self.gap(
                        field,
                        "dynamic code-flag boolean value requires expansion",
                        "code-flag-value-unverified",
                        None,
                    );
                } else {
                    self.boolean(field);
                }
            }
        }
    }

    fn vivarium_owner_structure(&mut self, n: &Node) {
        if n.op.as_str() != "=" {
            self.error(n, "operator", "Vivarium owners requires =", None);
        }
        if !self.block(n) {
            return;
        }
        if n.body().is_empty() {
            self.gap(
                n,
                "empty Vivarium owner-map defaults are not established by the export",
                "vivarium-owner-default-unverified",
                None,
            );
        }
        let mut seen = BTreeSet::new();
        for field in n.body() {
            if field.key.is_empty() {
                self.error(
                    field,
                    "structure",
                    "Vivarium owners expects category = country assignments",
                    None,
                );
                continue;
            }
            if !seen.insert(field.key.as_str()) {
                self.gap(field, "repeated Vivarium owner-category composition is not established by current examples", "vivarium-owner-composition-unverified", None);
            }
            if field.op.as_str() != "=" {
                self.error(
                    field,
                    "operator",
                    "Vivarium owner assignments require =",
                    None,
                );
            }
            self.symbol_reference("ship_categories", &field.key, field);
            if !dynamic(&field.key)
                && self
                    .program
                    .entities
                    .get("ship_categories")
                    .is_some_and(|categories| categories.contains(&field.key))
                && !NATIVE_FAUNA_CATEGORIES.contains(&field.key.as_str())
            {
                self.gap(field, "registered ship-category support for Vivarium fauna owners is not established by current examples", "vivarium-owner-category-unverified", None);
            }
            if self.scalar_field(field) && unresolved_owner_value(field.scalar()) {
                self.gap(
                    field,
                    "Vivarium owner value requires parameter or runtime-text expansion",
                    "vivarium-owner-value-unresolved",
                    None,
                );
            }
        }
    }

    fn war_name_structure(&mut self, n: &Node) {
        if n.op.as_str() != "=" {
            self.error(n, "operator", "war name requires =", None);
        }
        // The export permits a scalar war name. It does not say that a literal
        // such as "A Name" must be a localization key.
        if n.children.is_none() {
            self.scalar_field(n);
            return;
        }
        if n.field("key").is_none() {
            self.gap(n, "war name block form without a localization key is not established by the shipped example", "war-name-block-form-unverified", None);
        }
        let mut key_seen = false;
        for field in n.body() {
            if field.op.as_str() != "=" {
                self.error(field, "operator", "war-name parameters require =", None);
            }
            match field.key.as_str() {
                "key" => {
                    if key_seen {
                        self.gap(
                            field,
                            "repeated war-name localization key composition is unverified",
                            "war-name-composition-unverified",
                            None,
                        );
                    }
                    key_seen = true;
                    if self.scalar_field(field) {
                        if dynamic(field.scalar()) {
                            self.gap(
                                field,
                                "dynamic war-name localization identity requires expansion",
                                "dynamic-reference",
                                None,
                            );
                        } else {
                            self.localisation(field);
                        }
                    }
                }
                "variable_string" => {
                    if self.scalar_field(field) && dynamic(field.scalar()) {
                        // The native example uses This.MainAttacker. No source
                        // proves this is the declaring Country evaluation frame.
                        self.gap(field, "war-name runtime text substitution context is not established by the scalar contract", "war-name-substitution-context-unverified", None);
                    }
                }
                _ => self.gap(
                    field,
                    "war-name parameter form is not established by the shipped example",
                    "war-name-parameter-unverified",
                    None,
                ),
            }
        }
    }

    fn war_target_structure(&mut self, n: &Node) {
        if n.op.as_str() != "=" {
            self.error(n, "operator", "war target requires =", None);
        }
        if self.scalar_field(n) && unresolved_owner_value(n.scalar()) {
            self.gap(
                n,
                "war target requires parameter or runtime-text expansion",
                "war-target-value-unresolved",
                None,
            );
        }
    }

    /// Called from the effect-only static parameter pass. `false` for partial
    /// parent contracts deliberately preserves the existing generic traversal.
    pub(crate) fn effect_map_structure(&mut self, n: &Node) -> bool {
        if !matches!(
            n.key.as_str(),
            "set_country_code_flags" | "release_vivarium_fauna_count" | "declare_war"
        ) {
            return false;
        }
        if n.op.as_str() != "=" {
            self.error(n, "operator", format!("{} requires =", n.key), None);
        }
        if n.key == "set_country_code_flags" {
            self.code_flag_structure(n);
            return true;
        }
        if !self.block(n) {
            return false;
        }
        for field in n.body() {
            match (n.key.as_str(), field.key.as_str()) {
                ("release_vivarium_fauna_count", "owners") => self.vivarium_owner_structure(field),
                ("declare_war", "name") => self.war_name_structure(field),
                ("declare_war", "target") => self.war_target_structure(field),
                _ => {}
            }
        }
        false
    }

    /// Code flags need no receiver-dependent child checks; the normal effect
    /// input compatibility check must run before this complete-parent bypass.
    pub(crate) fn effect_map_parameters(&mut self, n: &Node) -> bool {
        n.key == "set_country_code_flags"
    }

    /// Handle only the evidenced child, leaving war goal, location and
    /// count on their existing parent paths. `inputs` is the pre-effect state.
    pub(crate) fn effect_map_parameter_child(
        &mut self,
        parent: &Node,
        field: &Node,
        ctx: &Context,
        inputs: &State,
    ) -> bool {
        match (parent.key.as_str(), field.key.as_str()) {
            ("declare_war", "name") => true,
            ("declare_war", "target") => {
                // effects.log:606-612 explicitly requires a Country target.
                // A parameter called target is not globally Country-valued, so
                // this effect-specific contract cannot be a generic heuristic.
                if field.value.is_some()
                    && field.children.is_none()
                    && !unresolved_owner_value(field.scalar())
                {
                    let target = self.resolve(field.scalar(), ctx, inputs, field, false);
                    self.compatible(
                        &target.this,
                        &["country".into()],
                        field,
                        "declare_war target",
                        ctx,
                    );
                }
                true
            }
            ("release_vivarium_fauna_count", "owners") => {
                for owner in field.body() {
                    if owner.key.is_empty()
                        || owner.value.is_none()
                        || owner.children.is_some()
                        || unresolved_owner_value(owner.scalar())
                    {
                        continue;
                    }
                    let target = self.resolve(owner.scalar(), ctx, inputs, owner, false);
                    self.compatible(
                        &target.this,
                        &["country".into()],
                        owner,
                        "Vivarium fauna owner",
                        ctx,
                    );
                }
                true
            }
            _ => false,
        }
    }
}
