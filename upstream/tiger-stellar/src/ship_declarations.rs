//! Declaration contracts complement explicit global-ship-design checks.
use crate::{
    ast::Node,
    domain::Context,
    engine::Engine,
    ship_design::{eligibility, matches_contract},
};
use std::collections::BTreeSet;

impl Engine {
    pub(crate) fn component_slot_template(&mut self, n: &Node) {
        self.structure_visit(n, true);
        if !self.block(n) {
            return;
        }
        for field in ["size", "component"] {
            if n.field(field).is_none() {
                self.error(
                    n,
                    "missing-field",
                    format!("component slot requires {field}"),
                    None,
                );
            }
        }
        for c in n.body() {
            match c.key.as_str() {
                "size" | "component" => {
                    if !self.scalar_field(c) {
                        continue;
                    }
                    let allowed = if c.key == "size" {
                        &[
                            "small",
                            "medium",
                            "large",
                            "extra_large",
                            "titanic",
                            "planet_killer",
                            "point_defence",
                            "torpedo",
                            "engulf",
                            "any",
                            "aux",
                        ][..]
                    } else {
                        &["weapon", "utility", "strike_craft", "any"][..]
                    };
                    if !allowed.contains(&c.scalar().to_ascii_lowercase().as_str()) {
                        self.error(
                            c,
                            "value",
                            format!("unknown component slot {} {}", c.key, c.scalar()),
                            None,
                        );
                    }
                }
                "is_fixed" => self.boolean(c),
                "entities" => {
                    if self.block(c) {
                        for entity in c.body() {
                            self.scalar_field(entity);
                        }
                    }
                }
                _ => self.gap(
                    c,
                    format!("component slot template field not yet modeled: {}", c.key),
                    "rule-unimplemented",
                    None,
                ),
            }
        }
    }
    pub(crate) fn section_component_slot(&mut self, n: &Node, ctx: &Context) {
        if !self.block(n) {
            return;
        }
        for field in ["name", "template"] {
            if n.field(field).is_none() {
                self.error(
                    n,
                    "missing-field",
                    format!("component_slot requires {field}"),
                    Some(ctx),
                );
            }
        }
        let mut seen = BTreeSet::new();
        for c in n.body() {
            if !seen.insert(c.key.as_str()) {
                self.error(
                    c,
                    "duplicate-field",
                    format!("duplicate component_slot field {}", c.key),
                    Some(ctx),
                );
            }
            match c.key.as_str() {
                "name" | "locatorname" => {
                    self.scalar_field(c);
                }
                "template" => {
                    if self.scalar_field(c) {
                        self.symbol_reference("component_slot_templates", c.scalar(), c);
                    }
                }
                "is_side_slot" => self.boolean(c),
                "rotation" | "section_instance" => self.numeric(c, Some(ctx)),
                "position" => {
                    if self.block(c) {
                        let mut axes = BTreeSet::new();
                        for axis in c.body() {
                            if !matches!(axis.key.as_str(), "x" | "y" | "z") {
                                self.error(
                                    axis,
                                    "unknown-field",
                                    format!("unknown position axis {}", axis.key),
                                    Some(ctx),
                                );
                            } else {
                                if !axes.insert(axis.key.as_str()) {
                                    self.error(
                                        axis,
                                        "duplicate-field",
                                        "duplicate position axis",
                                        Some(ctx),
                                    );
                                }
                                self.numeric(axis, Some(ctx));
                            }
                        }
                    }
                }
                _ => self.error(
                    c,
                    "unknown-field",
                    format!("unknown component_slot field {}", c.key),
                    Some(ctx),
                ),
            }
        }
    }
    pub(crate) fn ship_declaration_crosschecks(&mut self, category: &str, n: &Node, ctx: &Context) {
        if category == "section_templates" {
            let mut names = BTreeSet::new();
            for slot in n.body().iter().filter(|c| c.key == "component_slot") {
                if let Some(name) = slot.val("name")
                    && !names.insert(name)
                {
                    self.error(
                        slot,
                        "ship-compatibility",
                        format!("duplicate component slot name {name}"),
                        Some(ctx),
                    );
                }
            }
            if let (Some(hull), Some(slot)) = (n.val("ship_size"), n.val("fits_on_slot")) {
                let hull = self
                    .program
                    .definitions
                    .get("ship_sizes")
                    .and_then(|defs| defs.get(hull))
                    .cloned();
                if let Some(hull) = hull {
                    if let Some(slots) = hull.field("section_slots") {
                        if slots.children.is_some() && slots.field(slot).is_none() {
                            self.error(
                                n.field("fits_on_slot").unwrap(),
                                "ship-compatibility",
                                format!("section does not fit hull section slot {slot}"),
                                Some(ctx),
                            );
                        }
                    } else {
                        self.gap(
                            n,
                            "section hull slot contract is not explicitly declared",
                            "engine-contract-unverified",
                            Some(ctx),
                        );
                    }
                }
            }
        }
        if category == "ship_sizes" {
            if let Some(slots) = n.field("section_slots") {
                for slot in slots.body() {
                    let Some(template) = slot.val("template") else {
                        continue;
                    };
                    let section = self
                        .program
                        .definitions
                        .get("section_templates")
                        .and_then(|defs| defs.get(template))
                        .cloned();
                    if let Some(section) = section {
                        for (field, value) in [
                            ("ship_size", n.key.as_str()),
                            ("fits_on_slot", slot.key.as_str()),
                        ] {
                            match matches_contract(&section,field,value) {
                                Some(false)=>self.error(slot,"ship-compatibility",format!("section template {template} does not fit {}/{value} ({field})",n.key),Some(ctx)),
                                None=>self.gap(slot,format!("unresolved default section contract: {template}/{field}"),"engine-contract-unverified",Some(ctx)),
                                _=>{},
                            }
                        }
                    }
                }
            }
            for required in n
                .body()
                .iter()
                .filter(|c| c.key == "required_component_set")
            {
                if required.children.is_some() || required.scalar().contains(['$', '@', ':', '[']) {
                    continue;
                }
                let candidates = self
                    .program
                    .definitions
                    .get("component_templates")
                    .into_iter()
                    .flat_map(|defs| defs.values())
                    .filter(|c| c.val("component_set") == Some(required.scalar()))
                    .cloned()
                    .collect::<Vec<_>>();
                let conditions = candidates
                    .iter()
                    .map(|c| {
                        eligibility(c.field("potential").map(Node::body).unwrap_or(&[]), &n.key)
                    })
                    .collect::<Vec<_>>();
                let proof = self.prove_required_set(n, required.scalar(), &candidates);
                if candidates.is_empty() {
                    self.error(
                        required,
                        "ship-compatibility",
                        format!(
                            "required component set {} has no components",
                            required.scalar()
                        ),
                        Some(ctx),
                    );
                } else if conditions.iter().all(|v| *v == Some(false)) {
                    self.error(
                        required,
                        "ship-compatibility",
                        format!(
                            "no component in required set {} is applicable to hull {}",
                            required.scalar(),
                            n.key
                        ),
                        Some(ctx),
                    );
                } else if proof.reason.is_some_and(|r| r.ends_with("budget")) {
                    self.complete = false;
                    self.gap(
                        required,
                        format!(
                            "required set {} potential proof reached {}",
                            required.scalar(),
                            proof.reason.unwrap()
                        ),
                        "analysis-limit",
                        Some(ctx),
                    );
                } else if proof.status == "impossible" {
                    self.gap(required,format!("required set {} potential rejects {}; fixed/generated design bypass contract is not verified",required.scalar(),n.key),"engine-contract-unverified",Some(ctx));
                } else if proof.status != "covered" {
                    self.gap(
                        required,
                        format!(
                            "required set {} for {} has unresolved component eligibility",
                            required.scalar(),
                            n.key
                        ),
                        "engine-contract-unverified",
                        Some(ctx),
                    );
                }
                self.potential_proofs.push(proof);
            }
        }
        if matches!(category, "ship_sizes" | "component_templates") {
            let mut current = n.val("key").unwrap_or(&n.key).to_string();
            let mut visited = BTreeSet::new();
            let mut chain = Vec::new();
            while let Some(def) = self
                .program
                .definitions
                .get(category)
                .and_then(|defs| defs.get(&current))
            {
                if !visited.insert(current.clone()) {
                    chain.push(current);
                    self.error(
                        n,
                        "upgrade-cycle",
                        format!("upgrade cycle: {}", chain.join(" -> ")),
                        Some(ctx),
                    );
                    break;
                }
                chain.push(current);
                let Some(next) = def.val("upgrades_to") else {
                    break;
                };
                current = next.into();
            }
        }
    }
}
