use crate::{ast::Node, engine::Engine};

impl Engine {
    pub(crate) fn flag_structure(&mut self, n: &Node) {
        self.flag_fields(n, true);
    }
    pub(crate) fn change_flag_structure(&mut self, n: &Node) {
        if n.children.is_none() {
            if self.scalar_field(n) && n.scalar() != "random" {
                self.error(
                    n,
                    "value",
                    "change_country_flag scalar expects random",
                    None,
                );
            }
        } else {
            self.flag_fields(n, false);
        }
    }
    fn flag_fields(&mut self, n: &Node, creation: bool) {
        if n.children.is_none() {
            self.scalar_field(n);
            return;
        }
        for field in n.body() {
            match field.key.as_str() {
                "icon" | "background" => {
                    if !self.block(field) {
                        continue;
                    }
                    for key in ["category", "file"] {
                        if field.field(key).is_none() {
                            self.error(
                                field,
                                "missing-field",
                                format!("flag image requires {key}"),
                                None,
                            );
                        }
                    }
                    for part in field.body() {
                        if matches!(part.key.as_str(), "category" | "file") {
                            self.scalar_field(part);
                        } else {
                            self.error(
                                part,
                                "unknown-field",
                                format!("unknown flag image field {}", part.key),
                                None,
                            );
                        }
                    }
                    if let (Some(category), Some(file)) = (field.val("category"), field.val("file"))
                    {
                        self.asset_reference(&format!("flags/{category}/{file}"), field);
                    }
                }
                "colors" => {
                    if !self.block(field) {
                        continue;
                    }
                    if creation && field.body().len() != 4 {
                        self.error(
                            field,
                            "structure",
                            "flag colors requires four entries",
                            None,
                        );
                    }
                    if !creation && !matches!(field.body().len(), 2 | 4) {
                        self.gap(
                            field,
                            "change_country_flag color count is not verified by native examples",
                            "engine-contract-unverified",
                            None,
                        );
                    }
                    for color in field.body() {
                        if !color.key.is_empty()
                            || color.children.is_some()
                            || color.value.is_none()
                        {
                            self.error(
                                color,
                                "structure",
                                "flag colors expects a scalar list",
                                None,
                            );
                        } else if color.scalar() != "null" {
                            self.symbol_reference("flag_colors", color.scalar(), color);
                        }
                    }
                }
                "use_ship_color" if creation => self.boolean(field),
                _ => self.error(
                    field,
                    "unknown-field",
                    format!("unknown flag field {}", field.key),
                    None,
                ),
            }
        }
    }
}
