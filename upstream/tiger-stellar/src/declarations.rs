use crate::{
    ast::Node,
    domain::{Context, State, Types},
    engine::Engine,
};

impl Engine {
    pub(crate) fn definition(&mut self, category: &str, n: &Node) {
        if n.key.starts_with('@') || n.is_memory_directive() {
            return;
        }
        if category == "global_ship_designs" {
            self.design_contract(n);
            return;
        }
        if category == "script_values" {
            self.script_value_structure(n);
            return;
        }
        if category == "on_actions" {
            self.on_action_structure(n);
            return;
        }
        if self.explicit_declaration(category, n) {
            return;
        }
        if matches!(category, "gfx" | "interface") {
            self.graphics_detail_contracts(n);
            self.graphics_references(n);
            self.gap(
                n,
                "graphics resource paths, shader/material and GUI behavior are not fully migrated",
                "rule-unimplemented",
                None,
            );
            return;
        }
        self.structure_visit(n, true);
        let scope = match category {
            "buildings" | "decisions" => "planet",
            "pop_jobs" => "pop_group",
            "technology"
            | "edicts"
            | "ascension_perks"
            | "traditions"
            | "megastructures"
            | "ship_sizes"
            | "section_templates"
            | "economic_categories"
            | "special_projects"
            | "governments/civics"
            | "country_types"
            | "personalities"
            | "diplomatic_actions" => "country",
            "component_templates" => "ship",
            "static_modifiers" => "?",
            "on_actions" => "?",
            "component_sets" | "component_slot_templates" => "none",
            "solar_system_initializers" => "galactic_object",
            _ => {
                self.migration_gaps
                    .insert(format!("declaration/{category}"));
                self.gap(
                    n,
                    format!("definition grammar not yet migrated to Rust: {category}"),
                    "rule-unimplemented",
                    None,
                );
                return;
            }
        };
        let mut ctx = Context::new(scope);
        if category == "component_templates" {
            ctx.froms = vec![Types::new("country")];
        }
        if category == "megastructures" {
            ctx.froms = vec![Types::new("fleet")];
        }
        let body = self
            .materialize(n.body(), &ctx)
            .unwrap_or_else(|| n.body().to_vec());
        if category == "component_templates" && n.key == "weapon_component_template" {
            let stats = self
                .program
                .weapon_stats
                .get(n.val("key").unwrap_or(""))
                .cloned();
            for field in ["damage", "total_fire_time"] {
                let from_csv = stats.as_ref().is_some_and(|s| {
                    if field == "damage" {
                        s.fields.get("min_damage").is_some_and(|v| !v.is_empty())
                            && s.fields.get("max_damage").is_some_and(|v| !v.is_empty())
                    } else {
                        s.fields.get("cooldown").is_some_and(|v| !v.is_empty())
                    }
                });
                if !body.iter().any(|c| c.key == field) && !from_csv {
                    self.gap(
                        n,
                        format!(
                            "weapon {field} has no inline definition or resolved CSV statistics"
                        ),
                        "numeric-information-unresolved",
                        Some(&ctx),
                    );
                }
            }
            if let Some(stats) = stats {
                let source = Node {
                    source: stats.source.into(),
                    line: stats.line,
                    column: 1,
                    ..Default::default()
                };
                for (field, value) in &stats.fields {
                    if matches!(field.as_str(), "key" | "end") || value.is_empty() {
                        continue;
                    }
                    let node = Node {
                        key: field.clone(),
                        value: Some(value.clone()),
                        ..source.clone()
                    };
                    self.numeric(&node, Some(&ctx));
                    if value.parse::<f64>().is_err() {
                        self.error(
                            &node,
                            "weapon-csv",
                            "weapon CSV statistics require literal numeric cells",
                            Some(&ctx),
                        );
                    }
                }
                let a = stats
                    .fields
                    .get("min_damage")
                    .and_then(|s| s.parse::<f64>().ok());
                let b = stats
                    .fields
                    .get("max_damage")
                    .and_then(|s| s.parse::<f64>().ok());
                if a.is_some_and(|v| v < 0.) || b.is_some_and(|v| v < 0.) {
                    self.error(
                        &source,
                        "value",
                        "CSV damage must be nonnegative",
                        Some(&ctx),
                    );
                }
                if a.zip(b).is_some_and(|(a, b)| a > b) {
                    self.error(
                        &source,
                        "range-order",
                        "CSV min_damage exceeds max_damage",
                        Some(&ctx),
                    );
                }
                if stats
                    .fields
                    .get("cooldown")
                    .and_then(|s| s.parse::<f64>().ok())
                    .is_some_and(|v| v <= 0.)
                {
                    self.error(
                        &source,
                        "value",
                        "effective CSV cooldown must be positive",
                        Some(&ctx),
                    );
                }
            }
        }
        if category == "static_modifiers" {
            self.modifier(n, &ctx);
            return;
        }
        if category == "solar_system_initializers" {
            self.initializer(n, &ctx);
            return;
        }
        self.chain.push(serde_json::json!({"kind":"definition","category":category,"definition":n.val("key").unwrap_or(&n.key),"path":n.source,"line":n.line}).into());
        self.ship_declaration_crosschecks(category, n, &ctx);
        for c in &body {
            if c.key.starts_with('@') {
                continue;
            }
            self.structure_visit(c, true);
            if category == "component_templates" && self.weapon_numeric_contract(c, &ctx) {
                continue;
            }
            if self.ship_field(category, n, c, &ctx) {
                continue;
            }
            if category == "special_projects" && self.project_field(n, c, &ctx) {
                continue;
            }
            if self.simulation_field(category, c, &ctx) {
                continue;
            }
            match c.key.as_str() {
                "prerequisites" | "upgrades" | "upgrade_from" => {
                    let cat = if c.key == "prerequisites" {
                        "technology"
                    } else {
                        category
                    };
                    self.reference_list(c, cat);
                }
                "upgrades_to" => {
                    if self.scalar_field(c) {
                        self.reference(
                            if category == "ship_sizes" {
                                "ship_sizes"
                            } else {
                                "component_templates"
                            },
                            c.scalar(),
                            c,
                        );
                    }
                }
                "resources" => {
                    let resource_ctx = if category == "ship_sizes" {
                        let mut ctx = Context::new("ship|ship_growth_stage");
                        ctx.froms = vec![Types::new("country")];
                        ctx
                    } else {
                        ctx.clone()
                    };
                    self.resources(c, &resource_ctx);
                }
                "required_component_set" if category == "component_sets" => self.boolean(c),
                "required_component_set" | "component_set" => {
                    if self.scalar_field(c) {
                        self.symbol_reference("component_sets", c.scalar(), c);
                    }
                }
                "potential" | "possible" if category == "component_templates" => {
                    let mut t = Context::new("design");
                    t.froms = vec![Types::new("country")];
                    self.triggers(c.body(), &t, &State::default());
                }
                "potential_country" | "possible_country" | "selectable"
                    if category == "ship_sizes" =>
                {
                    self.triggers(c.body(), &Context::new("country"), &State::default())
                }
                "possible_construction" | "potential_construction" if category == "ship_sizes" => {
                    let mut t = Context::new("starbase|planet|ship|megastructure");
                    t.froms = vec![Types::new("country")];
                    self.triggers(c.body(), &t, &State::default());
                }
                "potential" | "allow" | "possible" | "trigger" | "destroy_trigger" | "active"
                | "visible" | "is_valid" | "can_use" | "ai_allow" | "abort_trigger"
                | "fail_trigger" | "valid_for_country" => {
                    let mut t = ctx.clone();
                    if category == "megastructures"
                        && matches!(c.key.as_str(), "possible" | "context_menu_potential")
                    {
                        t = Context::new("galactic_object");
                        t.froms = vec![Types::new("country"), Types::new("fleet")];
                    }
                    self.triggers(c.body(), &t, &State::default());
                }
                "ai_weight" | "ai_weights" | "weight_modifier" | "ai_chance" | "weight"
                | "random_weight" => {
                    let weight_ctx = if category == "component_templates" {
                        Context::new("country")
                    } else {
                        ctx.clone()
                    };
                    self.weighted(c, &weight_ctx, &State::default());
                }
                "country_modifier"
                | "planet_modifier"
                | "system_modifier"
                | "modifier"
                | "pop_modifier"
                | "ship_modifier"
                | "triggered_ship_modifier"
                | "triggered_ship_design_modifier"
                | "triggered_country_modifier"
                | "triggered_planet_modifier"
                | "triggered_pop_modifier"
                | "triggered_modifier" => self.modifier(c, &ctx),
                "effect"
                | "on_queued"
                | "on_unqueued"
                | "on_built"
                | "on_destroy"
                | "on_destroyed"
                | "on_enabled"
                | "on_disabled"
                | "on_build_complete"
                | "on_research_complete"
                | "on_success"
                | "on_fail"
                | "on_cancel"
                | "on_start"
                | "on_progress_25"
                | "on_progress_50"
                | "on_progress_75" => {
                    self.effects(c.body(), &ctx, &mut State::default());
                }
                "modification" if category == "governments/civics" => {
                    if c.children.is_none() {
                        self.boolean(c);
                    } else {
                        for part in c.body() {
                            if matches!(part.key.as_str(), "add" | "remove") {
                                self.triggers(
                                    part.body(),
                                    &Context::new("country"),
                                    &State::default(),
                                );
                            } else if part.key != "moddable_conditions_custom_tooltip" {
                                self.error(
                                    part,
                                    "unknown-field",
                                    "unknown civic modification field",
                                    Some(&ctx),
                                );
                            }
                        }
                    }
                }
                "section_slots" if category == "ship_sizes" => {
                    if !self.block(c) {
                        continue;
                    }
                    for slot in c.body() {
                        if self.block(slot) {
                            for field in slot.body() {
                                if field.key == "template" {
                                    if self.scalar_field(field) {
                                        self.symbol_reference(
                                            "section_templates",
                                            field.scalar(),
                                            field,
                                        );
                                    }
                                } else if field.key == "locator" {
                                    self.scalar_field(field);
                                } else if field.key != "locator" {
                                    self.error(
                                        field,
                                        "unknown-field",
                                        "unknown section slot field",
                                        Some(&ctx),
                                    );
                                }
                            }
                        }
                    }
                }
                "events" | "random_events" if category == "on_actions" => {
                    self.on_action_list(c, &ctx);
                }
                _ => {
                    let schema = self
                        .declarations
                        .categories
                        .get(category)
                        .and_then(|m| m.get(&c.key))
                        .cloned();
                    if let Some(schema) = schema {
                        let form = if c.children.is_some() {
                            "block"
                        } else {
                            "scalar"
                        };
                        if !schema.forms.iter().any(|f| f == form) {
                            self.gap(
                                c,
                                format!(
                                    "declaration form not covered by native schema: {category}/{}",
                                    c.key
                                ),
                                "rule-unimplemented",
                                Some(&ctx),
                            );
                        } else if form == "scalar" {
                            if schema.scalar_types == ["boolean"] {
                                self.boolean(c);
                            } else if schema.scalar_types == ["number"] {
                                self.numeric(c, Some(&ctx));
                            }
                        } else if !c
                            .body()
                            .iter()
                            .all(|n| n.key.is_empty() && n.children.is_none())
                        {
                            self.gap(
                                c,
                                format!(
                                    "declaration subgrammar not yet migrated: {category}/{}",
                                    c.key
                                ),
                                "rule-unimplemented",
                                Some(&ctx),
                            );
                            self.migration_gaps
                                .insert(format!("field/{category}/{}", c.key));
                        }
                    } else {
                        self.gap(
                            c,
                            format!("unmodeled declaration field: {category}/{}", c.key),
                            "rule-unimplemented",
                            Some(&ctx),
                        );
                        self.migration_gaps
                            .insert(format!("field/{category}/{}", c.key));
                    }
                }
            }
        }
        self.chain.pop();
    }
    pub(crate) fn reference_list(&mut self, n: &Node, category: &str) {
        if !self.block(n) {
            return;
        }
        for c in n.body() {
            if matches!(c.key.as_str(), "OR" | "AND" | "or" | "and") {
                self.reference_list(c, category);
            } else if !c.key.is_empty() || c.children.is_some() {
                self.error(
                    c,
                    "structure",
                    "reference list expects scalar entries or AND/OR groups",
                    None,
                );
            } else {
                self.reference(category, c.scalar(), c);
            }
        }
    }
    pub(crate) fn modifier(&mut self, n: &Node, ctx: &Context) {
        if !self.block(n) {
            return;
        }
        for c in n.body() {
            match c.key.as_str() {
                "potential" | "trigger" => self.triggers(c.body(), ctx, &State::default()),
                "modifier" => self.modifier(c, ctx),
                "icon" | "description" | "custom_tooltip" => {}
                "show_only_custom_tooltip" | "hide_from_country_list" | "important" => {
                    self.boolean(c)
                }
                _ => {
                    if !self.rules.modifiers.contains(&c.key) {
                        self.error(
                            c,
                            "unknown-modifier",
                            format!("unknown modifier {}", c.key),
                            Some(ctx),
                        );
                    } else {
                        self.numeric(c, Some(ctx));
                    }
                }
            }
        }
    }
    pub(crate) fn resources(&mut self, n: &Node, ctx: &Context) {
        if !self.block(n) {
            return;
        }
        for c in n.body() {
            match c.key.as_str() {
                "category" => self.reference("economic_categories", c.scalar(), c),
                "cost" | "upkeep" | "produces" | "logistics" => {
                    if self.block(c) {
                        for v in c.body() {
                            match v.key.as_str() {
                                "trigger" => self.triggers(v.body(), ctx, &State::default()),
                                "multiplier" | "mult" => self.numeric(v, Some(ctx)),
                                _ => {
                                    self.reference("strategic_resources", &v.key, v);
                                    self.numeric(v, Some(ctx));
                                }
                            }
                        }
                    }
                }
                _ => self.error(
                    c,
                    "unknown-field",
                    format!("unknown resources field {}", c.key),
                    Some(ctx),
                ),
            }
        }
    }
    fn ship_field(&mut self, category: &str, definition: &Node, n: &Node, ctx: &Context) -> bool {
        if category == "section_templates" {
            match n.key.as_str() {
                "component_slot" => self.section_component_slot(n, ctx),
                "ship_size" => {
                    if self.scalar_field(n) {
                        self.symbol_reference("ship_sizes", n.scalar(), n);
                    }
                }
                "fits_on_slot" => {
                    self.scalar_field(n);
                }
                _ => return false,
            }
            return true;
        }
        if category == "ship_sizes" && n.key == "min_upgrade_cost" {
            if !self.block(n) {
                return true;
            }
            for c in n.body() {
                self.reference("strategic_resources", &c.key, c);
                self.numeric(c, Some(ctx));
            }
            return true;
        }
        if category != "component_templates" {
            return false;
        }
        match n.key.as_str() {
            "damage" | "collateral_damage" | "chain_damage" | "windup" => {
                if n.children.is_none() {
                    if n.key == "windup" {
                        self.error(n, "structure", "windup requires min/max block", Some(ctx));
                    } else {
                        self.numeric(n, Some(ctx));
                        if n.scalar().parse::<f64>().is_ok_and(|v| v < 0.) {
                            self.error(n, "value", "weapon damage must be nonnegative", Some(ctx));
                        }
                    }
                    return true;
                }
                let mut seen = BTreeSet::new();
                for c in n.body() {
                    if !matches!(c.key.as_str(), "min" | "max") {
                        self.error(
                            c,
                            "unknown-field",
                            format!("unknown {} range field {}", n.key, c.key),
                            Some(ctx),
                        );
                    } else {
                        if !seen.insert(&c.key) {
                            self.error(c, "duplicate-field", "duplicate range endpoint", Some(ctx));
                        }
                        self.numeric(c, Some(ctx));
                        if c.scalar().parse::<f64>().is_ok_and(|v| v < 0.) {
                            self.error(c, "value", "range endpoint must be nonnegative", Some(ctx));
                        }
                    }
                }
                for key in ["min", "max"] {
                    if n.field(key).is_none() {
                        self.error(
                            n,
                            "missing-field",
                            format!("{} requires {key}", n.key),
                            Some(ctx),
                        );
                    }
                }
                if n.key != "windup"
                    && let (Some(a), Some(b)) = (
                        n.val("min").and_then(|v| v.parse::<f64>().ok()),
                        n.val("max").and_then(|v| v.parse::<f64>().ok()),
                    )
                    && a > b
                {
                    self.error(
                        n,
                        "range-order",
                        format!("{} min exceeds max", n.key),
                        Some(ctx),
                    );
                }
                true
            }
            "total_fire_time" | "cooldown" => {
                let ordinary = definition.key == "weapon_component_template";
                if (n.key == "cooldown" && ordinary)
                    || (n.key == "total_fire_time"
                        && definition.key == "strike_craft_component_template")
                {
                    self.error(
                        n,
                        "unknown-field",
                        "ordinary weapons use total_fire_time; strike craft use cooldown",
                        Some(ctx),
                    );
                }
                self.numeric(n, Some(ctx));
                if n.scalar().parse::<f64>().is_ok_and(|v| v <= 0.) {
                    self.error(
                        n,
                        "value",
                        "weapon firing interval must be positive",
                        Some(ctx),
                    );
                }
                true
            }
            "friendly_aura" | "hostile_aura" => {
                if !self.block(n) {
                    return true;
                }
                let target = n.val("apply_on").unwrap_or("ships");
                let frame = ctx.push(
                    Types::new(if target == "fleets" { "fleet" } else { "ship" }),
                    &format!("aura:{}", n.id),
                );
                if n.field("name").is_none() {
                    self.error(n, "missing-field", "aura requires name", Some(ctx));
                }
                for c in n.body() {
                    match c.key.as_str() {
                        "name" => {}
                        "radius" => self.numeric(c, Some(ctx)),
                        "system_wide" => self.boolean(c),
                        "apply_on" => {
                            if !matches!(c.scalar(), "ships" | "fleets") {
                                self.error(
                                    c,
                                    "value",
                                    "aura apply_on expects ships/fleets",
                                    Some(ctx),
                                );
                            }
                        }
                        "modifier" => self.modifier(c, &frame),
                        "limit" => self.triggers(c.body(), &frame, &State::default()),
                        "graphics" => self.aura_graphics(c, ctx),
                        "stack_info" => {
                            if c.field("id").is_none() {
                                self.error(c, "missing-field", "stack_info requires id", Some(ctx));
                            }
                            for f in c.body() {
                                if f.key == "priority" {
                                    self.numeric(f, Some(ctx));
                                } else if f.key != "id" {
                                    self.error(
                                        f,
                                        "unknown-field",
                                        "unknown stack_info field",
                                        Some(ctx),
                                    );
                                }
                            }
                        }
                        _ => {
                            self.gap(
                                c,
                                format!("aura subgrammar not yet migrated: {}", c.key),
                                "rule-unimplemented",
                                Some(ctx),
                            );
                            self.migration_gaps.insert("aura_graphics_damage".into());
                        }
                    }
                }
                true
            }
            "target_weights" => {
                if self.block(n) {
                    for c in n.body() {
                        self.reference("ship_sizes", &c.key, c);
                        self.numeric(c, Some(ctx));
                    }
                }
                true
            }
            _ => false,
        }
    }
    fn initializer(&mut self, n: &Node, ctx: &Context) {
        for c in n.body() {
            match c.key.as_str() {
                "planet" | "moon" => self.initializer(
                    c,
                    &ctx.push(Types::new("planet"), &format!("initializer:{}", c.id)),
                ),
                "init_effect" => self.effects(c.body(), ctx, &mut State::default()),
                "name" | "class" | "usage" | "flags" | "planet_flags" | "initializer" => {}
                "home_planet"
                | "has_ring"
                | "primitive_system"
                | "prevent_anomalies"
                | "is_starting_location"
                | "is_colonizable" => self.boolean(c),
                "size" | "orbit_distance" | "orbit_angle" | "count" | "spawn_chance"
                | "max_instances" | "radius" => {
                    if c.children.is_some() {
                        for endpoint in c.body() {
                            self.numeric(endpoint, Some(ctx));
                        }
                    } else {
                        self.numeric(c, Some(ctx));
                    }
                }
                _ if c.key.starts_with('@') => {}
                _ => {
                    self.gap(
                        c,
                        format!("initializer field not yet migrated: {}", c.key),
                        "rule-unimplemented",
                        Some(ctx),
                    );
                    self.migration_gaps.insert("initializer_fields".into());
                }
            }
        }
    }
}
use std::collections::BTreeSet;
