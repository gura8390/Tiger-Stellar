use crate::{
    ast::Node,
    domain::{Context, State},
    engine::Engine,
};

impl Engine {
    fn display_expand(&mut self, n: &Node, ctx: &Context) -> Option<Node> {
        fn has_inline(n: &Node) -> bool {
            n.key == "inline_script" || n.body().iter().any(has_inline)
        }
        if !n.body().iter().any(has_inline) {
            return None;
        }
        let mut result = n.clone_header();
        result.children = Some(self.expand_tree(n.body(), ctx).into_boxed_slice());
        Some(result)
    }
    fn description_structure(&mut self, n: &Node) {
        if !self.description_seen.insert(n.id) {
            return;
        }
        self.structure_visit(n, false);
        if n.children.is_none() {
            if self.scalar_field(n) {
                self.localisation(n);
            }
            return;
        }
        let mut preceding = false;
        for c in n.body() {
            let key = c.key.to_ascii_lowercase();
            if matches!(key.as_str(), "else_if" | "else") && !preceding {
                self.error(c, "structure", "description else has no preceding if", None);
            }
            preceding = key == "if" || (key == "else_if" && preceding);
            match key.as_str() {
                "text" => {
                    if self.scalar_field(c) {
                        self.localisation(c);
                    }
                }
                "show_sound" => {
                    self.scalar_field(c);
                }
                "trigger" | "exclusive_trigger" => {
                    if self.block(c) {
                        let ordinary = c
                            .body()
                            .iter()
                            .filter(|v| v.key != "switch")
                            .cloned()
                            .collect::<Vec<_>>();
                        self.structure_script(&ordinary, false);
                        for condition in c.body().iter().filter(|v| v.key == "switch") {
                            self.description_switch_structure(condition);
                        }
                    }
                }
                "switch" => self.description_switch_structure(c),
                "desc" | "random_valid" | "first_valid" => self.description_structure(c),
                "if" | "else_if" | "else" => {
                    if !self.block(c) {
                        continue;
                    }
                    if key != "else" && c.field("limit").is_none() {
                        self.error(
                            c,
                            "missing-field",
                            "conditional description requires limit",
                            None,
                        );
                    }
                    if let Some(limit) = c.field("limit")
                        && self.block(limit)
                    {
                        self.structure_script(limit.body(), false);
                    }
                    let mut body = c.clone_header();
                    body.children = Some(
                        c.body()
                            .iter()
                            .filter(|c| c.key != "limit")
                            .cloned()
                            .collect(),
                    );
                    self.description_structure(&body);
                }
                _ => self.error(
                    c,
                    "unknown-field",
                    format!("unknown description field {}", c.key),
                    None,
                ),
            }
        }
    }
    fn description_switch_structure(&mut self, n: &Node) {
        if !self.block(n) {
            return;
        }
        if let Some(selector) = n.field("trigger") {
            self.scalar_field(selector);
        } else {
            self.error(
                n,
                "missing-field",
                "description switch requires trigger",
                None,
            );
        }
        for branch in n.body().iter().filter(|b| b.key != "trigger") {
            self.description_structure(branch);
        }
    }
    pub(crate) fn description(&mut self, n: &Node, ctx: &Context, state: &State) {
        let expanded = self.display_expand(n, ctx);
        let n = expanded.as_ref().unwrap_or(n);
        self.description_structure(n);
        if n.children.is_none() {
            return;
        }
        let mut pending = None;
        for c in n.body() {
            let key = c.key.to_ascii_lowercase();
            if matches!(key.as_str(), "if" | "else_if" | "else") {
                let base = if key == "if" {
                    Some((ctx.clone(), state.clone()))
                } else {
                    pending.take()
                };
                if let Some((before, mut input)) = base {
                    let mut no = input.clone();
                    let limit = c.field("limit").map(Node::body).unwrap_or(&[]);
                    if key != "else" {
                        self.triggers(limit, &before, &input);
                    }
                    let yes = if key == "else" {
                        Some(before.clone())
                    } else {
                        self.refine(limit, &before, &mut input, true)
                    };
                    if let Some(yes) = yes {
                        let mut body = c.clone_header();
                        body.children = Some(
                            c.body()
                                .iter()
                                .filter(|b| b.key != "limit")
                                .cloned()
                                .collect(),
                        );
                        self.description(&body, &yes, &input);
                    }
                    pending = if key == "else" {
                        None
                    } else {
                        self.refine(limit, &before, &mut no, false).map(|v| (v, no))
                    };
                }
                continue;
            }
            pending = None;
            match key.as_str() {
                "trigger" | "exclusive_trigger" => {
                    let ordinary = c
                        .body()
                        .iter()
                        .filter(|v| v.key != "switch")
                        .cloned()
                        .collect::<Vec<_>>();
                    self.triggers(&ordinary, ctx, state);
                    for switch in c.body().iter().filter(|v| v.key == "switch") {
                        self.description_switch(switch, ctx, state);
                    }
                }
                "switch" => self.description_switch(c, ctx, state),
                "desc" | "random_valid" | "first_valid" => self.description(c, ctx, state),
                _ => {}
            }
        }
    }
    fn description_switch(&mut self, n: &Node, ctx: &Context, state: &State) {
        let Some(selector) = n.val("trigger") else {
            return;
        };
        let known = self.rules.triggers.contains_key(selector)
            || self.program.scripts.contains_key(&(false, selector.into()));
        if !known {
            self.error(
                n.field("trigger").unwrap(),
                "unknown-trigger",
                format!("unknown description switch selector {selector}"),
                Some(ctx),
            );
        }
        for branch in n.body().iter().filter(|b| b.key != "trigger") {
            if known && branch.key != "default" {
                let probe = Node {
                    key: selector.into(),
                    value: Some(branch.key.clone()),
                    children: None,
                    op: "=".into(),
                    ..branch.clone_header()
                };
                self.triggers(&[probe], ctx, state);
            }
            self.description(branch, ctx, state);
        }
    }
    fn picture_symbol(&mut self, n: &Node) {
        if self.scalar_field(n) && !matches!(n.scalar(), "" | "none" | "default") {
            self.symbol_reference("gfx_spritetype", n.scalar(), n);
        }
    }
    pub(crate) fn conditional_picture(&mut self, n: &Node, ctx: &Context, state: &State) {
        if n.children.is_none() {
            self.picture_symbol(n);
            return;
        }
        let expanded = self.display_expand(n, ctx);
        let n = expanded.as_ref().unwrap_or(n);
        for c in n.body() {
            match c.key.as_str() {
                "trigger" => {
                    if self.block(c) {
                        self.triggers(c.body(), ctx, state);
                    }
                }
                "picture" => self.conditional_picture(c, ctx, state),
                _ => self.error(
                    c,
                    "unknown-field",
                    format!("unknown conditional picture field {}", c.key),
                    Some(ctx),
                ),
            }
        }
    }
    pub(crate) fn option_icon(&mut self, n: &Node, ctx: &Context, state: &State) {
        if n.children.is_none() {
            self.picture_symbol(n);
            return;
        }
        let expanded = self.display_expand(n, ctx);
        let n = expanded.as_ref().unwrap_or(n);
        for c in n.body() {
            match c.key.as_str() {
                "icon" | "icon_background" => self.picture_symbol(c),
                "text" => {
                    if self.scalar_field(c) {
                        self.localisation(c);
                    }
                }
                "trigger" => {
                    if self.block(c) {
                        self.triggers(c.body(), ctx, state);
                    }
                }
                _ => self.error(
                    c,
                    "unknown-field",
                    format!("unknown option icon field {}", c.key),
                    Some(ctx),
                ),
            }
        }
    }
}
