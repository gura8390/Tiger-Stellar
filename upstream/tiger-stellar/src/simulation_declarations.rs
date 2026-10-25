//! Declaration subgrammars backed by the installed game's job definitions and
//! 99_README_MEGASTRUCTURES.txt. Engine entries are independent call contexts.
use crate::{
    ast::Node,
    domain::{Context, State, Types},
    engine::Engine,
};

impl Engine {
    fn job_swap(&mut self, n: &Node, ctx: &Context) {
        if !self.block(n) {
            return;
        }
        for variant in n.body() {
            if !self.block(variant) {
                continue;
            }
            if variant.key == "trigger" {
                self.triggers(variant.body(), ctx, &State::default());
                continue;
            }
            if variant.key.is_empty() {
                self.error(
                    variant,
                    "structure",
                    "job swap variants require names",
                    Some(ctx),
                );
                continue;
            }
            for field in variant.body() {
                match field.key.as_str() {
                    "trigger" | "potential" => {
                        if self.block(field) {
                            self.triggers(
                                field.body(),
                                &Context::new("planet|colony"),
                                &State::default(),
                            );
                        }
                    }
                    "building_icon" => {
                        if self.scalar_field(field)
                            && !self
                                .program
                                .entities
                                .get("buildings")
                                .is_some_and(|v| v.contains(field.scalar()))
                        {
                            if field.scalar().contains(['/', '\\', ':'])
                                || field.scalar().contains("..")
                            {
                                self.error(
                                    field,
                                    "value",
                                    "building_icon expects a building or icon identifier",
                                    Some(ctx),
                                );
                            } else {
                                self.asset_reference(
                                    &format!(
                                        "gfx/interface/icons/buildings/{}.dds",
                                        field.scalar()
                                    ),
                                    field,
                                );
                            }
                        }
                    }
                    // These are engine-generated job/UI identifiers, not all literal localization keys.
                    "icon" | "name" | "description" | "desc" | "condition_string" => {
                        self.scalar_field(field);
                    }
                    "category" => {
                        if self.scalar_field(field) {
                            self.symbol_reference("pop_categories", field.scalar(), field);
                        }
                    }
                    "weight" if field.children.is_some() => {
                        self.weighted(field, ctx, &State::default())
                    }
                    "weight" => self.numeric(field, Some(ctx)),
                    _ => self.error(
                        field,
                        "unknown-field",
                        format!("unknown job swappable_data field {}", field.key),
                        Some(ctx),
                    ),
                }
            }
        }
    }
    pub(crate) fn simulation_field(&mut self, category: &str, n: &Node, ctx: &Context) -> bool {
        if category == "pop_jobs" {
            match n.key.as_str() {
                "swappable_data" => self.job_swap(n, ctx),
                "possible_pre_triggers" => {
                    if self.block(n) {
                        for field in n.body() {
                            if matches!(
                                field.key.as_str(),
                                "has_owner"
                                    | "is_enslaved"
                                    | "is_being_purged"
                                    | "is_being_assimilated"
                                    | "has_planet"
                                    | "is_sapient"
                                    | "is_robotic"
                            ) {
                                self.boolean(field);
                            } else {
                                self.error(
                                    field,
                                    "unknown-field",
                                    format!("unknown job pre-trigger {}", field.key),
                                    Some(ctx),
                                );
                            }
                        }
                    }
                }
                _ => return false,
            }
            return true;
        }
        if category != "megastructures" {
            return false;
        }
        match n.key.as_str() {
            "entity_offset" => {
                if self.block(n) {
                    for axis in n.body() {
                        if matches!(axis.key.as_str(), "x" | "y" | "z") {
                            self.numeric(axis, Some(ctx));
                        } else {
                            self.error(
                                axis,
                                "unknown-field",
                                format!("unknown coordinate {}", axis.key),
                                Some(ctx),
                            );
                        }
                    }
                }
            }
            "potential"
            | "possible"
            | "context_menu_potential"
            | "dismantle_potential"
            | "dismantle_possible"
            | "should_ai_dismantle"
            | "tooltip_system_filter" => {
                let (this, froms): (&str, &[&str]) = match n.key.as_str() {
                    "potential" => ("country", &["fleet|megastructure"]),
                    "possible" => ("galactic_object", &["country", "fleet|megastructure"]),
                    "context_menu_potential" => ("galactic_object", &["country", "fleet"]),
                    "tooltip_system_filter" => ("galactic_object", &["country"]),
                    _ => ("megastructure", &["country"]),
                };
                let mut frame = Context::new(this);
                frame.froms = froms.iter().map(|t| Types::new(t)).collect();
                frame.prev_tail_unknown = true;
                if self.block(n) {
                    self.triggers(n.body(), &frame, &State::default());
                }
            }
            "placement_rules" => {
                if self.block(n) {
                    for field in n.body() {
                        if field.key != "planet_possible" {
                            self.gap(
                                field,
                                format!("placement_rules field contract unverified: {}", field.key),
                                "engine-contract-unverified",
                                Some(ctx),
                            );
                            continue;
                        }
                        let mut base = Context::new("galactic_object");
                        base.froms = vec![Types::new("country"), Types::new("fleet|megastructure")];
                        let frame = base.push(Types::new("planet"), "placement_planet");
                        if self.block(field) {
                            self.triggers(field.body(), &frame, &State::default());
                        }
                    }
                }
            }
            "on_build_start"
            | "on_build_cancel"
            | "on_build_complete"
            | "on_dismantle_start"
            | "on_dismantle_cancel"
            | "on_dismantle_complete" => {
                let mut frame = Context::new("galactic_object");
                frame.froms = match n.key.as_str() {
                    "on_build_start" => vec![Types::new("country"), Types::new("megastructure|?")],
                    "on_build_complete" => vec![
                        Types::new("country"),
                        Types::new("megastructure"),
                        Types::new("fleet"),
                    ],
                    "on_dismantle_start" | "on_dismantle_cancel" => {
                        vec![Types::new("country"), Types::new("megastructure")]
                    }
                    _ => vec![Types::new("country")],
                };
                // PREV is a starbase only when completion creates one; not a universal guarantee.
                frame.prev_tail_unknown = true;
                if self.block(n) {
                    self.effects(n.body(), &frame, &mut State::default());
                }
            }
            "triggered_country_modifier"
            | "triggered_system_modifier"
            | "triggered_megastructure_modifier" => {
                self.modifier(n, &Context::new("megastructure"))
            }
            "tooltip_system_score" => {
                let mut frame = Context::new("galactic_object");
                frame.froms = vec![Types::new("country")];
                self.weighted(n, &frame, &State::default());
            }
            _ => return false,
        }
        true
    }
}
