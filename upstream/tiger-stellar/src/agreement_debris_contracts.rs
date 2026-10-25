//! Agreement assignments and debris resource maps evidenced by Stellaris 4.5.2.
//! Registration/type checks do not prove a term's runtime `possible` predicate,
//! that an agreement exists, or that debris notifications can be displayed.
use crate::{
    ast::Node,
    domain::{Context, State},
    engine::Engine,
};
use std::collections::BTreeSet;

fn dynamic(value: &str) -> bool {
    value.contains(['$', '@', ':', '|', '[', ']'])
}

impl Engine {
    fn agreement_debris_number_shape(&mut self, n: &Node) {
        if !self.scalar_field(n) {
            return;
        }
        // The identity check mirrors script_values::value_structure_body. It
        // must also run in a false branch, without evaluating a value, resolving
        // its receiver, reading numeric facts, or recording a runtime invocation.
        let raw = n.scalar();
        let reference = raw
            .strip_prefix("value:")
            .map(|expression| (None, expression))
            .or_else(|| {
                raw.split_once(".value:")
                    .map(|(selector, expression)| (Some(selector), expression))
            });
        if let Some((selector, expression)) = reference {
            let name = expression.split('|').next().unwrap_or("");
            if name.contains(['$', '[', ']', '@']) {
                self.gap(
                    n,
                    "dynamic script-value identity requires expansion",
                    "numeric-information-unresolved",
                    None,
                );
            } else {
                if !self
                    .program
                    .definitions
                    .get("script_values")
                    .is_some_and(|definitions| definitions.contains_key(name))
                {
                    self.error(
                        n,
                        "missing-item",
                        format!("unknown script value {name}"),
                        None,
                    );
                }
                // A fixed missing identity is still an error when arguments or
                // receiver are dynamic. Unknown expansion must not hide it.
                if expression.contains(['$', '[', ']', '@'])
                    || selector.is_some_and(|value| value.contains(['$', '[', ']', '@']))
                {
                    self.gap(
                        n,
                        "dynamic script-value arguments or receiver require expansion",
                        "numeric-information-unresolved",
                        None,
                    );
                }
            }
        }
        if let Ok(value) = raw.parse::<f64>()
            && !value.is_finite()
        {
            self.error(n, "value", "numeric value must be finite", None);
        }
    }

    fn agreement_has_term(&self, name: &str) -> bool {
        self.program
            .entities
            .get("agreement_terms")
            .is_some_and(|terms| terms.contains(name))
    }

    fn agreement_term_assignment_shape(&mut self, n: &Node) {
        if dynamic(&n.key) {
            self.symbol_reference("agreement_terms", &n.key, n);
            return;
        }
        // The current export gives this exact numeric selector. A custom term
        // with the same name leaves precedence unresolved, rather than silently
        // preferring either the database or the built-in selector.
        if n.key == "resource_subsidies_alloys" {
            if self.agreement_has_term(&n.key) {
                self.gap(n, "agreement term and documented resource selector share an identity; precedence is unverified", "agreement-term-selector-ambiguity", None);
            } else {
                self.symbol_reference("strategic_resources", "alloys", n);
                self.agreement_debris_number_shape(n);
            }
            return;
        }
        if n.key.starts_with("resource_subsidies_") && !self.agreement_has_term(&n.key) {
            self.gap(
                n,
                "agreement resource selector is not established by the current exported example",
                "agreement-resource-selector-unverified",
                None,
            );
            return;
        }
        self.symbol_reference("agreement_terms", &n.key, n);
        if !self.agreement_has_term(&n.key) {
            return;
        }
        let term_type = self
            .program
            .definitions
            .get("agreement_terms")
            .and_then(|terms| terms.get(&n.key))
            .and_then(|term| term.val("term_type"))
            .map(str::to_string);
        match term_type.as_deref() {
            Some("discrete") => {}
            Some("resource") => {
                self.gap(n, "direct resource-term assignment is not established by the exported numeric selector example", "agreement-resource-term-form-unverified", None);
                return;
            }
            _ => {
                self.gap(
                    n,
                    "agreement term value grammar is not established for this term_type",
                    "agreement-term-contract-unverified",
                    None,
                );
                return;
            }
        }
        if !self.scalar_field(n) {
            return;
        }
        self.symbol_reference("agreement_term_values", n.scalar(), n);
        if dynamic(n.scalar()) {
            return;
        }
        let value_exists = self
            .program
            .entities
            .get("agreement_term_values")
            .is_some_and(|values| values.contains(n.scalar()));
        if !value_exists {
            return;
        }
        let membership = self
            .program
            .definitions
            .get("agreement_term_values")
            .and_then(|values| values.get(n.scalar()))
            .and_then(|value| value.val("term"))
            .map(str::to_string);
        match membership {
            Some(term) if !dynamic(&term) => {
                if term != n.key {
                    self.error(
                        n,
                        "value",
                        format!(
                            "agreement term value {} belongs to {term}, not {}",
                            n.scalar(),
                            n.key
                        ),
                        None,
                    );
                }
            }
            _ => self.gap(
                n,
                "agreement term value membership cannot be established from its definition",
                "agreement-term-membership-unverified",
                None,
            ),
        }
    }

