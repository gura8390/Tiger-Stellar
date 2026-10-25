use crate::{
    ast::Node,
    domain::{Context, State, Types},
    engine::Engine,
};

impl Engine {
    pub(crate) fn explicit_declaration(&mut self, category: &str, n: &Node) -> bool {
        if category == "component_slot_templates" {
            self.component_slot_template(n);
            return true;
        }
        if category == "name_lists" {
            self.name_list(n);
            return true;
        }
        let allowed: &[&str] = match category {
            "crisis_levels" => &[
                "required_crisis_currency",
                "required_menace",
                "perks",
                "allow",
                "on_unlock",
            ],
            "crisis_paths" => &["crisis_currency", "levels", "objectives"],
            "crisis_objectives" => &["reward", "recurring", "potential"],
            "menace_perks" => &[
                "portrait",
                "modifier",
                "federation_modifier",
                "on_unlock",
                "resources",
            ],
            _ => return false,
        };
        self.structure_visit(n, true);
        if !self.block(n) {
            return true;
        }
        // Native crisis_levels/00_crisis_levels.txt and crisis_objectives
        // document THIS=country. Neither documents a ROOT or FROM contract.
        let mut ctx = Context::new("country");
        ctx.root = Types::new("?");
        self.chain.push(serde_json::json!({"kind":"definition","category":category,"definition":n.key,"path":n.source,"line":n.line}).into());
        let body = self
            .materialize(n.body(), &ctx)
            .unwrap_or_else(|| n.body().to_vec());
        for c in &body {
            self.structure_visit(c, true);
            if c.key.starts_with('@') {
                continue;
            }
            if !allowed.contains(&c.key.as_str()) {
                self.error(
                    c,
                    "unknown-field",
                    format!("{category}: unknown field {}", c.key),
                    Some(&ctx),
                );
                continue;
            }
            match c.key.as_str() {
                "allow" | "potential" | "on_unlock" => {
                    if self.block(c) {
                        if c.key == "on_unlock" {
                            self.effects(c.body(), &ctx, &mut State::default());
                        } else {
                            self.triggers(c.body(), &ctx, &State::default());
                        }
                    }
                }
                "perks" => self.reference_list(c, "menace_perks"),
                "levels" => self.reference_list(c, "crisis_levels"),
                "objectives" => self.reference_list(c, "crisis_objectives"),
                "crisis_currency" => {
                    if self.scalar_field(c) {
                        self.reference("strategic_resources", c.scalar(), c);
                    }
                }
                "required_crisis_currency" | "required_menace" => self.numeric(c, Some(&ctx)),
                "recurring" => self.boolean(c),
                "reward" if c.children.is_none() => self.numeric(c, Some(&ctx)),
                "reward" => {
                    for item in c.body() {
                        match item.key.as_str() {
                            "round" => self.boolean(item),
                            "round_to" => self.numeric(item, Some(&ctx)),
                            _ => self.weighted(
                                &Node {
                                    children: Some(vec![item.clone()].into_boxed_slice()),
                                    ..c.clone()
                                },
                                &ctx,
                                &State::default(),
                            ),
                        }
                    }
                }
                "modifier" => self.modifier(c, &ctx),
                "federation_modifier" => {
                    self.modifier(c, &ctx.push(Types::new("federation"), "federation"))
                }
                "portrait" => {
                    if self.scalar_field(c) {
                        self.reference("gfx_spritetype", c.scalar(), c);
                    }
                }
                "resources" => {
                    self.resources(c, &ctx);
                    if let Some(cost) = c.field("cost") {
                        self.gap(
                            cost,
                            "menace perk resource cost application is not documented",
                            "engine-contract-unverified",
                            Some(&ctx),
                        );
                    }
                }
                _ => unreachable!(),
            }
        }
        self.chain.pop();
        true
    }

