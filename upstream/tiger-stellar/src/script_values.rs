//! Stellaris script-value contracts from common/script_values/00_script_values.txt.
//! Evaluation is pure: conditional math can read refined facts, never export them.
use crate::{
    ast::Node,
    domain::{Context, NumberFact, State},
    engine::Engine,
};
use std::collections::BTreeMap;

fn numeric_op(key: &str) -> bool {
    matches!(
        key,
        "base"
            | "set"
            | "weight"
            | "factor"
            | "add"
            | "subtract"
            | "mult"
            | "multiply"
            | "divide"
            | "modulo"
            | "round_to"
            | "min"
            | "max"
            | "pow"
    )
}
fn unary_op(key: &str) -> bool {
    matches!(
        key,
        "round" | "ceiling" | "floor" | "abs" | "square" | "square_root"
    )
}
fn math_op(key: &str) -> bool {
    numeric_op(key) || unary_op(key)
}

impl Engine {
    pub(crate) fn script_value_structure(&mut self, def: &Node) {
        if def.op.as_str() != "=" {
            self.error(def, "operator", "script-value definition requires =", None);
        }
        if def.macro_parts.is_some() {
            return;
        }
        if self.block(def) {
            self.value_structure_body(def.body(), false);
        }
    }
    fn value_structure_body(&mut self, body: &[Node], modifier: bool) {
        for n in body {
            if n.key.starts_with('@') {
                continue;
            }
            if math_op(&n.key) {
                if modifier && n.key == "base" {
                    self.error(n, "unknown-field", "base is not a modifier operation", None);
                }
                if n.op.as_str() != "=" {
                    self.error(n, "operator", "script-value operation requires =", None);
                }
                if unary_op(&n.key) {
                    self.boolean(n);
                } else if self.scalar_field(n) {
                    let reference = n
                        .scalar()
                        .strip_prefix("value:")
                        .or_else(|| n.scalar().split_once(".value:").map(|(_, name)| name));
                    if let Some(name) = reference.map(|s| s.split('|').next().unwrap_or(""))
                        && !name.contains(['$', '[', ']', '@'])
                        && !self
                            .program
                            .definitions
                            .get("script_values")
                            .is_some_and(|defs| defs.contains_key(name))
                    {
                        self.error(
                            n,
                            "missing-item",
                            format!("unknown script value {name}"),
                            None,
                        );
                    }
                    if let Ok(value) = n.scalar().parse::<f64>() {
                        if !value.is_finite() {
                            self.error(n, "value", "numeric value must be finite", None);
                        }
                        if value == 0.0
                            && matches!(n.key.as_str(), "divide" | "modulo" | "round_to")
                        {
                            self.error(
                                n,
                                "value",
                                "script-value divisor or rounding multiple must not be zero",
                                None,
                            );
                        }
                    }
                }
            } else if !modifier && n.key == "modifier" {
                if n.op.as_str() != "=" {
                    self.error(n, "operator", "script-value modifier requires =", None);
                }
                if self.block(n) {
                    self.value_structure_body(n.body(), true);
                }
            } else if !modifier && n.key == "complex_trigger_modifier" {
                if n.op.as_str() != "=" {
                    self.error(n, "operator", "complex_trigger_modifier requires =", None);
                }
                if !self.block(n) {
                    continue;
                }
                for field in ["trigger"] {
                    if n.field(field).is_none() {
                        self.error(
                            n,
                            "missing-field",
                            format!("complex_trigger_modifier requires {field}"),
                            None,
                        );
                    }
                }
                for field in n.body() {
                    if field.op.as_str() != "=" {
                        self.error(
                            field,
                            "operator",
                            "complex_trigger_modifier fields require =",
                            None,
                        );
                    }
                    match field.key.as_str() {
                        "parameters" => {
                            self.block(field);
                        }
                        "trigger" | "trigger_scope" | "mode" | "mult" | "multiplier"
                        | "min_value" => {
                            self.scalar_field(field);
                        }
                        _ => self.error(
                            field,
                            "unknown-field",
                            "unknown complex_trigger_modifier field",
                            None,
                        ),
                    }
                    if field.key == "mode"
                        && (!numeric_op(field.scalar()) || field.scalar() == "base")
                    {
                        self.error(field, "value", "unknown numeric script-value mode", None);
                    }
                }
            } else if !modifier {
                self.error(
                    n,
                    "unknown-field",
                    format!("unknown script-value operation {}", n.key),
                    None,
                );
            }
        }
    }
    pub(crate) fn script_value_reference(
        &mut self,
        n: &Node,
        ctx: &Context,
        state: &State,
    ) -> Option<NumberFact> {
        let raw = n.scalar();
        let (selector, expression) = if let Some(expression) = raw.strip_prefix("value:") {
            (None, expression)
        } else if let Some((selector, expression)) = raw.split_once(".value:") {
            (Some(selector), expression)
        } else {
            return None;
        };
        let unknown = NumberFact::assigned(None);
        let name = expression.split('|').next().unwrap_or("");
        if name.contains(['$', '[', ']', '@']) {
            self.gap(
                n,
                "dynamic script-value identity or arguments require expansion",
                "numeric-information-unresolved",
                Some(ctx),
            );
            return Some(unknown);
        }
        let Some(def) = self
            .program
            .definitions
            .get("script_values")
            .and_then(|defs| defs.get(name))
            .cloned()
        else {
            self.error(
                n,
                "missing-item",
                format!("unknown script value {name}"),
                Some(ctx),
            );
            return Some(unknown);
        };
        if def.op.as_str() != "=" {
            self.error(
                &def,
                "operator",
                "script-value definition requires =",
                Some(ctx),
            );
        }
        self.invoked_values.insert(name.into());
        if expression.contains(['$', '[', ']', '@'])
            || selector.is_some_and(|s| s.contains(['$', '[', ']', '@']))
        {
            self.gap(
                n,
                "dynamic script-value arguments or receiver require expansion",
                "numeric-information-unresolved",
                Some(ctx),
            );
            return Some(unknown);
        }
        let mut parts = expression.split('|').collect::<Vec<_>>();
        let name = parts.remove(0);
        if parts.last() == Some(&"") {
            parts.pop();
        }
        let mut args = BTreeMap::new();
        if parts.len() % 2 != 0
            || parts.chunks_exact(2).any(|p| {
                p[0].is_empty()
                    || p[1].is_empty()
                    || args.insert(p[0].into(), p[1].into()).is_some()
            })
        {
            self.error(
                n,
                "macro-argument",
                "script-value arguments must be unique name/value pairs",
                Some(ctx),
            );
            return Some(unknown);
        }
        let root = !self.active.iter().any(|key| key.starts_with("value:"));
        if root {
            self.value_work = 0;
        }
        let key = format!("value:{name}");
        if self.active.contains(&key) || self.active.len() >= 40 || self.value_work >= 4096 {
            self.complete = false;
            self.gap(
                n,
                format!("script-value recursion/work budget reached: {name}"),
                "analysis-limit",
                Some(ctx),
            );
            return Some(unknown);
        }
        let frame = selector.map_or_else(
            || ctx.clone(),
            |selector| self.resolve(selector, ctx, state, n, false),
        );
        let depth = self.chain.len();
        self.chain.push(serde_json::json!({"kind":"script-value","definition":name,"path":n.source,"line":n.line,"definition_path":def.source,"definition_line":def.line}).into());
        self.active.push(key);
        let result = if let Some(body) = self.script_value_body(&def, n, &args, &frame) {
            if def.macro_parts.is_none() && def.children.is_none() {
                self.error(
                    &def,
                    "structure",
                    "script value requires a block",
                    Some(&frame),
                );
                unknown
            } else {
                self.value_structure_body(&body, false);
                self.value_body(&body, &frame, state, NumberFact::assigned(Some(0.0)), false)
            }
        } else {
            unknown
        };
        self.active.pop();
        self.chain.truncate(depth);
        Some(result)
    }
    fn value_operand(&mut self, n: &Node, ctx: &Context, state: &State) -> NumberFact {
        if let Some(value) = self.script_value_reference(n, ctx, state) {
            return value;
        }
        let mut result = NumberFact::assigned(self.numeric_in_state(n, ctx, state));
        if n.scalar().parse::<f64>().is_ok() {
            return result;
        }
        let raw = n.scalar().strip_prefix("var:").unwrap_or(n.scalar());
        let (frame, name) = raw.rsplit_once('.').map_or_else(
            || (ctx.clone(), raw),
            |(selector, name)| (self.resolve(selector, ctx, state, n, true), name),
        );
        if crate::variables::variable_name(name) && raw.parse::<f64>().is_err() {
            result.nonzero = state
                .number(&State::number_key(&frame.object, name))
                .nonzero;
        }
        result
    }
    fn value_body(
        &mut self,
        body: &[Node],
        ctx: &Context,
        state: &State,
        mut result: NumberFact,
        modifier: bool,
    ) -> NumberFact {
        // base/weight/factor are initializers outside modifier, irrespective of
        // inline math placement. Multiple initializers have no verified order.
        if !modifier {
            let bases = body
                .iter()
                .filter(|n| matches!(n.key.as_str(), "base" | "weight" | "factor"))
                .collect::<Vec<_>>();
            if bases.len() > 1 {
                self.gap(
                    bases[1],
                    "multiple script-value initializers require engine order verification",
                    "numeric-information-unresolved",
                    Some(ctx),
                );
                result = NumberFact::assigned(None);
            }
            for n in &bases {
                let value = self.value_operand(n, ctx, state);
                if bases.len() == 1 {
                    result = value;
                }
            }
        }
        for n in body {
            self.value_work += 1;
            if self.value_work > 4096 {
                self.complete = false;
                self.gap(
                    n,
                    "script-value operation budget reached",
                    "analysis-limit",
                    Some(ctx),
                );
                return NumberFact::assigned(None);
            }
            if !modifier && matches!(n.key.as_str(), "base" | "weight" | "factor") {
                continue;
            }
            if math_op(&n.key) {
                let rhs = if numeric_op(&n.key) {
                    self.value_operand(n, ctx, state)
                } else {
                    NumberFact::assigned(None)
                };
                result = self.value_math(n, result, rhs, ctx);
            } else if !modifier && n.key == "modifier" && n.children.is_some() {
                let (math, guards): (Vec<_>, Vec<_>) =
                    n.body().iter().cloned().partition(|n| math_op(&n.key));
                self.triggers(&guards, ctx, state);
                let mut yes_state = state.clone();
                let yes = self.refine(&guards, ctx, &mut yes_state, true);
                if let Some(frame) = yes {
                    let changed = self.value_body(&math, &frame, &yes_state, result.clone(), true);
                    if self
                        .refine(&guards, ctx, &mut state.clone(), false)
                        .is_none()
                    {
                        result = changed;
                    } else {
                        result = result.join(&changed);
                    }
                }
            } else if !modifier && n.key == "complex_trigger_modifier" {
                let frame = n
                    .val("trigger_scope")
                    .map_or_else(|| ctx.clone(), |s| self.resolve(s, ctx, state, n, false));
                if let Some(trigger) = n.val("trigger") {
                    if let Some(rule) = self
                        .rules
                        .triggers
                        .get(trigger)
                        .or_else(|| self.rules.numeric_expressions.get(trigger))
                        .cloned()
                    {
                        self.compatible(
                            &frame.this,
                            &rule.input,
                            n,
                            "complex script-value trigger",
                            &frame,
                        );
                    } else if self.program.scripts.contains_key(&(false, trigger.into())) {
                        self.gap(n,"scripted complex numeric trigger return contract requires verification","numeric-trigger-contract-unverified",Some(&frame));
                    } else {
                        self.error(
                            n,
                            "unknown-trigger",
                            format!("unknown complex numeric trigger {trigger}"),
                            Some(&frame),
                        );
                    }
                }
                if let Some(mult) = n.field("mult") {
                    self.value_operand(mult, &frame, state);
                }
                if let Some(multiplier) = n.field("multiplier") {
                    self.value_operand(multiplier, &frame, state);
                    self.gap(multiplier,"shipped multiplier spelling is valid; its relation to mult requires verification","numeric-query-multiplier-unverified",Some(&frame));
                }
                let verified = self.complex_value_query(n, &frame, state);
                if !verified {
                    self.gap(n, "complex numeric trigger parameters and result require a dedicated contract", "numeric-trigger-contract-unverified", Some(&frame));
                }
                if let Some(min) = n.field("min_value") {
                    self.value_operand(min, &frame, state);
                    self.gap(
                        min,
                        "numeric query clamp order relative to mult requires verification",
                        "numeric-clamp-contract-unverified",
                        Some(&frame),
                    );
                }
                if n.val("mode").is_none() {
                    self.gap(
                        n,
                        "default complex numeric trigger mode requires verification",
                        "numeric-information-unresolved",
                        Some(&frame),
                    );
                }
                let operation = Node {
                    key: n.val("mode").unwrap_or("").into(),
                    ..n.clone_header()
                };
                result = self.value_math(&operation, result, NumberFact::assigned(None), &frame);
            }
        }
        result
    }
    fn value_math(
        &mut self,
        n: &Node,
        left: NumberFact,
        right: NumberFact,
        ctx: &Context,
    ) -> NumberFact {
        let (a, b) = (left.value(), right.value());
        if n.key == "abs" && n.scalar() == "yes" {
            let mut result = NumberFact::assigned(a.map(f64::abs));
            result.nonzero = left.nonzero;
            return result;
        }
        if n.key == "pow" {
            if a == Some(0.0) && b.is_some_and(|b| b < 0.0) {
                self.error(
                    n,
                    "value",
                    "zero cannot be raised to a negative power",
                    Some(ctx),
                );
                return NumberFact::assigned(None);
            }
            if let Some((base, exponent)) = a.zip(b)
                && exponent.fract() == 0.0
                && exponent.abs() <= 32.0
                && !(base == 0.0 && exponent == 0.0)
            {
                let value = base.powi(exponent as i32);
                let single = (base as f32).powi(exponent as i32) as f64;
                if value.is_finite() && value == single && (value as f32) as f64 == value {
                    return NumberFact::assigned(Some(value));
                }
            }
            self.gap(
                n,
                "power result, domain or precision is not proven by the bounded numeric model",
                "numeric-power-unproven",
                Some(ctx),
            );
            return NumberFact::assigned(None);
        }
        if matches!(n.key.as_str(), "divide" | "modulo" | "round_to") {
            if b == Some(0.0) {
                self.error(
                    n,
                    "value",
                    "script-value divisor or rounding multiple must not be zero",
                    Some(ctx),
                );
                return NumberFact::assigned(None);
            }
            if !right.nonzero {
                self.gap(
                    n,
                    "script-value divisor is not proven nonzero",
                    "numeric-divisor-unproven",
                    Some(ctx),
                );
            }
        }
        let value = match n.key.as_str() {
            "base" | "weight" | "set" => return right,
            "add" => a.zip(b).map(|(a, b)| a + b),
            "subtract" => a.zip(b).map(|(a, b)| a - b),
            "mult" | "multiply" | "factor" => a.zip(b).map(|(a, b)| a * b),
            "divide" => a.zip(b).map(|(a, b)| a / b),
            "modulo" => a.zip(b).map(|(a, b)| a % b),
            "min" => a.zip(b).map(|(a, b)| a.max(b)),
            "max" => a.zip(b).map(|(a, b)| a.min(b)),
            "ceiling" if n.scalar() == "yes" => a.map(f64::ceil),
            "floor" if n.scalar() == "yes" => a.map(f64::floor),
            "square" if n.scalar() == "yes" => a.map(|a| a * a),
            "square_root" if n.scalar() == "yes" => {
                if a.is_some_and(|a| a < 0.0) {
                    self.error(n, "value", "square_root input is negative", Some(ctx));
                }
                a.filter(|a| *a >= 0.0).map(f64::sqrt)
            }
            "round" if n.scalar() == "yes" => a.filter(|a| a.fract().abs() != 0.5).map(f64::round),
            "round_to" => a.zip(b).and_then(|(a, b)| {
                let q = a / b;
                (q.is_finite() && (q as f32) as f64 == q && q.fract().abs() != 0.5)
                    .then(|| q.round() * b)
            }),
            _ if unary_op(&n.key) && n.scalar() == "no" => return left,
            // pow implementation and rounding precision remain unproved; never
            // use their guessed constants to certify a later division.
            _ => None,
        };
        NumberFact::assigned(value)
    }
    pub(crate) fn uninvoked_script_values(&mut self) {
        let defs = self
            .program
            .definitions
            .get("script_values")
            .into_iter()
            .flat_map(|d| d.iter())
            .filter(|(name, n)| {
                !self.invoked_values.contains(*name)
                    && self
                        .program
                        .files
                        .iter()
                        .any(|f| f.target && f.path == n.source.as_ref())
            })
            .map(|(_, n)| n.clone())
            .collect::<Vec<_>>();
        for def in defs {
            self.gap(
                &def,
                "script value has no established caller context",
                "caller-context-unknown",
                None,
            );
            if def.macro_parts.is_none() {
                let call = Node {
                    value: Some(format!("value:{}", def.key)),
                    ..(*def).clone()
                };
                self.script_value_reference(&call, &Context::unknown_caller(), &State::default());
            }
        }
    }
}