    /// Static checks run even in unreachable branches. Return true only for the
    /// two effects owned by this module; normal rule.input checks remain active.
    pub(crate) fn agreement_debris_structure(&mut self, n: &Node) -> bool {
        if !matches!(
            n.key.as_str(),
            "set_agreement_terms" | "add_resource_from_debris"
        ) {
            return false;
        }
        if n.op.as_str() != "=" {
            self.error(n, "operator", format!("{} requires =", n.key), None);
        }
        if !self.block(n) {
            return true;
        }
        let agreement = n.key == "set_agreement_terms";
        if n.body().is_empty() {
            self.gap(
                n,
                "empty assignment-map behavior is not established by the current examples",
                if agreement {
                    "agreement-term-contract-unverified"
                } else {
                    "debris-resource-contract-unverified"
                },
                None,
            );
        }
        if !agreement && !n.body().is_empty() && n.field("system").is_none() {
            self.gap(
                n,
                "debris notification system default is not established by the current examples",
                "debris-system-default-unverified",
                None,
            );
        }
        if !agreement
            && !n.body().is_empty()
            && !n
                .body()
                .iter()
                .any(|field| !matches!(field.key.as_str(), "" | "system" | "mult"))
        {
            self.gap(n, "debris map without a resource assignment is not established by the current examples", "debris-resource-contract-unverified", None);
        }
        let mut seen = BTreeSet::new();
        for field in n.body() {
            if field.key.is_empty() {
                self.error(
                    field,
                    "structure",
                    format!("{} expects named assignments", n.key),
                    None,
                );
                continue;
            }
            if !seen.insert(field.key.as_str()) {
                self.gap(
                    field,
                    "repeated assignment composition is not established by the current examples",
                    if agreement {
                        "agreement-term-composition-unverified"
                    } else {
                        "debris-resource-composition-unverified"
                    },
                    None,
                );
            }
            if field.op.as_str() != "=" {
                self.error(
                    field,
                    "operator",
                    format!("{} assignments require =", n.key),
                    None,
                );
            }
            if agreement {
                self.agreement_term_assignment_shape(field);
            } else if field.key == "system" {
                self.scalar_field(field);
            } else {
                if field.key != "mult" {
                    self.symbol_reference("strategic_resources", &field.key, field);
                }
                self.agreement_debris_number_shape(field);
            }
        }
        true
    }

    /// Called with the pre-effect numeric inputs, before generic unknown
    /// parameter fallback. No bindings, refinements or numeric facts escape.
    pub(crate) fn agreement_debris_parameters(
        &mut self,
        n: &Node,
        ctx: &Context,
        inputs: &State,
    ) -> bool {
        match n.key.as_str() {
            "set_agreement_terms" => {
                for field in n.body() {
                    if field.key != "resource_subsidies_alloys"
                        || self.agreement_has_term(&field.key)
                        || field.children.is_some()
                    {
                        continue;
                    }
                    if field.scalar().parse::<f64>().is_ok() {
                        self.numeric_in_state(field, ctx, inputs);
                    } else {
                        // The export proves a float, not the evaluation object
                        // of a runtime query. Keep independent identities so a
                        // caller's known zero cannot create a false division.
                        let mut frame = Context::unknown_caller();
                        frame.object = format!("agreement-expression:{}.this", field.id);
                        frame.root_object = format!("agreement-expression:{}.root", field.id);
                        self.gap(field, "agreement resource expression evaluation context is not established by the current literal example", "agreement-resource-value-unverified", Some(&frame));
                        self.numeric_in_state(field, &frame, inputs);
                    }
                }
                true
            }
            "add_resource_from_debris" => {
                for field in n.body() {
                    if field.key.is_empty() || field.children.is_some() {
                        continue;
                    }
                    if field.key == "system" {
                        let target = self.resolve(field.scalar(), ctx, inputs, field, false);
                        self.compatible(
                            &target.this,
                            &["galactic_object".into()],
                            field,
                            "debris notification system",
                            ctx,
                        );
                    } else {
                        self.numeric_in_state(field, ctx, inputs);
                    }
                }
                true
            }
            _ => false,
        }
    }
}
