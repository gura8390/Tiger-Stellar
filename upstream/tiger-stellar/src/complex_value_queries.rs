//! Numeric query contracts evidenced by Stellaris 4.5.2, not by trigger names.
//!
//! Sources: script_documentation/triggers.log entries for the six queries below;
//! common/script_values/00_script_values.txt:559-594, 1398-1448, 1727-1737;
//! 08_script_values_nomads.txt:96-123, 852-861. In particular num_buildings.limit
//! runs in colony scope, while count_owned_pop_amount.limit runs in pop_group.
use crate::{
    ast::Node,
    domain::{Context, State, Types},
    engine::Engine,
};
use std::collections::BTreeSet;

fn dynamic(value: &str) -> bool {
    value.contains(['$', '@', ':', '|', '[', ']'])
}

impl Engine {
    /// Check a complex numeric query in its already-resolved trigger_scope.
    /// `true` means the query contract is known, including a rejected violation;
    /// unknown semantics return false so the caller retains its coverage gap.
    /// Reading a limit never exports refinements or numeric facts to its caller.
    pub(crate) fn complex_value_query(&mut self, n: &Node, ctx: &Context, state: &State) -> bool {
        let Some(query) = n.val("trigger") else {
            return false;
        };
        if !matches!(
            query,
            "check_galaxy_setup_value"
                | "num_buildings"
                | "num_districts"
                | "count_owned_pop_amount"
                | "count_owned_planet"
                | "resource_income_compare"
        ) {
            return false;
        }
        let parameters = n.field("parameters");
        let required = match query {
            "check_galaxy_setup_value" => Some("setting"),
            "resource_income_compare" => Some("resource"),
            _ => None,
        };
        if let Some(required) = required
            && parameters.and_then(|p| p.field(required)).is_none()
        {
            self.error(
                parameters.unwrap_or(n),
                "missing-field",
                format!("{query} numeric query requires parameters.{required}"),
                Some(ctx),
            );
        }
        let mut verified = true;
        // The API description says colony, while the exported iterator output
        // is planet. Preserve both candidates and an explicit unknown boundary;
        // neither this nor a successful ROOT check proves a planet-only output.
        if query == "count_owned_planet" {
            self.gap(
                n,
                "count_owned_planet query output is described as colony but exported as planet; limit retains both candidates",
                "numeric-trigger-contract-unverified",
                Some(ctx),
            );
            verified = false;
        }
        // Native num_districts (08_script_values_nomads:856-858) explicitly
        // supplies type=any. Neither query has evidence establishing a missing
        // type default, so absence remains unknown rather than an invented error.
        if matches!(query, "num_buildings" | "num_districts")
            && parameters.and_then(|p| p.field("type")).is_none()
        {
            self.gap(
                parameters.unwrap_or(n),
                format!("{query} numeric query without type has no verified default"),
                "numeric-trigger-contract-unverified",
                Some(ctx),
            );
            verified = false;
        }
        let Some(parameters) = parameters else {
            return verified;
        };
        if !self.block(parameters) {
            return true;
        }
        if parameters.op.as_str() != "=" {
            self.error(
                parameters,
                "operator",
                "complex numeric query parameters require =",
                Some(ctx),
            );
        }
        let mut seen = BTreeSet::new();
        for field in parameters.body() {
            if !seen.insert(field.key.as_str()) {
                self.error(
                    field,
                    "duplicate-field",
                    format!("duplicate {query} query parameter {}", field.key),
                    Some(ctx),
                );
            }
            // Comparison operands are documented for ordinary triggers. Their
            // effect on a numeric query's returned quantity is not established.
            // Check their syntax and reads, retain unknown result semantics.
            if (field.key == "value"
                && matches!(
                    query,
                    "check_galaxy_setup_value"
                        | "resource_income_compare"
                        | "num_buildings"
                        | "num_districts"
                ))
                || (field.key == "count"
                    && matches!(query, "count_owned_pop_amount" | "count_owned_planet"))
            {
                if !matches!(field.op.as_str(), "=" | "!=" | ">" | ">=" | "<" | "<=") {
                    self.error(
                        field,
                        "operator",
                        "numeric query comparison requires a comparison operator",
                        Some(ctx),
                    );
                }
                if !(field.key == "count" && field.scalar() == "all") {
                    self.numeric_in_state(field, ctx, state);
                }
                self.gap(
                    field,
                    "comparison parameter in a complex numeric query has no verified return-value semantics",
                    "numeric-trigger-contract-unverified",
                    Some(ctx),
                );
                verified = false;
                continue;
            }
            if field.op.as_str() != "=" {
                self.error(
                    field,
                    "operator",
                    "complex numeric query parameter requires =",
                    Some(ctx),
                );
            }
            match (query, field.key.as_str()) {
                ("check_galaxy_setup_value", "setting") => {
                    let values = [
                        "num_empires",
                        "num_advanced_empires",
                        "num_fallen_empires",
                        "num_marauder_empires",
                        "mid_game_year",
                        "end_game_year",
                        "victory_year",
                        "num_guaranteed_colonies",
                        "num_gateways",
                        "num_wormhole_pairs",
                        "num_hyperlanes",
                        "habitable_worlds_scale",
                        "primitive_worlds_scale",
                        "crisis_strength_scale",
                        "tech_costs_scale",
                        "fallen_empire_strength_scale",
                    ];
                    verified &= self.complex_query_enum(field, &values, ctx);
                }
                ("num_buildings", "type") | ("num_districts", "type") => {
                    if self.scalar_field(field) && field.scalar() != "any" {
                        self.symbol_reference(
                            if query == "num_buildings" {
                                "buildings"
                            } else {
                                "districts"
                            },
                            field.scalar(),
                            field,
                        );
                        verified &= !dynamic(field.scalar());
                    }
                }
                ("num_buildings", "disabled" | "in_construction") => {
                    verified &= self.complex_query_enum(field, &["any", "yes", "no"], ctx);
                }
                ("num_buildings", "owner_type") => {
                    verified &= self.complex_query_enum(
                        field,
                        &["normal", "corporate", "subject_holding"],
                        ctx,
                    );
                }
                ("num_buildings", "category") => {
                    if self.scalar_field(field) && field.scalar() != "any" {
                        // The API gives examples (unity/resource), not an
                        // exhaustive registry. Rejecting an unseen name would
                        // confuse absence of evidence with a missing entity.
                        self.gap(
                            field,
                            "building category names have no closed numeric-query registry",
                            "numeric-trigger-contract-unverified",
                            Some(ctx),
                        );
                        verified = false;
                    }
                }
                ("resource_income_compare", "resource") => {
                    if self.scalar_field(field) {
                        self.symbol_reference("strategic_resources", field.scalar(), field);
                        verified &= !dynamic(field.scalar());
                    }
                }
                ("num_buildings" | "count_owned_pop_amount" | "count_owned_planet", "limit") => {
                    if self.block(field) {
                        let output = match query {
                            "num_buildings" => "colony",
                            "count_owned_pop_amount" => "pop_group",
                            _ => "planet|colony",
                        };
                        let frame = ctx.push(
                            Types::new(output),
                            &format!("complex-query-limit:{}", field.id),
                        );
                        let mut input = state.clone();
                        input.forget_number_object(&frame.object);
                        self.triggers(field.body(), &frame, &input);
                    }
                }
                _ => self.error(
                    field,
                    "unknown-field",
                    format!("unknown {query} numeric query parameter {}", field.key),
                    Some(ctx),
                ),
            }
        }
        verified
    }

    fn complex_query_enum(&mut self, n: &Node, accepted: &[&str], ctx: &Context) -> bool {
        if !self.scalar_field(n) {
            return true;
        }
        if dynamic(n.scalar()) {
            self.gap(
                n,
                "dynamic complex numeric query enum requires expansion",
                "numeric-trigger-contract-unverified",
                Some(ctx),
            );
            false
        } else {
            if !accepted.contains(&n.scalar()) {
                self.error(
                    n,
                    "value",
                    format!("{} expects one of {}", n.key, accepted.join(", ")),
                    Some(ctx),
                );
            }
            true
        }
    }
}