    pub(crate) fn scalar_field(&mut self, n: &Node) -> bool {
        if n.children.is_some() || n.value.is_none() {
            self.error(n, "structure", format!("{} expects a scalar", n.key), None);
            false
        } else {
            true
        }
    }

    fn name_strings(&mut self, n: &Node) {
        if !self.block(n) {
            return;
        }
        for c in n.body() {
            if !c.key.is_empty() || c.children.is_some() || c.value.is_none() {
                self.error(
                    c,
                    "name-list-structure",
                    "name list expects scalar name/localization entries",
                    None,
                );
            } else {
                self.localisation(c);
            }
        }
    }
    fn name_pool(&mut self, n: &Node) {
        if !self.block(n) {
            return;
        }
        if n.body().iter().all(|c| c.key.is_empty()) {
            self.name_strings(n);
            return;
        }
        for c in n.body() {
            match c.key.as_str() {
                "names" | "random_names" => self.name_strings(c),
                "sequential_name" | "sequential_name_male" | "sequential_name_female" => {
                    if self.scalar_field(c) {
                        self.localisation(c);
                    }
                }
                "weight" => self.numeric(c, None),
                "generic" => self.name_pool(c),
                k if self
                    .program
                    .entities
                    .get("planet_classes")
                    .is_some_and(|s| s.contains(k)) =>
                {
                    self.name_pool(c)
                }
                _ => self.gap(
                    c,
                    format!("name list pool alternative: {}", c.key),
                    "rule-unimplemented",
                    None,
                ),
            }
        }
    }
    fn name_list(&mut self, n: &Node) {
        self.structure_visit(n, true);
        if !self.block(n) {
            return;
        }
        for c in n.body() {
            self.structure_visit(c, true);
            match c.key.as_str() {
                "selectable" | "trigger" => {
                    if self.block(c) {
                        self.triggers(
                            c.body(),
                            &Context::new(if c.key == "selectable" {
                                "none"
                            } else {
                                "country"
                            }),
                            &State::default(),
                        );
                    }
                }
                "design_names" => self.name_strings(c),
                "by_rarity_names" => {
                    if self.block(c) {
                        for rarity in c.body() {
                            if matches!(
                                rarity.key.as_str(),
                                "common" | "rare" | "epic" | "exceptional"
                            ) {
                                self.name_strings(rarity);
                            } else {
                                self.error(
                                    rarity,
                                    "unknown-field",
                                    "unknown ship name rarity",
                                    None,
                                );
                            }
                        }
                    }
                }
                "randomized" | "should_name_home_system_planets" => self.boolean(c),
                "category" | "alias" | "customize_random_override" => {
                    self.scalar_field(c);
                }
                "fleet_names" => self.name_pool(c),
                "character_names" => {
                    if self.block(c) {
                        for variant in c.body() {
                            if self.block(variant) {
                                for names in variant.body() {
                                    if matches!(
                                        names.key.as_str(),
                                        "full_names_male"
                                            | "full_names_female"
                                            | "first_names_male"
                                            | "first_names_female"
                                            | "second_names"
                                            | "regnal_first_names_male"
                                            | "regnal_first_names_female"
                                            | "regnal_full_names_male"
                                            | "regnal_full_names_female"
                                            | "regnal_second_names"
                                    ) {
                                        self.name_strings(names);
                                    } else {
                                        self.gap(
                                            names,
                                            format!("character name list field: {}", names.key),
                                            "rule-unimplemented",
                                            None,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                "ship_names" | "ship_class_names" | "army_names" | "planet_names" => {
                    if self.block(c) {
                        for sub in c.body() {
                            self.name_pool(sub);
                        }
                    }
                }
                k if k.starts_with('@') => {}
                _ => self.error(
                    c,
                    "unknown-field",
                    format!("unknown name list field {}", c.key),
                    None,
                ),
            }
        }
    }

    pub(crate) fn on_action_list(&mut self, n: &Node, ctx: &Context) {
        self.on_action_dispatch_list(n, ctx, &State::default());
    }
}
