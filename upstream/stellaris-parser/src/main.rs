use std::io::{self, BufRead, Write};
use serde_json::{Value, json};
fn main() {
    let mut out = io::BufWriter::new(io::stdout().lock());
    for line in io::stdin().lock().lines() {
        let result = (|| -> Result<Value, Box<dyn std::error::Error>> {
            let req: Value = serde_json::from_str(&line?)?;
            let path = req["path"].as_str().ok_or("missing path")?;
            let content = match req["content"].as_str() { Some(s) => s.to_owned(), None => std::fs::read_to_string(path)? };
            Ok(stellaris_parser::stellaris_bridge::parse(path, content, req["globals"].as_str().unwrap_or("")))
        })();
        let value = result.unwrap_or_else(|e|json!({"fatal":e.to_string()}));
        writeln!(out,"{value}").expect("stdout");
        out.flush().expect("stdout flush");
    }
}
