//! Object-local numeric facts. Unknown runtime values are numbers, not missing
//! variables; constants and presence are tracked independently and path-joined.
use crate::{
    ast::Node,
    domain::{Availability, Context, NumberFact, State},
    engine::Engine,
};

pub(crate) fn variable_name(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|c| c.is_alphanumeric() || c == '_')
}
fn operation(name: &str) -> bool {
    matches!(
        name,
        "set_variable"
            | "change_variable"
            | "add_variable"
            | "subtract_variable"
            | "multiply_variable"
            | "divide_variable"
            | "modulo_variable"
            | "round_variable_to_closest"
            | "clear_variable"
            | "round_variable"
            | "floor_variable"
            | "ceiling_variable"
            | "set_variable_to_random_value"
            | "export_trigger_value_to_variable"
            | "export_modifier_to_variable"
            | "export_modifier_duration_to_variable"
            | "export_resource_stockpile_to_variable"
            | "export_resource_income_to_variable"
            | "export_resource_maximum_to_variable"
            | "check_variable"
    )
}
pub(crate) fn preserves_numbers(name: &str) -> bool {
    matches!(
        name,
        "set_country_flag"
            | "remove_country_flag"
            | "set_planet_flag"
            | "remove_planet_flag"
            | "set_ship_flag"
            | "remove_ship_flag"
            | "set_fleet_flag"
            | "remove_fleet_flag"
            | "log"
            | "add_resource"
            | "create_country"
            | "create_species"
            | "create_fleet"
            | "create_random_fleet"
            | "create_ship"
            | "create_leader"
            | "create_army"
            | "spawn_planet"
            | "create_megastructure"
            | "spawn_megastructure"
            | "spawn_system"
            | "create_ambient_object"
            | "start_situation"
    )
}
impl Engine {
    pub(crate) fn variable_structure(&mut self, n: &Node, effect: bool) -> bool {
        if !effect && n.key == "is_variable_set" {
            if self.scalar_field(n) && n.scalar().is_empty() {
                self.error(n, "missing-field", "variable name must not be empty", None);
            }
            return true;
        }
        if !operation(&n.key) || (n.key == "check_variable") == effect {
            return false;
        }
        if matches!(
            n.key.as_str(),
            "clear_variable" | "round_variable" | "floor_variable" | "ceiling_variable"
        ) {
            if self.scalar_field(n) && n.scalar().is_empty() {
                self.error(n, "missing-field", "variable name must not be empty", None);
            }
            return true;
        }
        if !self.block(n) {
            return true;
        }
        let (required, allowed): (&[&str], &[&str]) = match n.key.as_str() {
            "set_variable_to_random_value" => (&["which"], &["which", "min", "max", "rounded"]),
            "export_trigger_value_to_variable" => (
                &["trigger", "variable"],
                &["trigger", "parameters", "variable", "rounded"],
            ),
            "export_modifier_to_variable" | "export_modifier_duration_to_variable" => {
                (&["modifier", "variable"], &["modifier", "variable"])
            }
            "export_resource_stockpile_to_variable"
            | "export_resource_income_to_variable"
            | "export_resource_maximum_to_variable" => {
                (&["resource", "variable"], &["resource", "variable"])
            }
            _ => (&["which", "value"], &["which", "value"]),
        };
        for name in required {
            if n.field(name).is_none() {
                self.error(
                    n,
                    "missing-field",
                    format!("{} requires {name}", n.key),
                    None,
                );
            }
        }
        for field in n.body() {
            if !allowed.contains(&field.key.as_str()) {
                self.error(
                    field,
                    "unknown-field",
                    format!("{}: unknown parameter {}", n.key, field.key),
                    None,
                );
                continue;
            }
            match field.key.as_str() {
                "parameters" => {
                    self.block(field);
                }
                "rounded" => self.boolean(field),
                _ => {
                    if self.scalar_field(field) {
                        if field.op.as_str() != "="
                            && !(n.key == "check_variable"
                                && field.key == "value"
                                && matches!(
                                    field.op.as_str(),
                                    "==" | "!=" | ">" | "<" | ">=" | "<="
                                ))
                        {
                            self.error(
                                field,
                                "operator",
                                "unexpected operator in variable parameters",
                                None,
                            );
                        }
                        if field.scalar().is_empty() {
                            self.error(
                                field,
                                "missing-field",
                                "variable parameter must not be empty",
                                None,
                            );
                        }
                        if matches!(field.key.as_str(), "value" | "min" | "max")
                            && let Ok(value) = field.scalar().parse::<f64>()
                        {
                            if !value.is_finite() {
                                self.error(field, "value", "numeric value must be finite", None);
                            }
                            if matches!(
                                n.key.as_str(),
                                "divide_variable" | "modulo_variable" | "round_variable_to_closest"
                            ) && value == 0.0
                            {
                                self.error(
                                    field,
                                    "value",
                                    "variable divisor or rounding multiple must not be zero",
                                    None,
                                );
                            }
                        }
                    }
                }
            }
        }
        true
    }
    pub(crate) fn variable_read(
        &mut self,
        name: &str,
        ctx: &Context,
        state: &State,
        n: &Node,
    ) -> NumberFact {
        if let Some(rule) = self.rules.triggers.get("check_variable").cloned() {
            self.compatible(&ctx.this, &rule.input, n, "numeric variable storage", ctx);
        }
        let fact = state.number(&State::number_key(&ctx.object, name));
        match fact.availability {
            Availability::Absent => self.error(
                n,
                "variable",
                format!("numeric variable is known to be unset on this path: {name}"),
                Some(ctx),
            ),
            Availability::Maybe => self.gap(
                n,
                format!(
                    "numeric variable assignment is not established on every analyzed path: {name}"
                ),
                "numeric-binding-unproven",
                Some(ctx),
            ),
            Availability::Definite => {}
        }
        fact
    }
    fn numeric_trigger(&mut self, name: &str, ctx: &Context, n: &Node) {
        if let Some(rule) = self
            .rules
            .numeric_expressions
            .get(name)
            .or_else(|| self.rules.triggers.get(name))
            .cloned()
        {
            self.compatible(&ctx.this, &rule.input, n, &format!("numeric {name}"), ctx);
            if !rule.numeric && name != "is_difficulty" {
                self.gap(
                    n,
                    format!("numeric result contract is not documented for trigger {name}"),
                    "numeric-trigger-contract-unverified",
                    Some(ctx),
                );
            }
        } else if self.program.scripts.contains_key(&(false, name.into())) {
            self.gap(
                n,
                format!("scripted numeric trigger return contract requires verification: {name}"),
                "numeric-trigger-contract-unverified",
                Some(ctx),
            );
        } else {
            self.error(
                n,
                "unknown-trigger",
                format!("unknown numeric trigger {name}"),
                Some(ctx),
            );
        }
    }
    fn numeric_modifier(&mut self, name: &str, ctx: &Context, n: &Node) {
        if !self.rules.modifiers.contains(name) {
            self.error(
                n,
                "unknown-modifier",
                format!("unknown modifier {name}"),
                Some(ctx),
            );
            return;
        }
        // The shipped variable guide documents querying the sum of modifiers
        // applying to an object. Exporter scopes are positive witnesses, not
        // proof that every other object type is forbidden for an expression.
        let documented = self
            .rules
            .effects
            .get("export_modifier_to_variable")
            .is_some_and(|r| {
                !ctx.this.unknown
                    && !ctx.this.known.is_empty()
                    && ctx.this.known.iter().all(|s| r.input.contains(s))
            });
        if !documented {
            self.gap(
                n,
                "modifier numeric query receiver contract requires verification",
                "numeric-modifier-context-unverified",
                Some(ctx),
            );
        }
    }
    pub(crate) fn numeric_in_state(
        &mut self,
        n: &Node,
        ctx: &Context,
        state: &State,
    ) -> Option<f64> {
        if n.children.is_some() {
            self.error(n, "structure", "expected a numeric scalar", Some(ctx));
            return None;
        }
        let raw = n.scalar();
        if let Some(value) = self.script_value_reference(n, ctx, state) {
            return value.value();
        }
        if let Ok(value) = raw.parse::<f64>() {
            if value.is_finite() {
                return Some(value);
            }
            self.error(n, "value", "numeric value must be finite", Some(ctx));
            return None;
        }
        if raw.contains(['$', '[', ']', '@']) {
            self.gap(
                n,
                format!("numeric expression not yet resolved by Rust core: {raw}"),
                "numeric-information-unresolved",
                Some(ctx),
            );
            return None;
        }
        let expression = raw.strip_prefix("var:").unwrap_or(raw);
        if let Some(name) = expression.strip_prefix("trigger:") {
            self.numeric_trigger(name, ctx, n);
            return None;
        }
        if let Some(name) = expression.strip_prefix("modifier:") {
            self.numeric_modifier(name, ctx, n);
            return None;
        }
        let (frame, name) = if let Some((selector, name)) = expression.rsplit_once('.') {
            (self.resolve(selector, ctx, state, n, false), name)
        } else {
            (ctx.clone(), expression)
        };
        if let Some(trigger) = name.strip_prefix("trigger:") {
            self.numeric_trigger(trigger, &frame, n);
            return None;
        }
        if let Some(name) = name.strip_prefix("modifier:") {
            self.numeric_modifier(name, &frame, n);
            return None;
        }
        if !variable_name(name) {
            if !raw.starts_with("var:")
                && (raw.starts_with("event_target:") || raw.starts_with("global_event_target:"))
                && !raw.contains('.')
            {
                self.error(
                    n,
                    "value",
                    format!("scope object used as a number; expected scope.variable: {raw}"),
                    Some(ctx),
                );
            } else {
                self.gap(
                    n,
                    format!("numeric expression or variable identity requires verification: {raw}"),
                    "numeric-information-unresolved",
                    Some(ctx),
                );
            }
            return None;
        }
        self.variable_read(name, &frame, state, n).value()
    }
    fn numeric_nonzero(&mut self, n: &Node, ctx: &Context, state: &State) -> bool {
        if let Some(value) = self.script_value_reference(n, ctx, state) {
            return value.nonzero;
        }
        if let Ok(value) = n.scalar().parse::<f64>() {
            return NumberFact::assigned(Some(value)).nonzero;
        }
        let raw = n.scalar().strip_prefix("var:").unwrap_or(n.scalar());
        if raw.contains(['$', '@', '[', ']']) || raw.starts_with("trigger:") {
            return false;
        }
        let (frame, name) = if let Some((selector, name)) = raw.rsplit_once('.') {
            (self.resolve(selector, ctx, state, n, true), name)
        } else {
            (ctx.clone(), raw)
        };
        variable_name(name)
            && state
                .number(&State::number_key(&frame.object, name))
                .nonzero
    }
    pub(crate) fn variable_operation(
        &mut self,
        n: &Node,
        effect: bool,
        ctx: &Context,
        state: &mut State,
    ) -> bool {
        if !operation(&n.key) || (n.key == "check_variable") == effect {
            return false;
        }
        let scalar = matches!(
            n.key.as_str(),
            "clear_variable" | "round_variable" | "floor_variable" | "ceiling_variable"
        );
        let Some(name_node) = (if scalar {
            Some(n)
        } else {
            n.field(if n.key.starts_with("export_") {
                "variable"
            } else {
                "which"
            })
        }) else {
            return true;
        };
        if name_node.children.is_some() || name_node.scalar().is_empty() {
            return true;
        }
        let name = name_node.scalar();
        if !variable_name(name) {
            self.gap(
                name_node,
                format!("variable identity cannot be resolved statically: {name}"),
                "dynamic-variable-identity",
                Some(ctx),
            );
            if effect {
                state.forget_numbers();
            }
            return true;
        }
        let key = State::number_key(&ctx.object, name);
        if n.key == "clear_variable" {
            state.set_number(
                key,
                NumberFact {
                    availability: Availability::Absent,
                    constant: None,
                    nonzero: false,
                },
            );
            return true;
        }
        if n.key.starts_with("export_") {
            if let Some(trigger) = n.val("trigger") {
                // These quantity-export contracts are shown by the current
                // engine documentation (has_resource) and shipped sacrifice
                // effects (num_assigned_jobs); ordinary boolean argument shape
                // does not describe the exporter's numeric return type.
                let documented = matches!(trigger, "has_resource" | "num_assigned_jobs");
                if documented {
                    let argument = if trigger == "has_resource" {
                        "resource"
                    } else {
                        "job"
                    };
                    if n.field("parameters")
                        .is_none_or(|p| p.field(argument).is_none())
                    {
                        self.gap(n, format!("numeric quantity export input parameter is not established: {trigger}/{argument}"), "numeric-trigger-contract-unverified", Some(ctx));
                    }
                }
                if documented && let Some(rule) = self.rules.triggers.get(trigger).cloned() {
                    if ctx.this.known.contains("colony")
                        && !rule.input.iter().any(|s| s == "colony")
                    {
                        self.gap(n, "colony carrier normalization for numeric export input requires verification", "numeric-trigger-contract-unverified", Some(ctx));
                    } else {
                        self.compatible(
                            &ctx.this,
                            &rule.input,
                            n,
                            &format!("numeric {trigger}"),
                            ctx,
                        );
                    }
                } else {
                    self.numeric_trigger(trigger, ctx, n);
                }
                if let Some(parameters) = n.field("parameters") {
                    for field in parameters.body() {
                        let category = match (trigger, field.key.as_str()) {
                            ("has_resource", "resource") => Some("strategic_resources"),
                            ("num_assigned_jobs", "job") => Some("pop_jobs"),
                            _ => None,
                        };
                        if let Some(category) = category {
                            if self.scalar_field(field) {
                                self.symbol_reference(category, field.scalar(), field);
                            }
                        } else {
                            self.gap(field, format!("numeric trigger parameter contract requires verification: {trigger}/{}", field.key), "numeric-trigger-contract-unverified", Some(ctx));
                        }
                    }
                }
            }
            if let Some(resource) = n.field("resource")
                && resource.children.is_none()
            {
                self.symbol_reference("strategic_resources", resource.scalar(), resource);
            }
            if let Some(modifier) = n.field("modifier")
                && modifier.children.is_none()
            {
                if n.key == "export_modifier_duration_to_variable" {
                    self.symbol_reference("static_modifiers", modifier.scalar(), modifier);
                } else if !self.rules.modifiers.contains(modifier.scalar()) {
                    self.gap(
                        modifier,
                        format!(
                            "modifier export contract requires verification: {}",
                            modifier.scalar()
                        ),
                        "numeric-export-contract-unverified",
                        Some(ctx),
                    );
                }
            }
            state.set_number(key, NumberFact::assigned(None));
            return true;
        }
        if n.key == "set_variable_to_random_value" {
            let min = n
                .field("min")
                .and_then(|v| self.numeric_in_state(v, ctx, state));
            let max = n
                .field("max")
                .and_then(|v| self.numeric_in_state(v, ctx, state));
            if let (Some(min), Some(max)) = (min, max) {
                if min > max {
                    self.error(
                        n,
                        "range-order",
                        "variable random range min exceeds max",
                        Some(ctx),
                    );
                }
            } else {
                self.gap(n, "variable random range relation is not proven (runtime values or undocumented defaults)", "numeric-range-unproven", Some(ctx));
            }
            let rounded = n.val("rounded") == Some("yes");
            let constant = if min == max {
                min.and_then(|v| {
                    if rounded {
                        ((v as f32) as f64 == v && v.fract().abs() != 0.5).then(|| v.round())
                    } else {
                        Some(v)
                    }
                })
            } else {
                None
            };
            let nonzero = matches!((min, max), (Some(lo), Some(hi)) if lo <= hi && (lo as f32).is_finite() && (hi as f32).is_finite() &&
                if rounded { lo >= 1.0 || hi <= -1.0 } else { (lo > 0.0 && (lo as f32) > 0.0) || (hi < 0.0 && (hi as f32) < 0.0) });
            state.set_number(
                key,
                NumberFact {
                    nonzero,
                    ..NumberFact::assigned(constant)
                },
            );
            return true;
        }
        let value = if scalar {
            None
        } else {
            n.field("value")
                .and_then(|v| self.numeric_in_state(v, ctx, state))
        };
        let nonzero = n
            .field("value")
            .is_some_and(|v| self.numeric_nonzero(v, ctx, state));
        if n.key == "check_variable" {
            self.variable_read(name, ctx, state, n);
            return true;
        }
        if n.key == "set_variable" {
            state.set_number(
                key,
                NumberFact {
                    nonzero,
                    ..NumberFact::assigned(value)
                },
            );
            return true;
        }
        let previous = self.variable_read(name, ctx, state, n);
        if matches!(
            n.key.as_str(),
            "divide_variable" | "modulo_variable" | "round_variable_to_closest"
        ) && value == Some(0.0)
        {
            self.error(
                n.field("value").unwrap_or(n),
                "value",
                "variable divisor or rounding multiple must not be zero",
                Some(ctx),
            );
            state.set_number(
                key,
                NumberFact {
                    constant: None,
                    nonzero: false,
                    ..previous
                },
            );
            return true;
        }
        if matches!(
            n.key.as_str(),
            "divide_variable" | "modulo_variable" | "round_variable_to_closest"
        ) && !nonzero
        {
            self.gap(
                n.field("value").unwrap_or(n),
                "numeric divisor or rounding multiple is not proven nonzero",
                "numeric-divisor-unproven",
                Some(ctx),
            );
        }
        // Engine storage precision is not established as f64. Fold only values
        // and results exactly representable in both f32 and f64, and leave
        // rounding ties unknown rather than inventing a tie-breaking rule.
        let shared = |v: f64| v.is_finite() && (v as f32) as f64 == v;
        let computed = previous
            .value()
            .filter(|v| shared(*v))
            .and_then(|old| match n.key.as_str() {
                "round_variable" => (old.fract().abs() != 0.5).then(|| old.round()),
                "floor_variable" => Some(old.floor()),
                "ceiling_variable" => Some(old.ceil()),
                "change_variable" | "add_variable" => value.filter(|v| shared(*v)).map(|v| old + v),
                "subtract_variable" => value.filter(|v| shared(*v)).map(|v| old - v),
                "multiply_variable" => value.filter(|v| shared(*v)).map(|v| old * v),
                "divide_variable" => value.filter(|v| shared(*v)).map(|v| old / v),
                "modulo_variable" => value.filter(|v| shared(*v)).map(|v| old % v),
                "round_variable_to_closest" => value
                    .filter(|v| shared(*v) && (old / v).fract().abs() != 0.5)
                    .map(|v| (old / v).round() * v),
                _ => None,
            })
            .filter(|v| shared(*v));
        if computed.is_some_and(|v| !v.is_finite()) {
            self.error(
                n,
                "value",
                "variable arithmetic produces a non-finite value",
                Some(ctx),
            );
        }
        state.set_number(
            key,
            NumberFact {
                availability: previous.availability,
                ..NumberFact::assigned(computed)
            },
        );
        true
    }
}
