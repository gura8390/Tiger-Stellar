//! Engine-exported parameter subgrammars, with static shape checks separated
//! from caller-dependent scope checks. Unknown engine contracts stay explicit.
use crate::{
    ast::Node,
    domain::{Context, State},
    engine::Engine,
};

impl Engine {
    pub(crate) fn tooltip_metadata(parent: &str, n: &Node) -> bool {
        matches!(
            parent,
            "custom_tooltip"
                | "custom_description"
                | "custom_tooltip_with_modifiers"
                | "custom_tooltip_fail"
                | "success_text"
        ) && (matches!(n.key.as_str(), "text" | "fail_text")
            || (n.key == "success_text" && n.children.is_none()))
    }
    fn planet_class_literal(&self, value: &str) -> bool {
        !self.is_scope(value)
            || self
                .program
                .entities
                .get("planet_classes")
                .is_some_and(|s| s.contains(value))
    }
    fn fields_present(&mut self, n: &Node, required: &[&str]) {
        for key in required {
            if n.field(key).is_none() {
                self.error(
                    n,
                    "missing-field",
                    format!("{} requires {key}", n.key),
                    None,
                );
            }
        }
    }
    fn civic_list(&mut self, n: &Node, source_allowed: bool) {
        if n.children.is_none() {
            if self.scalar_field(n) && n.scalar() != "random" && !source_allowed {
                self.error(n, "value", "civics expects random or a civic block", None);
            }
            return;
        }
        for c in n.body() {
            if c.key != "civic" {
                self.error(c, "unknown-field", "civics expects civic entries", None);
            } else if self.scalar_field(c) && c.scalar() != "random" {
                self.symbol_reference("governments/civics", c.scalar(), c);
            }
        }
    }
    fn message_variable(&mut self, n: &Node) {
        if !self.block(n) {
            return;
        }
        let literal = n.field("key").is_some() || n.field("value").is_some();
        let keys = if literal {
            &["key", "value"][..]
        } else {
            &["type", "localization", "scope"][..]
        };
        self.fields_present(n, keys);
        for c in n.body() {
            if !keys.contains(&c.key.as_str()) {
                self.error(
                    c,
                    "unknown-field",
                    format!("unknown message variable field {}", c.key),
                    None,
                );
            } else if self.scalar_field(c) && !literal && c.key == "type" && c.scalar() != "name" {
                self.gap(
                    c,
                    format!("message variable type contract unverified: {}", c.scalar()),
                    "engine-contract-unverified",
                    None,
                );
            }
        }
        // Typed localization is a substitution placeholder (e.g. SYSTEM),
        // while literal value can be arbitrary runtime text, including GFX.
    }
    pub(crate) fn parameter_structure(&mut self, n: &Node, effect: bool) -> bool {
        if !effect && self.district_trigger_structure(n) {
            return true;
        }
        if effect {
            if self.effect_map_structure(n) {
                return true;
            }
            if self.agreement_debris_structure(n) {
                return true;
            }
            if n.key == "spawn_planet" {
                self.spawn_planet_structure(n);
                return true;
            }
            if self.timeline_structure(n) {
                return true;
            }
            if n.key == "fire_on_action" {
                self.fire_action_structure(n);
                return true;
            }
            self.creation_callback_structure(n);
            if n.key == "create_random_fleet" {
                self.random_fleet_shape(n);
                return true;
            }
        }
        if self.variable_structure(n, effect) {
            return true;
        }
        match (effect, n.key.as_str()) {
            (true, "enable_special_project") => {
                self.project_activation_structure(n);
                return true;
            }
            (false, "relative_power" | "has_resource") => {
                if !self.block(n) {
                    return true;
                }
                let power = n.key == "relative_power";
                self.fields_present(
                    n,
                    if power {
                        &["who", "value"]
                    } else {
                        &["type", "amount"]
                    },
                );
                for c in n.body() {
                    match (power, c.key.as_str()) {
                        (true, "who") => {
                            self.scalar_field(c);
                        }
                        (true, "value") => {
                            if self.scalar_field(c)
                                && !matches!(
                                    c.scalar(),
                                    "pathetic"
                                        | "inferior"
                                        | "equivalent"
                                        | "superior"
                                        | "overwhelming"
                                )
                            {
                                self.error(c, "value", "invalid relative_power threshold", None);
                            }
                        }
                        (true, "category") => {
                            if self.scalar_field(c)
                                && !matches!(c.scalar(), "fleet" | "economy" | "technology" | "all")
                            {
                                self.error(c, "value", "invalid relative_power category", None);
                            }
                        }
                        (false, "type") => {
                            if self.scalar_field(c) {
                                self.reference("strategic_resources", c.scalar(), c);
                            }
                        }
                        (false, "amount") => self.numeric(c, None),
                        _ => self.error(
                            c,
                            "unknown-field",
                            format!("{}: unknown parameter {}", n.key, c.key),
                            None,
                        ),
                    }
                }
                return true;
            }
            (true, "change_country_flag") => {
                self.change_flag_structure(n);
                return true;
            }
            (true, "set_fleet_settings") => {
                self.fleet_settings(n);
                return true;
            }
            (false, "has_active_event") => {
                if self.block(n) {
                    for c in n.body() {
                        if !c.key.is_empty() || c.children.is_some() || c.value.is_none() {
                            self.error(
                                c,
                                "structure",
                                "has_active_event expects a scalar event ID list",
                                None,
                            );
                        } else if !self.program.events.contains_key(c.scalar()) {
                            self.error(
                                c,
                                "missing-event",
                                format!("unknown event {}", c.scalar()),
                                None,
                            );
                        }
                    }
                }
                return true;
            }
            (false, "calc_true_if") => {
                if self.block(n) {
                    self.fields_present(n, &["amount"]);
                    for c in n.body().iter().filter(|c| c.key == "amount") {
                        self.numeric(c, None);
                    }
                    let body = n
                        .body()
                        .iter()
                        .filter(|c| c.key != "amount")
                        .cloned()
                        .collect::<Vec<_>>();
                    self.structure_script(&body, false);
                }
                return true;
            }
            (true, "change_government") => {
                if n.children.is_none() {
                    if self.scalar_field(n) && n.scalar() != "random" {
                        self.error(n, "value", "change_government scalar expects random", None);
                    }
                    return true;
                }
                for c in n.body() {
                    match c.key.as_str() {
                        "civics" => self.civic_list(c, false),
                        "authority" => {
                            if self.scalar_field(c) && c.scalar() != "random" {
                                self.symbol_reference("governments/authorities", c.scalar(), c);
                            }
                        }
                        "cooldown" | "remove_invalid_civics" => self.boolean(c),
                        _ => self.error(
                            c,
                            "unknown-field",
                            format!("unknown change_government field {}", c.key),
                            None,
                        ),
                    }
                }
                return true;
            }
            (true, "create_message") => {
                if self.block(n) {
                    for c in n.body() {
                        match c.key.as_str() {
                            "variable" => self.message_variable(c),
                            "localization" => {
                                if self.scalar_field(c) {
                                    self.localisation(c);
                                }
                            }
                            "type" | "target" => {
                                self.scalar_field(c);
                            }
                            "days" => self.numeric(c, None),
                            _ => self.gap(
                                c,
                                format!("undocumented create_message parameter: {}", c.key),
                                "rule-unimplemented",
                                None,
                            ),
                        }
                    }
                }
                return true;
            }
            (true, "add_resource") => {
                if self.block(n) {
                    for c in n.body() {
                        match c.key.as_str() {
                            "mult" | "multiplier" => self.numeric(c, None),
                            // These alternate fields were accepted by 0.1.8, but the
                            // native export only verifies resource-name/value pairs.
                            "resource" => {
                                if self.scalar_field(c) {
                                    self.symbol_reference("strategic_resources", c.scalar(), c);
                                }
                                self.gap(c,"add_resource resource/amount form is not verified by native documentation","engine-contract-unverified",None);
                            }
                            "amount" => {
                                self.numeric(c, None);
                                self.gap(c,"add_resource resource/amount form is not verified by native documentation","engine-contract-unverified",None);
                            }
                            "cost" | "income" => {
                                self.scalar_field(c);
                                self.gap(
                                    c,
                                    format!("add_resource {} contract is unverified", c.key),
                                    "engine-contract-unverified",
                                    None,
                                );
                            }
                            "" => self.error(
                                c,
                                "structure",
                                "add_resource expects named resource entries",
                                None,
                            ),
                            _ => {
                                self.symbol_reference("strategic_resources", &c.key, c);
                                self.numeric(c, None);
                            }
                        }
                    }
                }
                return true;
            }
            _ => {}
        }
        for c in n.body() {
            self.parameter_child_structure(n, c);
        }
        false
    }
    fn parameter_child_structure(&mut self, n: &Node, c: &Node) {
        match (n.key.as_str(), c.key.as_str()) {
            ("create_fleet", "settings") => self.fleet_settings(c),
            (
                "create_ambient_object",
                "entity_offset" | "entity_offset_angle" | "entity_offset_height",
            ) => self.parameter_numeric_range(c, None),
            (
                "create_ambient_object",
                "use_3d_location" | "entity_scale_to_size" | "play_animation_once" | "is_wreck",
            ) => self.boolean(c),
            ("create_ambient_object", "scale") => self.numeric(c, None),
            ("create_ambient_object", "entity_face_object") => {
                self.scalar_field(c);
            }
            ("create_country", "flag") => self.flag_structure(c),
            ("create_country", "name") if c.children.is_some() => {
                self.fields_present(c, &["key"]);
                for field in c.body() {
                    match field.key.as_str() {
                        "key" => {
                            if self.scalar_field(field) {
                                self.localisation(field);
                            }
                        }
                        "variable_string" => {
                            self.scalar_field(field);
                        }
                        _ => self.gap(
                            field,
                            format!(
                                "country name substitution grammar unimplemented: {}",
                                field.key
                            ),
                            "rule-unimplemented",
                            None,
                        ),
                    }
                }
            }
            ("create_country", "civics") => self.civic_list(c, true),
            ("create_country", "ethos") => {
                if c.children.is_none() {
                    self.scalar_field(c);
                } else {
                    for field in c.body() {
                        if field.key != "ethic" {
                            self.error(field, "unknown-field", "ethos expects ethic entries", None);
                        } else if self.scalar_field(field) && field.scalar() != "random" {
                            self.symbol_reference("ethics", field.scalar(), field);
                        }
                    }
                }
            }
            ("create_leader", "traits") => {
                if !self.block(c) {
                    return;
                }
                for field in c.body() {
                    if field.key.is_empty() {
                        self.error(
                            field,
                            "structure",
                            "leader traits requires level = trait entries",
                            None,
                        );
                        continue;
                    }
                    // The engine explicitly permits zero AND non-integer keys.
                    if self.scalar_field(field)
                        && !matches!(field.scalar(), "random" | "random_traits")
                    {
                        if field.scalar().starts_with("leader_trait_")
                            || self
                                .program
                                .entities
                                .get("traits")
                                .is_some_and(|s| s.contains(field.scalar()))
                        {
                            self.symbol_reference("traits", field.scalar(), field);
                        } else {
                            self.gap(
                                field,
                                format!(
                                    "leader trait special token contract unverified: {}",
                                    field.scalar()
                                ),
                                "engine-contract-unverified",
                                None,
                            );
                        }
                    }
                }
            }
            ("create_leader", "tier") if self.scalar_field(c) => {
                self.symbol_reference("leader_tiers", c.scalar(), c);
            }
            ("create_species", "traits") => {
                if c.children.is_none() {
                    self.scalar_field(c);
                } else {
                    for field in c.body() {
                        if !matches!(field.key.as_str(), "" | "trait" | "ideal_planet_class") {
                            self.error(
                                field,
                                "unknown-field",
                                format!("unknown species traits field {}", field.key),
                                None,
                            );
                            continue;
                        }
                        if self.scalar_field(field) {
                            if field.key == "ideal_planet_class" {
                                if self.planet_class_literal(field.scalar()) {
                                    self.symbol_reference("planet_classes", field.scalar(), field);
                                }
                            } else if !matches!(field.scalar(), "random" | "random_traits") {
                                self.symbol_reference("traits", field.scalar(), field);
                            }
                        }
                    }
                }
            }
            ("modify_species", "add_trait" | "remove_trait") if self.scalar_field(c) => {
                self.reference("traits", c.scalar(), c);
            }
            ("give_specimen", "targets") => {
                if !self.block(c) {
                    return;
                }
                for field in c.body() {
                    if !field.key.is_empty() || field.children.is_some() || field.value.is_none() {
                        self.error(
                            field,
                            "structure",
                            "specimen targets expects a scalar scope list",
                            None,
                        );
                    }
                }
            }
            ("clone_leader", "skip_background_generation") => self.boolean(c),
            ("add_random_research_option", "fail_effects") if self.block(c) => {
                self.structure_script(c.body(), true);
            }
            _ => {}
        }
    }
    pub(crate) fn explicit_parameters(
        &mut self,
        n: &Node,
        effect: bool,
        ctx: &Context,
        state: &mut State,
    ) -> bool {
        if self.variable_operation(n, effect, ctx, state) {
            return true;
        }
        match (effect, n.key.as_str()) {
            (true, "enable_special_project") => {
                self.project_activation(n, ctx, state);
                true
            }
            (false, "has_active_event")
            | (true, "change_government")
            | (true, "add_resource")
            | (true, "change_country_flag" | "set_fleet_settings") => true,
            (false, "calc_true_if") => {
                let body = n
                    .body()
                    .iter()
                    .filter(|c| c.key != "amount")
                    .cloned()
                    .collect::<Vec<_>>();
                self.trigger_alternatives(&body, ctx, state);
                true
            }
            (true, "create_message") => {
                for c in n.body() {
                    if c.key == "target" && c.value.is_some() {
                        self.resolve(c.scalar(), ctx, state, c, false);
                    }
                    if c.key == "variable"
                        && c.field("key").is_none()
                        && c.field("value").is_none()
                        && let Some(scope) = c.field("scope")
                        && scope.value.is_some()
                    {
                        self.resolve(scope.scalar(), ctx, state, scope, false);
                    }
                }
                true
            }
            _ => false,
        }
    }
    pub(crate) fn explicit_parameter_child(
        &mut self,
        n: &Node,
        c: &Node,
        ctx: &Context,
        state: &mut State,
    ) -> bool {
        match (n.key.as_str(), c.key.as_str()) {
            ("create_fleet", "settings") => true,
            (
                "create_ambient_object",
                "entity_offset"
                | "entity_offset_angle"
                | "entity_offset_height"
                | "use_3d_location"
                | "entity_scale_to_size"
                | "play_animation_once"
                | "is_wreck"
                | "scale",
            ) => true,
            ("create_ambient_object", "entity_face_object") => {
                if c.value.is_some() {
                    self.resolve(c.scalar(), ctx, state, c, false);
                }
                true
            }
            ("create_country", "name") if c.children.is_some() => true,
            ("create_country", "civics" | "ethos" | "flag") => {
                if c.value.is_some() && !matches!(c.scalar(), "random" | "none") {
                    let target = self.resolve(c.scalar(), ctx, state, c, false);
                    self.compatible(
                        &target.this,
                        &["country".into()],
                        c,
                        &format!("{} source", c.key),
                        ctx,
                    );
                }
                true
            }
            ("create_leader", "traits" | "tier")
            | ("modify_species", "add_trait" | "remove_trait")
            | ("clone_leader", "skip_background_generation") => true,
            ("create_species", "traits") => {
                if c.children.is_some() {
                    for field in c
                        .body()
                        .iter()
                        .filter(|f| f.key == "ideal_planet_class" && f.value.is_some())
                    {
                        if self.planet_class_literal(field.scalar()) {
                            continue;
                        }
                        let target = self.resolve(field.scalar(), ctx, state, field, false);
                        self.compatible(
                            &target.this,
                            &["planet".into(), "species".into()],
                            field,
                            "ideal_planet_class source",
                            ctx,
                        );
                    }
                } else if !matches!(c.scalar(), "random" | "random_traits") && c.value.is_some() {
                    if self.is_scope(c.scalar()) {
                        let target = self.resolve(c.scalar(), ctx, state, c, false);
                        if target.this.unknown {
                            self.compatible(
                                &target.this,
                                &["species".into(), "country".into()],
                                c,
                                "traits source",
                                ctx,
                            );
                        } else if !target
                            .this
                            .known
                            .iter()
                            .all(|t| matches!(t.as_str(), "species" | "country"))
                        {
                            self.gap(
                                c,
                                format!(
                                    "create_species traits selector contract unverified for {}",
                                    target.this.label()
                                ),
                                "engine-contract-unverified",
                                Some(ctx),
                            );
                        }
                    } else {
                        self.reference("traits", c.scalar(), c);
                    }
                }
                true
            }
            ("give_specimen", "targets") => {
                for field in c
                    .body()
                    .iter()
                    .filter(|f| f.key.is_empty() && f.value.is_some() && f.children.is_none())
                {
                    self.resolve(field.scalar(), ctx, state, field, false);
                }
                true
            }
            ("add_random_research_option", "fail_effects") => {
                let mut failure = state.clone();
                self.effects(c.body(), ctx, &mut failure);
                *state = state.join(&failure);
                true
            }
            _ => false,
        }
    }
    pub(crate) fn trigger_alternatives(&mut self, nodes: &[Node], ctx: &Context, state: &State) {
        let mut i = 0;
        while i < nodes.len() {
            let start = i;
            i += 1;
            if nodes[start].key.eq_ignore_ascii_case("if") {
                while i < nodes.len()
                    && matches!(
                        nodes[i].key.to_ascii_lowercase().as_str(),
                        "else_if" | "else"
                    )
                {
                    let last = nodes[i].key.eq_ignore_ascii_case("else");
                    i += 1;
                    if last {
                        break;
                    }
                }
            }
            self.triggers(&nodes[start..i], ctx, state);
        }
    }
}
