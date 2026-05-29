//! Minimal flag parser supporting `--key value`, `--key=value`, and bare `--flag`.

pub struct ParsedArgs {
    /// (key, optional value) in order; repeated keys are preserved.
    pub items: Vec<(String, Option<String>)>,
    pub help: bool,
}

impl ParsedArgs {
    /// First value provided for `key`, if any.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.items
            .iter()
            .find(|(k, v)| k == key && v.is_some())
            .and_then(|(_, v)| v.as_deref())
    }

    /// True if the flag appeared at all (valued or bare).
    pub fn has(&self, key: &str) -> bool {
        self.items.iter().any(|(k, _)| k == key)
    }
}

pub fn parse(raw: &[String]) -> ParsedArgs {
    let mut items = Vec::new();
    let mut help = false;
    let mut i = 0;
    while i < raw.len() {
        let a = &raw[i];
        if a == "-h" || a == "--help" {
            help = true;
            i += 1;
            continue;
        }
        if let Some(rest) = a.strip_prefix("--") {
            if let Some((k, v)) = rest.split_once('=') {
                items.push((k.to_string(), Some(v.to_string())));
            } else if i + 1 < raw.len() && !raw[i + 1].starts_with('-') {
                items.push((rest.to_string(), Some(raw[i + 1].clone())));
                i += 1;
            } else {
                items.push((rest.to_string(), None));
            }
        } else if let Some(rest) = a.strip_prefix('-') {
            items.push((rest.to_string(), None));
        } else {
            items.push((String::new(), Some(a.clone())));
        }
        i += 1;
    }
    ParsedArgs { items, help }
}
