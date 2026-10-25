use crate::{
    ast::Node,
    domain::{Context, State, Types},
    engine::Engine,
};

#[derive(Clone)]
pub(crate) struct ProjectInput {
    name: String,
    creation: Types,
    location: Types,
    chain: Vec<crate::report::TraceFrame>,
}

impl Engine {
    pub(crate) fn project_activation_structure(&mut self, n: &Node) {
        if !self.block(n) {
            return;
        }
        if n.field("name").is_none() {
            self.error(
                n,
                "missing-field",
                "enable_special_project requires name",
                None,
            );
        }
        // Engine export: name=<project key>, owner=<target, default ROOT>,
        // location=<target, default THIS>. Prose words extracted by the generic
        // parameter importer ("key", "default", "root") are not field names.
        for field in n.body() {
            match field.key.as_str() {
                "name" => {
                    if self.scalar_field(field) {
                        self.symbol_reference("special_projects", field.scalar(), field);
                    }
                }
                "owner" | "location" => {
                    if self.scalar_field(field) && field.scalar().is_empty() {
                        self.error(field, "value", "project target must not be empty", None);
                    }
                }
                _ => self.error(
                    field,
                    "unknown-field",
                    format!("enable_special_project: unknown parameter {}", field.key),
                    None,
                ),
            }
        }
    }
    pub(crate) fn project_activation(&mut self, n: &Node, ctx: &Context, state: &State) {
        if n.children.is_none() {
            return;
        }
        let owner = n.field("owner");
        if owner.is_none_or(|field| {
            field.children.is_none() && field.value.is_some() && !field.scalar().is_empty()
        }) {
            let node = owner.unwrap_or(n);
            let value = owner.map_or("root", Node::scalar);
            let frame = self.resolve(value, ctx, state, node, false);
            self.compatible(
                &frame.this,
                &["country".into()],
                node,
                if owner.is_some() {
                    "enable_special_project owner"
                } else {
                    "enable_special_project default owner"
                },
                ctx,
            );
        }
        let mut location_types = ctx.this.clone();
        if let Some(location) = n.field("location")
            && location.children.is_none()
            && location.value.is_some()
            && !location.scalar().is_empty()
        {
            // All input scopes are exported. Do not invent a narrower location
            // type whitelist; still validate explicitly written scope links.
            location_types = self
                .resolve(location.scalar(), ctx, state, location, false)
                .this;
        }
        if let Some(name) = n.field("name")
            && name.children.is_none()
            && self
                .program
                .definitions
                .get("special_projects")
                .is_some_and(|defs| defs.contains_key(name.scalar()))
            && n.field("location")
                .is_none_or(|f| f.children.is_none() && !f.scalar().is_empty())
        {
            self.schedule_project(name.scalar(), ctx, location_types, n);
        }
        // Starting a project does not execute its future callbacks or establish
        // their target writes in this caller. Their engine input remains separate.
    }
    pub(crate) fn project_field(&mut self, definition: &Node, field: &Node, ctx: &Context) -> bool {
        self.project_field_with_input(definition, field, ctx, None)
    }
    fn project_field_with_input(
        &mut self,
        definition: &Node,
        field: &Node,
        ctx: &Context,
        input: Option<&ProjectInput>,
    ) -> bool {
        match field.key.as_str() {
            "on_success" | "on_start" | "on_progress_25" | "on_progress_50" | "on_progress_75"
            | "on_fail" | "on_cancel" | "abort_trigger" | "fail_trigger" => {
                if !self.block(field) {
                    return true;
                }
                // Current game's common/special_projects/documentation.txt:
                // success/start/progress use the research receiver, while all
                // failure/cancellation conditions and effects use the country.
                // ROOT is unspecified; it must not inherit the declared THIS.
                let scope = definition
                    .val("event_scope")
                    .and_then(crate::loader::event_scope)
                    .unwrap_or("country");
                let success = matches!(
                    field.key.as_str(),
                    "on_success"
                        | "on_start"
                        | "on_progress_25"
                        | "on_progress_50"
                        | "on_progress_75"
                );
                let mut frame = Context::new(if success { scope } else { "country" });
                frame.root = Types::new("?");
                if success {
                    frame.froms =
                        vec![input.map_or_else(|| Types::new("?"), |i| i.location.clone())];
                    frame.prevs = vec![Types::new("country")];
                } else {
                    // Project receiver may no longer exist. This establishes
                    // its type only, not a runtime presence guarantee.
                    frame.froms = vec![
                        Types::new(scope),
                        input.map_or_else(|| Types::new("?"), |i| i.location.clone()),
                    ];
                }
                if matches!(field.key.as_str(), "abort_trigger" | "fail_trigger") {
                    self.triggers(field.body(), &frame, &State::default());
                } else {
                    self.effects(field.body(), &frame, &mut State::default());
                }
            }
            "AI_wait_days" => {
                let mut frame = Context::new("country");
                frame.root = Types::new("?");
                // FROM is the creation scope, not the research receiver.
                // Until activation inputs are propagated it remains unknown.
                frame.froms = vec![input.map_or_else(|| Types::new("?"), |i| i.creation.clone())];
                self.weighted(field, &frame, &State::default());
            }
            "desc" => {
                // Vanilla 06_projects_nomads documents THIS as the vessel.
                // It does not establish a ROOT or FROM contract here.
                let scope = definition
                    .val("event_scope")
                    .and_then(crate::loader::event_scope)
                    .unwrap_or("country");
                let mut frame = Context::new(scope);
                frame.root = Types::new("?");
                self.description(field, &frame, &State::default());
            }
            "requirements" => self.project_requirements(field, ctx),
            "triggered_requirement" => {
                if !self.block(field) {
                    return true;
                }
                for required in ["text", "trigger"] {
                    if field.field(required).is_none() {
                        self.error(
                            field,
                            "missing-field",
                            format!("triggered_requirement requires {required}"),
                            Some(ctx),
                        );
                    }
                }
                for item in field.body() {
                    match item.key.as_str() {
                        "text" => {
                            if self.scalar_field(item) {
                                self.localisation(item);
                            }
                        }
                        "count" => self.numeric(item, Some(ctx)),
                        "trigger" => {
                            if self.block(item) {
                                // This condition selects the research fleet,
                                // independently of event_scope's receiver.
                                let mut frame = Context::new("fleet");
                                frame.root = Types::new("?");
                                self.triggers(item.body(), &frame, &State::default());
                            }
                        }
                        _ => self.error(
                            item,
                            "unknown-field",
                            format!("unknown triggered_requirement field {}", item.key),
                            Some(ctx),
                        ),
                    }
                }
            }
            _ => return false,
        }
        true
    }
    fn schedule_project(&mut self, name: &str, ctx: &Context, location: Types, node: &Node) {
        // Queue a possible later callback, never execute it in the enabling
        // caller or import that caller's transient local target bindings.
        // Keep each observed type variant; do not replace the independent
        // unknown engine-entry check of target project declarations.
        let key = format!("{name}\n{}\n{}", ctx.this.label(), location.label());
        if self.project_inputs.contains_key(&key) {
            return;
        }
        if self
            .project_inputs
            .values()
            .filter(|i| i.name == name)
            .count()
            >= 64
        {
            self.complete = false;
            self.gap(
                node,
                format!("special project input context budget reached: {name}"),
                "analysis-limit",
                Some(ctx),
            );
            return;
        }
        let mut chain = self.chain.clone();
        chain.push(serde_json::json!({"kind":"project-activation","definition":name,"path":node.source,"line":node.line,"context":ctx}).into());
        self.project_inputs.insert(
            key.clone(),
            ProjectInput {
                name: name.into(),
                creation: ctx.this.clone(),
                location,
                chain,
            },
        );
        self.project_queue.push_back(key);
    }
    pub(crate) fn project_callbacks(&mut self, key: &str) {
        let input = self.project_inputs[key].clone();
        let Some(definition) = self
            .program
            .definitions
            .get("special_projects")
            .and_then(|defs| defs.get(&input.name))
            .cloned()
        else {
            return;
        };
        let saved = std::mem::replace(&mut self.chain, input.chain.clone());
        self.chain.push(serde_json::json!({"kind":"project-callbacks","definition":input.name,"path":definition.source,"line":definition.line,"location_types":input.location,"creation_types":input.creation}).into());
        for field in definition.body() {
            if matches!(
                field.key.as_str(),
                "on_success"
                    | "on_start"
                    | "on_progress_25"
                    | "on_progress_50"
                    | "on_progress_75"
                    | "on_fail"
                    | "on_cancel"
                    | "abort_trigger"
                    | "fail_trigger"
                    | "AI_wait_days"
            ) {
                self.project_field_with_input(
                    &definition,
                    field,
                    &Context::new("country"),
                    Some(&input),
                );
            }
        }
        self.chain = saved;
    }
    fn project_requirements(&mut self, n: &Node, ctx: &Context) {
        if !self.block(n) {
            return;
        }
        for item in n.body() {
            match item.key.as_str() {
                "skill"
                | "carries_colony"
                | "shipclass_military"
                | "fleet_power"
                | "fleet_size"
                | "shipclass_constructor"
                | "shipclass_science_ship"
                | "shipclass_colonizer"
                | "shipclass_transport"
                | "assault_armies"
                | "defense_armies" => self.numeric(item, Some(ctx)),
                "leader" | "has_trait" | "has_technology" => {
                    if self.scalar_field(item) {
                        let category = match item.key.as_str() {
                            "leader" => "leader_classes",
                            "has_trait" => "traits",
                            _ => "technology",
                        };
                        self.symbol_reference(category, item.scalar(), item);
                    }
                }
                "handled_by_event_tooltip" => {
                    if self.scalar_field(item) {
                        self.localisation(item);
                    }
                }
                "is_founder_species"
                | "research_station"
                | "mining_station"
                | "observation_station" => self.boolean(item),
                "size_requirement" => {
                    if !self.block(item) {
                        continue;
                    }
                    for part in item.body() {
                        match part.key.as_str() {
                            "ship_size" => {
                                if self.scalar_field(part) {
                                    self.symbol_reference("ship_sizes", part.scalar(), part);
                                }
                            }
                            "count" => self.numeric(part, Some(ctx)),
                            _ => self.error(
                                part,
                                "unknown-field",
                                format!("unknown size_requirement field {}", part.key),
                                Some(ctx),
                            ),
                        }
                    }
                }
                _ => self.error(
                    item,
                    "unknown-field",
                    format!("unknown project requirement {}", item.key),
                    Some(ctx),
                ),
            }
        }
    }
}
