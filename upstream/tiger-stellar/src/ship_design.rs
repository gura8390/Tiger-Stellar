//! Explicit loadout contracts, not automatic-design or engine simulation.
//! Native common/global_ship_designs/000_documentation.txt defines stages;
//! component-slot and hull declarations provide the cross-file contracts.
use crate::{
    ast::Node,
    domain::{Context, State},
    engine::Engine,
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[derive(Debug, Serialize)]
pub struct Feasibility {
    pub design: String,
    pub hull: String,
    pub net_component_power: Option<f64>,
    pub status: String,
    pub missing_conditions: BTreeSet<String>,
    pub automatic_design_validated: bool,
}
#[derive(Clone)]
struct Slot {
    size: String,
    kind: String,
}
fn conjunction(values: impl IntoIterator<Item = Option<bool>>) -> Option<bool> {
    let mut unknown = false;
    for v in values {
        match v {
            Some(false) => return Some(false),
            None => unknown = true,
            _ => {}
        }
    }
    if unknown { None } else { Some(true) }
}
pub(crate) fn eligibility(nodes: &[Node], hull: &str) -> Option<bool> {
    conjunction(nodes.iter().map(|n| {
        let kind = n.key.to_ascii_lowercase();
        match kind.as_str() {
            "is_ship_size" if n.children.is_none() && !n.scalar().contains(['$', '@', ':']) => {
                Some(n.scalar() == hull)
            }
            "always" if n.children.is_none() => match n.scalar() {
                "yes" => Some(true),
                "no" => Some(false),
                _ => None,
            },
            "and" | "not" | "nand" | "or" | "nor" if n.children.is_some() => {
                let value = if matches!(kind.as_str(), "or" | "nor") {
                    conjunction(
                        n.body()
                            .iter()
                            .map(|c| eligibility(std::slice::from_ref(c), hull).map(|v| !v)),
                    )
                    .map(|v| !v)
                } else {
                    eligibility(n.body(), hull)
                };
                if matches!(kind.as_str(), "not" | "nor" | "nand") {
                    value.map(|v| !v)
                } else {
                    value
                }
            }
            _ => None,
        }
    }))
}
pub(crate) fn matches_contract(n: &Node, key: &str, value: &str) -> Option<bool> {
    let field = n.field(key)?;
    if field.children.is_some() {
        if field.body().iter().any(|c| {
            !c.key.is_empty() || c.children.is_some() || c.scalar().contains(['$', '@', ':'])
        }) {
            None
        } else {
            Some(field.body().iter().any(|c| c.scalar() == value))
        }
    } else if field.scalar().contains(['$', '@', ':']) {
        None
    } else {
        Some(field.scalar() == value)
    }
}
impl Engine {
    fn design_definition(&mut self, category: &str, key: &str, n: &Node) -> Option<Arc<Node>> {
        if key.is_empty() {
            self.error(
                n,
                "missing-field",
                format!("missing {category} reference"),
                None,
            );
            return None;
        }
        self.reference(category, key, n);
        self.program
            .definitions
            .get(category)
            .and_then(|m| m.get(key))
            .cloned()
    }
    pub(crate) fn design_contract(&mut self, n: &Node) {
        self.structure_visit(n, true);
        if !self.block(n) {
            return;
        }
        let allowed = [
            "name",
            "role",
            "upgrades_to",
            "newborn_ship_design",
            "creature_designer_template",
            "country_type",
            "fleet_specialization_tag",
            "use_design_name",
            "is_event_design",
            "allow_buildable_trigger",
            "is_special_buildable",
            "graphical_culture",
            "growth_stages",
            "ship_size",
            "section",
            "required_component",
            "resource_restriction",
            "custom_military_power",
            "ship_owner_type",
            "custom_design",
            "custom_upgrades_to",
            "custom_newborn_design",
            "auto_gen_design",
            "initial_design",
        ];
        for c in n.body() {
            if !allowed.contains(&c.key.as_str()) && !c.key.starts_with('@') {
                self.gap(
                    c,
                    format!("global ship design field: {}", c.key),
                    "rule-unimplemented",
                    None,
                );
            } else if c.key == "allow_buildable_trigger" {
                if self.block(c) {
                    self.triggers(c.body(), &Context::new("country"), &State::default());
                }
            } else if matches!(c.key.as_str(), "upgrades_to" | "newborn_ship_design") {
                if self.scalar_field(c) {
                    self.reference("global_ship_designs", c.scalar(), c);
                }
            } else if matches!(
                c.key.as_str(),
                "creature_designer_template"
                    | "use_design_name"
                    | "is_event_design"
                    | "is_special_buildable"
                    | "custom_design"
                    | "auto_gen_design"
                    | "initial_design"
            ) {
                self.boolean(c);
            }
        }
        if let Some(stages) = n.field("growth_stages") {
            if self.block(stages) {
                if stages.body().is_empty() {
                    self.error(
                        stages,
                        "missing-field",
                        "growth_stages requires at least one stage",
                        None,
                    );
                }
                for stage in stages.body() {
                    if stage.key.is_empty() && self.block(stage) {
                        self.design_stage(n, stage);
                    } else {
                        self.error(
                            stage,
                            "structure",
                            "growth_stages expects anonymous stage blocks",
                            None,
                        );
                    }
                }
            }
        } else {
            self.design_stage(n, n);
        }
    }
    fn design_stage(&mut self, design: &Node, stage: &Node) {
        let error_epoch = self.error_epoch;
        let hull_name = stage.val("ship_size").unwrap_or("");
        let Some(hull) = self.design_definition("ship_sizes", hull_name, stage) else {
            return;
        };
        let mut unknown = BTreeSet::new();
        let mut selected: Vec<Arc<Node>> = vec![];
        let mut seen_sections = BTreeSet::new();
        for section_call in stage.body().iter().filter(|c| c.key == "section") {
            if !self.block(section_call) {
                unknown.insert("missing section".into());
                continue;
            }
            let section_name = section_call.val("template").unwrap_or("");
            let slot = section_call.val("slot").unwrap_or("");
            if slot.is_empty() {
                self.error(section_call, "missing-field", "section requires slot", None);
            }
            if !seen_sections.insert(slot.to_string()) {
                self.error(
                    section_call,
                    "ship-compatibility",
                    format!("duplicate section slot {slot}"),
                    None,
                );
            }
            let Some(section) =
                self.design_definition("section_templates", section_name, section_call)
            else {
                unknown.insert("missing section".into());
                continue;
            };
            for (field, value) in [("ship_size", hull_name), ("fits_on_slot", slot)] {
                match matches_contract(&section,field,value){
                    Some(true)=>{},
                    Some(false)=>self.error(section_call,"ship-compatibility",format!("section {section_name} is incompatible with {hull_name}/{slot}: {field}"),None),
                    None=>{unknown.insert(format!("unresolved section {field}: {section_name}"));}
                }
            }
            if let Some(hull_slots) = hull.field("section_slots")
                && hull_slots.field(slot).is_none()
            {
                self.error(
                    section_call,
                    "ship-compatibility",
                    format!("unknown hull section slot {slot}"),
                    None,
                );
            }
            let mut slots = BTreeMap::<String, Slot>::new();
            for declaration in section.body().iter().filter(|c| c.key == "component_slot") {
                let key = declaration.val("name").unwrap_or("");
                let template = declaration.val("template").unwrap_or("");
                if let Some(contract) =
                    self.design_definition("component_slot_templates", template, declaration)
                {
                    if let (Some(size), Some(kind)) =
                        (contract.val("size"), contract.val("component"))
                    {
                        if slots
                            .insert(
                                key.into(),
                                Slot {
                                    size: size.to_ascii_lowercase(),
                                    kind: kind.to_ascii_lowercase(),
                                },
                            )
                            .is_some()
                        {
                            self.error(
                                declaration,
                                "ship-compatibility",
                                "duplicate component slot declaration",
                                None,
                            );
                        }
                    } else {
                        unknown.insert(format!("unresolved slot contract: {key}"));
                    }
                } else {
                    unknown.insert(format!("unresolved slot contract: {key}"));
                }
            }
            for size in ["small", "medium", "large", "aux"] {
                let count = section.val(&format!("{size}_utility_slots")).unwrap_or("0");
                match count.parse::<usize>() {
                    Ok(count) if count <= 4096 => {
                        for i in 1..=count {
                            slots.insert(
                                format!("{}_UTILITY_{i}", size.to_ascii_uppercase()),
                                Slot {
                                    size: size.into(),
                                    kind: "utility".into(),
                                },
                            );
                        }
                    }
                    _ => {
                        unknown.insert(format!("unresolved or over-budget utility slots: {size}"));
                    }
                }
            }
            let mut occupied = BTreeSet::new();
            for component_call in section_call.body().iter().filter(|c| c.key == "component") {
                if !self.block(component_call) {
                    unknown.insert("missing component".into());
                    continue;
                }
                let key = component_call.val("template").unwrap_or("");
                let slot = component_call.val("slot").unwrap_or("");
                if !occupied.insert(slot.to_string()) {
                    self.error(
                        component_call,
                        "ship-compatibility",
                        format!("duplicate component slot {slot}"),
                        None,
                    );
                }
                let contract = slots.get(slot);
                if contract.is_none() {
                    // A missing slot template means its compatibility is unknown,
                    // rather than proving that every caller slot is invalid.
                    if unknown.iter().any(|s| {
                        s.starts_with("unresolved slot contract")
                            || s.starts_with("unresolved or over-budget utility slots")
                    }) {
                        unknown.insert(format!("unresolved selected slot: {slot}"));
                    } else {
                        self.error(
                            component_call,
                            "ship-compatibility",
                            format!("unknown component slot {slot} in {section_name}"),
                            None,
                        );
                    }
                }
                let Some(component) =
                    self.design_definition("component_templates", key, component_call)
                else {
                    unknown.insert("missing component".into());
                    continue;
                };
                if let Some(contract) = contract {
                    let kind = match component.key.as_str() {
                        "utility_component_template" => "utility",
                        "strike_craft_component_template" => "strike_craft",
                        "weapon_component_template" => "weapon",
                        _ => "?",
                    };
                    if let Some(size) = component.val("size") {
                        if size.contains(['$', '@', ':'])
                            || contract.size.contains(['$', '@', ':'])
                            || contract.kind.contains(['$', '@', ':'])
                            || kind == "?"
                        {
                            unknown.insert(format!("unresolved size/type contract: {key}"));
                        } else if (contract.size != "any"
                            && !contract.size.eq_ignore_ascii_case(size))
                            || (contract.kind != "any" && contract.kind != kind)
                        {
                            self.error(
                                component_call,
                                "ship-compatibility",
                                format!(
                                    "component {key} has incompatible size/type for slot {slot}"
                                ),
                                None,
                            );
                        }
                    } else {
                        unknown.insert(format!("unknown component size: {key}"));
                    }
                }
                selected.push(component);
            }
            if slots.keys().any(|s| !occupied.contains(s)) {
                unknown.insert("unfilled optional component slots".into());
            }
        }
        let required = hull
            .body()
            .iter()
            .filter(|n| n.key == "required_component_set")
            .map(|n| n.scalar().to_string())
            .collect::<BTreeSet<_>>();
        let mut sets = BTreeSet::new();
        for core_call in stage
            .body()
            .iter()
            .filter(|c| c.key == "required_component")
        {
            if !self.scalar_field(core_call) {
                unknown.insert("missing core component".into());
                continue;
            }
            let Some(component) =
                self.design_definition("component_templates", core_call.scalar(), core_call)
            else {
                unknown.insert("missing core component".into());
                continue;
            };
            let set = component.val("component_set").unwrap_or("");
            if !required.contains(set) {
                self.error(
                    core_call,
                    "ship-compatibility",
                    format!(
                        "core component {} is not in a hull required_component_set",
                        core_call.scalar()
                    ),
                    None,
                );
            }
            if !sets.insert(set.to_string()) {
                self.error(
                    core_call,
                    "ship-compatibility",
                    "duplicate required component set in design",
                    None,
                );
            }
            selected.push(component);
        }
        if required.iter().any(|s| !sets.contains(s)) {
            unknown.insert("unfilled required component sets".into());
        }
        if let Some(hull_slots) = hull.field("section_slots")
            && hull_slots
                .body()
                .iter()
                .any(|s| !seen_sections.contains(&s.key))
        {
            unknown.insert("unfilled hull section slots".into());
        }
        let mut net = Some(0.);
        for component in &selected {
            let key = component.val("key").unwrap_or(&component.key);
            match eligibility(
                component.field("potential").map(Node::body).unwrap_or(&[]),
                hull_name,
            ) {
                Some(false) => self.error(
                    stage,
                    "ship-compatibility",
                    format!("component {key} is excluded from hull {hull_name}"),
                    None,
                ),
                None => {
                    unknown.insert(format!("conditional eligibility: {key}"));
                }
                _ => {}
            }
            let value = self
                .program
                .weapon_stats
                .get(key)
                .and_then(|s| s.fields.get("power"))
                .filter(|v| !v.is_empty())
                .map(String::as_str)
                .or_else(|| component.val("power"));
            if let Some(power) = value
                .and_then(|s| s.parse::<f64>().ok())
                .filter(|v| v.is_finite())
            {
                net = net.and_then(|v| {
                    let sum = v + power;
                    if sum.is_finite() { Some(sum) } else { None }
                });
            } else {
                net = None;
                unknown.insert(format!("unknown power: {key}"));
            }
        }
        fn adjusted_power(n: &Node) -> bool {
            n.key.starts_with("triggered_")
                || matches!(n.key.as_str(), "ship_power_add" | "ship_power_mult")
                || n.body().iter().any(adjusted_power)
        }
        if std::iter::once(&hull)
            .chain(selected.iter())
            .any(|n| adjusted_power(n))
        {
            unknown.insert("conditional or modifier-adjusted power".into());
        }
        if net.is_none() {
            unknown.insert("unresolved numeric power sum".into());
        }
        if net.is_some_and(|p| p < 0.) && unknown.is_empty() {
            self.error(
                stage,
                "ship-power",
                format!(
                    "explicit loadout has negative component power balance: {}",
                    net.unwrap()
                ),
                None,
            );
        }
        let design_name = design.val("name").unwrap_or("<unnamed>").to_string();
        if !unknown.is_empty() {
            self.gap(
                stage,
                format!(
                    "explicit ship design feasibility unresolved: {design_name}; {}",
                    unknown.iter().cloned().collect::<Vec<_>>().join("; ")
                ),
                "numeric-information-unresolved",
                None,
            );
        }
        let status = if !unknown.is_empty() {
            "unknown"
        } else if self.error_epoch != error_epoch {
            "infeasible"
        } else {
            "static_explicit_loadout"
        };
        self.ship_feasibility.push(Feasibility {
            design: design_name,
            hull: hull_name.into(),
            net_component_power: net,
            status: status.into(),
            missing_conditions: unknown,
            automatic_design_validated: false,
        });
        self.metrics.explicit_ship_designs += 1;
    }
}
