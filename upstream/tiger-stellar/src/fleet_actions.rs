//! Fleet-action declarations have their own grammar and deferred callbacks.
use crate::{
    ast::Node,
    domain::{Context, State, Types},
    engine::Engine,
};
use std::collections::BTreeSet;

impl Engine {
    fn fleet_script(
        &mut self,
        n: &Node,
        ctx: &Context,
        state: &State,
        ids: &mut BTreeSet<String>,
        effect: bool,
    ) {
        if !self.block(n) {
            return;
        }
        if let Some(id) = n.field("id") {
            if self.scalar_field(id) {
                if id.scalar().is_empty() {
                    self.error(
                        id,
                        "missing-field",
                        "fleet action id must not be empty",
                        Some(ctx),
                    );
                } else if !ids.insert(id.scalar().into()) {
                    self.error(
                        id,
                        "duplicate-field",
                        format!("duplicate fleet action id {}", id.scalar()),
                        Some(ctx),
                    );
                }
            }
        } else {
            self.error(
                n,
                "missing-field",
                "fleet action script requires save/load id",
                Some(ctx),
            );
        }
        let body = n
            .body()
            .iter()
            .filter(|c| c.key != "id")
            .cloned()
            .collect::<Vec<_>>();
        if effect {
            let frame = if matches!(ctx.this.label().as_str(), "fleet" | "?") {
                ctx.clone()
            } else {
                self.gap(
                    n,
                    "fleet action effect callback scope unverified for selected object",
                    "engine-contract-unverified",
                    Some(ctx),
                );
                ctx.push(Types::new("?"), &format!("fleet-callback:{}", n.id))
            };
            // Queued callbacks do not execute while constructing the queue.
            // Their writes cannot establish immediate caller bindings.
            self.effects(&body, &frame, &mut state.clone());
        } else {
            self.triggers(&body, ctx, state);
        }
    }
    pub(crate) fn fleet_actions(
        &mut self,
        n: &Node,
        ctx: &Context,
        state: &State,
        ids: &mut BTreeSet<String>,
    ) {
        if !self.block(n) {
            return;
        }
        let body = self
            .materialize(n.body(), ctx)
            .unwrap_or_else(|| n.body().to_vec());
        for c in &body {
            match c.key.as_str() {
                "move_to" | "orbit_planet" => {
                    if self.scalar_field(c) {
                        let target = self.resolve(c.scalar(), ctx, state, c, false);
                        let accepted = if c.key == "orbit_planet" {
                            vec!["planet".into()]
                        } else {
                            [
                                "planet",
                                "galactic_object",
                                "ship",
                                "fleet",
                                "starbase",
                                "megastructure",
                            ]
                            .into_iter()
                            .map(String::from)
                            .collect()
                        };
                        self.compatible(
                            &target.this,
                            &accepted,
                            c,
                            &format!("fleet action {}", c.key),
                            ctx,
                        );
                    }
                }
                "wait" if c.children.is_none() => self.numeric(c, Some(ctx)),
                "wait" => {
                    if self.block(c) {
                        if c.field("duration").is_none() {
                            self.error(
                                c,
                                "missing-field",
                                "fleet wait requires duration",
                                Some(ctx),
                            );
                        }
                        for part in c.body() {
                            if matches!(part.key.as_str(), "duration" | "random") {
                                self.numeric(part, Some(ctx));
                            } else {
                                self.error(
                                    part,
                                    "unknown-field",
                                    format!("unknown fleet wait field {}", part.key),
                                    Some(ctx),
                                );
                            }
                        }
                    }
                }
                "effect" => self.fleet_script(c, ctx, state, ids, true),
                "repeat" => {
                    if !self.block(c) {
                        continue;
                    }
                    let mut actions = vec![];
                    for part in c.body() {
                        match part.key.as_str() {
                            "max_iterations" => self.numeric(part, Some(ctx)),
                            "while" => self.fleet_script(part, ctx, state, ids, false),
                            _ => actions.push(part.clone()),
                        }
                    }
                    self.fleet_actions(
                        &Node {
                            children: Some(actions.into_boxed_slice()),
                            ..c.clone()
                        },
                        ctx,
                        state,
                        ids,
                    );
                }
                "find_closest_planet"
                | "find_random_planet"
                | "find_random_system"
                | "find_closest_system" => {
                    if !self.block(c) {
                        continue;
                    }
                    let planet = c.key.ends_with("planet");
                    let selected = ctx.push(
                        Types::new(if planet { "planet" } else { "galactic_object" }),
                        &format!("fleet-selected:{}", c.id),
                    );
                    let found = if planet {
                        "found_planet"
                    } else {
                        "found_system"
                    };
                    for part in c.body() {
                        if part.key == "trigger" {
                            self.fleet_script(part, &selected, state, ids, false);
                        } else if part.key == found {
                            self.fleet_actions(part, &selected, state, ids);
                        } else if part.key == "failed" {
                            self.fleet_actions(part, ctx, state, ids);
                        } else {
                            self.error(
                                part,
                                "unknown-field",
                                format!("unknown {} action field {}", c.key, part.key),
                                Some(ctx),
                            );
                        }
                    }
                }
                _ if self.rules.effects.contains_key(&c.key) => self.error(
                    c,
                    "structure",
                    format!(
                        "fleet actions require effect={{id=... <effects>}}, not bare {}",
                        c.key
                    ),
                    Some(ctx),
                ),
                _ => self.gap(
                    c,
                    format!("fleet action grammar not implemented: {}", c.key),
                    "rule-unimplemented",
                    Some(ctx),
                ),
            }
        }
    }
}
