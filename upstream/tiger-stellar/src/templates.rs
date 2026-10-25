use std::collections::BTreeMap;

pub fn render(text: &str, args: &BTreeMap<String, String>) -> Result<String, String> {
    fn conditional(
        text: &str,
        args: &BTreeMap<String, String>,
        pos: &mut usize,
        closing: bool,
    ) -> Result<String, String> {
        let mut out = String::new();
        let mut quoted = false;
        let mut escaped = false;
        let mut comment = false;
        while *pos < text.len() {
            let tail = &text[*pos..];
            let ch = tail.chars().next().unwrap();
            if comment {
                out.push(ch);
                *pos += ch.len_utf8();
                if ch == '\n' {
                    comment = false;
                }
                continue;
            }
            if quoted {
                out.push(ch);
                *pos += ch.len_utf8();
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    quoted = false;
                }
                continue;
            }
            if ch == '#' {
                comment = true;
                out.push(ch);
                *pos += 1;
                continue;
            }
            if ch == '"' {
                quoted = true;
                out.push(ch);
                *pos += 1;
                continue;
            }
            if tail.starts_with("[[") {
                let end = tail.find(']').ok_or("unterminated conditional header")?;
                let cond = tail[2..end].trim();
                if cond.is_empty() {
                    return Err("empty conditional parameter".into());
                }
                *pos += end + 1;
                let body = conditional(text, args, pos, true)?;
                out.push_str(&" ".repeat(end + 1));
                let active =
                    args.contains_key(cond.trim_start_matches('!')) != cond.starts_with('!');
                if active {
                    out.push_str(&body);
                } else {
                    out.extend(body.chars().map(|c| if c == '\n' { '\n' } else { ' ' }));
                }
                out.push(' ');
                continue;
            }
            if tail.starts_with("@[") || tail.starts_with("@\\[") {
                let mut depth = 0;
                let mut end = None;
                for (offset, c) in tail.char_indices() {
                    if c == '[' {
                        depth += 1;
                    } else if c == ']' {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(offset + 1);
                            break;
                        }
                    }
                }
                let end = end.ok_or("unterminated reader expression")?;
                out.push_str(&tail[..end]);
                *pos += end;
                continue;
            }
            if ch == ']' && closing {
                *pos += 1;
                return Ok(out);
            }
            out.push(ch);
            *pos += ch.len_utf8();
        }
        if closing {
            Err("unterminated conditional block".into())
        } else {
            Ok(out)
        }
    }
    let active = conditional(text, args, &mut 0, false)?;
    let mut out = String::new();
    let mut pos = 0;
    let mut comment = false;
    let mut quoted = false;
    let mut escaped = false;
    while pos < active.len() {
        let tail = &active[pos..];
        let ch = tail.chars().next().unwrap();
        if comment {
            out.push(ch);
            pos += ch.len_utf8();
            if ch == '\n' {
                comment = false;
            }
            continue;
        }
        if ch == '#' && !quoted {
            comment = true;
            out.push(ch);
            pos += 1;
            continue;
        }
        if ch == '$' {
            let end = tail[1..]
                .find('$')
                .map(|i| i + 1)
                .ok_or("unterminated template parameter")?;
            let spec = &tail[1..end];
            if spec.contains(['\n', '\r']) {
                return Err("multiline template parameter".into());
            }
            let (name, default) = spec
                .split_once('|')
                .map_or((spec, None), |(n, d)| (n, Some(d)));
            let value = args
                .get(name)
                .map(String::as_str)
                .or(default)
                .ok_or_else(|| format!("missing template argument: {name}"))?;
            out.push_str(value);
            pos += end + 1;
            continue;
        }
        if ch == '"' && !escaped {
            quoted = !quoted;
        }
        if ch == '\\' && quoted {
            escaped = !escaped;
        } else {
            escaped = false;
        }
        out.push(ch);
        pos += ch.len_utf8();
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conditional_parameters_and_comments() {
        let a = BTreeMap::from([("NAME".into(), "世界".into())]);
        assert_eq!(
            render("x=\"$NAME$\" # $MISSING$\n", &a).unwrap(),
            "x=\"世界\" # $MISSING$\n"
        );
        assert!(!render("[[MISSING] x=$BAD$ ]", &a).unwrap().contains("BAD"));
    }
    #[test]
    fn nested_reader() {
        let a = BTreeMap::from([("A".into(), "yes".into())]);
        assert!(
            render("[[A] x=@[1+2] [[!B]y=yes]]", &a)
                .unwrap()
                .contains("x=@[1+2]")
        );
    }
    #[test]
    fn missing_is_error() {
        assert!(render("x=$NAME$", &BTreeMap::new()).is_err());
        assert!(render("[[A]x=yes", &BTreeMap::new()).is_err());
    }
    #[test]
    fn selected_relic_does_not_supply_a_different_required_parameter() {
        let args = BTreeMap::from([("SELECTED_RELIC".into(), "r_example".into())]);
        let native = "NOT={has_country_flag=gave_relic_$RELIC$}";
        assert_eq!(
            render(native, &args).unwrap_err(),
            "missing template argument: RELIC"
        );
        let args = BTreeMap::from([("RELIC".into(), "r_example".into())]);
        assert!(
            render(native, &args)
                .unwrap()
                .contains("gave_relic_r_example")
        );
    }
}
