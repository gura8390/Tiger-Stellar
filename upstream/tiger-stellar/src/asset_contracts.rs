use crate::{
    ast::Node,
    domain::{Context, State},
    engine::Engine,
};
use std::io::Read;

/// Basic exported 0.1.8 DDS contract. Payload, format compatibility and rendering
/// are deliberately not claimed by a valid header.
pub(crate) fn dds_header_valid(data: &[u8]) -> bool {
    let u32_at = |offset| u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
    data.len() >= 128
        && &data[..4] == b"DDS "
        && u32_at(4) == 124
        && u32_at(12) > 0
        && u32_at(16) > 0
        && u32_at(76) == 32
        && (&data[84..88] != b"DX10" || data.len() >= 148)
}

impl Engine {
    pub(crate) fn asset_reference(&mut self, path: &str, n: &Node) -> bool {
        let rel = path.replace('\\', "/").to_ascii_lowercase();
        if rel.starts_with('/') || rel.contains(':') || rel.split('/').any(|p| p == "..") {
            self.error(
                n,
                "value",
                "resource reference must stay within the game resource tree",
                None,
            );
            return false;
        }
        if rel.contains(['$', '@', '[', ']']) {
            self.gap(
                n,
                format!("dynamic resource reference: {rel}"),
                "dynamic-reference",
                None,
            );
            return false;
        }
        let mut candidates = vec![rel.clone()];
        if let Some(source) = self.program.relative_sources.get(n.source.as_ref()) {
            if let Some((parent, _)) = source.rsplit_once('/') {
                candidates.push(format!("{parent}/{rel}"));
            }
            if source.starts_with("gfx/models/")
                && !rel.contains('/')
                && (rel.ends_with(".dds") || rel.ends_with(".tga"))
            {
                candidates.push(format!("gfx/models/{rel}"));
            }
        }
        let found = candidates.into_iter().find_map(|key| {
            self.program
                .asset_files
                .get(&key)
                .cloned()
                .map(|provider| (key, provider))
        });
        let Some((rel, (path, layer))) = found else {
            self.error(
                n,
                "missing-item",
                format!("unknown resource file `{rel}`"),
                None,
            );
            return false;
        };
        if !self.program.asset_health.contains_key(&rel) {
            if let Err(error) = self.program.record(&path, &rel, layer) {
                self.error(
                    n,
                    "input-read",
                    format!("cannot read referenced resource {rel}: {error}"),
                    None,
                );
                return false;
            }
            let good = if rel.ends_with(".dds") {
                let mut header = Vec::with_capacity(148);
                match std::fs::File::open(&path)
                    .and_then(|file| file.take(148).read_to_end(&mut header))
                {
                    Ok(_) => dds_header_valid(&header),
                    Err(error) => {
                        self.error(
                            n,
                            "input-read",
                            format!("cannot read DDS header {rel}: {error}"),
                            None,
                        );
                        return false;
                    }
                }
            } else {
                true
            };
            self.program.asset_health.insert(rel.clone(), good);
        }
        let good = self.program.asset_health[&rel];
        if !good {
            self.error(
                n,
                "dds-header",
                format!("invalid or truncated DDS header/dimensions: {rel}"),
                None,
            );
        }
        good
    }
    pub(crate) fn graphics_references(&mut self, n: &Node) {
        let kind = n.key.to_ascii_lowercase();
        if matches!(
            kind.as_str(),
            "texturefile"
                | "texture"
                | "file"
                | "diffuse"
                | "normal"
                | "specular"
                | "texture_diffuse"
                | "texture_normal"
                | "texture_specular"
                | "diffuse_texture"
                | "normal_texture"
                | "specular_texture"
        ) && n.value.is_some()
            && (n.scalar().contains(['/', '\\'])
                || [".dds", ".tga", ".mesh", ".anim"]
                    .iter()
                    .any(|suffix| n.scalar().to_ascii_lowercase().ends_with(suffix)))
            && self.scalar_field(n)
        {
            self.asset_reference(n.scalar(), n);
        }
        if n.children.is_none()
            && matches!(
                n.key.to_ascii_lowercase().as_str(),
                "pdxmesh" | "clone" | "entity"
            )
            && self.scalar_field(n)
        {
            self.reference(
                if n.key.eq_ignore_ascii_case("pdxmesh") {
                    "gfx_pdxmesh"
                } else {
                    "gfx_entity"
                },
                n.scalar(),
                n,
            );
        }
        for c in n.body() {
            self.graphics_references(c);
        }
    }
    pub(crate) fn aura_graphics(&mut self, n: &Node, ctx: &Context) {
        if !self.block(n) {
            return;
        }
        for effect in n.body() {
            if !matches!(effect.key.as_str(), "area_effect" | "ship_effect") {
                self.error(
                    effect,
                    "unknown-field",
                    "unknown aura graphics effect",
                    Some(ctx),
                );
                continue;
            }
            if !self.block(effect) {
                continue;
            }
            if effect.field("entity").is_none() {
                self.error(
                    effect,
                    "missing-field",
                    "aura graphics effect requires entity",
                    Some(ctx),
                );
            }
            for c in effect.body() {
                match c.key.as_str() {
                    "entity" => {
                        if self.scalar_field(c) {
                            self.reference("gfx_entity", c.scalar(), c);
                        }
                    }
                    "dynamic_scale" => self.boolean(c),
                    _ => self.error(
                        c,
                        "unknown-field",
                        format!("unknown aura graphics field {}", c.key),
                        Some(ctx),
                    ),
                }
            }
        }
    }
    pub(crate) fn picture_event_data(&mut self, n: &Node, ctx: &Context, state: &State) {
        if !self.block(n) {
            return;
        }
        for c in n.body() {
            let accepted: &[&str] = match c.key.as_str() {
                "portrait" => &["country", "leader"],
                "planet_background" => &["country", "planet"],
                "graphical_culture" | "city_level" | "room" => &["country"],
                _ => {
                    self.gap(
                        c,
                        format!("picture_event_data unmodeled selector: {}", c.key),
                        "rule-unimplemented",
                        Some(ctx),
                    );
                    continue;
                }
            };
            if !self.scalar_field(c) {
                continue;
            }
            if self.is_scope(c.scalar()) {
                // A possible writer can establish a type without establishing
                // that the selector exists at display time. Keep both checks.
                let target = self.resolve(c.scalar(), ctx, state, c, false);
                if target.this.unknown
                    || !target
                        .this
                        .known
                        .iter()
                        .all(|t| accepted.contains(&t.as_str()))
                {
                    self.gap(
                        c,
                        format!(
                            "picture_event_data selector contract unverified for {}: {}",
                            c.key,
                            target.this.label()
                        ),
                        if target.this.unknown {
                            "scope-type-unknown"
                        } else {
                            "engine-contract-unverified"
                        },
                        Some(ctx),
                    );
                }
            } else if c.key == "portrait" {
                self.reference("gfx_portrait", c.scalar(), c);
            } else if c.key == "room" {
                self.asset_reference(&format!("gfx/portraits/city_sets/{}.dds", c.scalar()), c);
            } else {
                self.gap(
                    c,
                    format!(
                        "picture_event_data literal selector not yet migrated: {}",
                        c.key
                    ),
                    "rule-unimplemented",
                    Some(ctx),
                );
            }
        }
    }
}
