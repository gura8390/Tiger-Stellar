use std::collections::BTreeMap;
#[derive(Clone, Debug)]
pub struct WeaponStats {
    pub fields: BTreeMap<String, String>,
    pub source: String,
    pub line: u32,
}
pub fn rows(text: &str) -> Result<Vec<(u32, Vec<String>)>, String> {
    let mut out = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut line = 1;
    let mut start = 1;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
                if c == '\n' {
                    line += 1;
                }
            }
            continue;
        }
        if c == '#' && row.is_empty() && field.trim().is_empty() {
            field.clear();
            for c in chars.by_ref() {
                if c == '\n' {
                    line += 1;
                    break;
                }
            }
            start = line;
            continue;
        }
        match c {
            '"' if field.is_empty() => quoted = true,
            ';' => row.push(std::mem::take(&mut field)),
            '\r' if chars.peek() == Some(&'\n') => {}
            '\n' | '\r' => {
                row.push(std::mem::take(&mut field));
                if row.iter().any(|s| !s.trim().is_empty()) {
                    out.push((start, std::mem::take(&mut row)));
                } else {
                    row.clear();
                }
                line += 1;
                start = line;
            }
            _ => field.push(c),
        }
    }
    if quoted {
        return Err(format!("unterminated CSV quote at row {start}"));
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        out.push((start, row));
    }
    Ok(out)
}
pub fn weapons(text: &str, path: &str) -> Result<BTreeMap<String, WeaponStats>, String> {
    let data = rows(text)?;
    let mut header = None;
    let mut result = BTreeMap::new();
    for (line, row) in data {
        if row.first().is_some_and(|s| s.trim() == "key") {
            header = Some(row);
            continue;
        }
        let Some(fields) = &header else {
            return Err(format!(
                "{path}:{line}: weapon CSV is missing its key header"
            ));
        };
        if row.first().is_none_or(|s| s.trim().is_empty()) {
            continue;
        }
        let key = row[0].trim().to_string();
        let values = fields
            .iter()
            .zip(row.iter())
            .map(|(k, v)| (k.trim().into(), v.trim().into()))
            .collect();
        result.insert(
            key,
            WeaponStats {
                fields: values,
                source: path.into(),
                line,
            },
        );
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quoted_and_comments() {
        let r = rows(
            "# comment \"ignored\"\r\nkey;label;min_damage;max_damage\r\nx;\"a;\"\"b\"\"\";1;2\r\n",
        )
        .unwrap();
        assert_eq!(r[1].0, 3);
        assert_eq!(r[1].1[1], "a;\"b\"");
    }
    #[test]
    fn broken_quotes_fail() {
        assert!(rows("key;x\na;\"bad").is_err());
    }
    #[test]
    fn no_header_is_not_empty_success() {
        assert!(weapons("x;1;2\n", "x.csv").is_err());
    }
}
