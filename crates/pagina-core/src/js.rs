/// JavaScript execution via Boa engine.
///
/// Executes `<script>` blocks before layout, allowing dynamic content generation.
/// The DOM is exposed as a simplified `document` object.

use boa_engine::{Context, Source};
use markup5ever_rcdom::{Handle, NodeData, RcDom};

/// Execute all `<script>` elements in the DOM.
/// Returns a list of text insertions that scripts requested via `document.write()`.
pub fn execute_scripts(dom: &RcDom) -> Vec<String> {
    let scripts = extract_scripts(&dom.document);
    if scripts.is_empty() {
        return Vec::new();
    }

    let mut context = Context::default();
    let mut writes: Vec<String> = Vec::new();

    register_document_api(&mut context, &mut writes);

    for script in &scripts {
        let _ = context.eval(Source::from_bytes(script.as_bytes()));
    }

    writes
}

/// Extract text content from all `<script>` elements.
fn extract_scripts(handle: &Handle) -> Vec<String> {
    let mut scripts = Vec::new();
    collect_scripts(handle, &mut scripts);
    scripts
}

fn collect_scripts(handle: &Handle, scripts: &mut Vec<String>) {
    if is_script_element(handle) {
        let js = extract_element_text(handle);
        if !js.is_empty() {
            scripts.push(js);
        }
        return;
    }
    for child in handle.children.borrow().iter() {
        collect_scripts(child, scripts);
    }
}

fn is_script_element(handle: &Handle) -> bool {
    matches!(&handle.data, NodeData::Element { name, .. } if name.local.as_ref() == "script")
}

fn extract_element_text(handle: &Handle) -> String {
    let mut text = String::new();
    for child in handle.children.borrow().iter() {
        if let NodeData::Text { contents } = &child.data {
            text.push_str(&contents.borrow());
        }
    }
    text
}

fn register_document_api(context: &mut Context, _writes: &mut Vec<String>) {
    let script = r#"
        var document = {
            title: '',
            _writes: [],
            write: function(s) { this._writes.push(String(s)); },
            writeln: function(s) { this._writes.push(String(s) + '\n'); },
            getElementById: function(id) { return null; },
            querySelector: function(sel) { return null; },
        };
        var window = { document: document };
        var console = {
            log: function() {},
            warn: function() {},
            error: function() {},
        };
    "#;

    let _ = context.eval(Source::from_bytes(script.as_bytes()));
}

/// After executing scripts, extract any `document.write()` output.
pub fn extract_document_writes(context: &mut Context) -> Vec<String> {
    let Ok(val) = context.eval(Source::from_bytes(b"JSON.stringify(document._writes)")) else {
        return Vec::new();
    };
    let s = val.to_string(context)
        .ok()
        .map(|js| js.to_std_string_escaped())
        .unwrap_or_default();
    parse_json_string_array(&s)
}

fn parse_json_string_array(s: &str) -> Vec<String> {
    let s = s.trim();
    if !s.starts_with('[') || !s.ends_with(']') {
        return Vec::new();
    }
    let inner = &s[1..s.len() - 1];
    if inner.is_empty() {
        return Vec::new();
    }
    tokenize_json_strings(inner)
}

/// State machine for JSON string tokenization.
struct JsonTokenizer {
    items: Vec<String>,
    current: String,
    in_string: bool,
    escape: bool,
}

impl JsonTokenizer {
    fn new() -> Self {
        Self { items: Vec::new(), current: String::new(), in_string: false, escape: false }
    }

    fn feed(&mut self, ch: char) {
        if self.escape {
            self.current.push(ch);
            self.escape = false;
            return;
        }
        if ch == '\\' && self.in_string {
            self.escape = true;
            return;
        }
        if ch == '"' {
            self.in_string = !self.in_string;
            return;
        }
        if ch == ',' && !self.in_string {
            self.items.push(std::mem::take(&mut self.current));
            return;
        }
        if self.in_string {
            self.current.push(ch);
        }
    }

    fn finish(mut self) -> Vec<String> {
        if !self.current.is_empty() {
            self.items.push(self.current);
        }
        self.items
    }
}

fn tokenize_json_strings(input: &str) -> Vec<String> {
    let mut tokenizer = JsonTokenizer::new();
    for ch in input.chars() {
        tokenizer.feed(ch);
    }
    tokenizer.finish()
}

/// High-level: execute scripts and return generated HTML fragments.
pub fn run_scripts(dom: &RcDom) -> Vec<String> {
    let scripts = extract_scripts(&dom.document);
    if scripts.is_empty() {
        return Vec::new();
    }

    let mut context = Context::default();
    register_document_api(&mut context, &mut Vec::new());

    for script in &scripts {
        let _ = context.eval(Source::from_bytes(script.as_bytes()));
    }

    extract_document_writes(&mut context)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::parse_html;

    #[test]
    fn empty_dom_returns_empty_writes() {
        let dom = parse_html("<html><body></body></html>");
        let writes = run_scripts(&dom);
        assert!(writes.is_empty(), "no scripts should produce no writes");
    }

    #[test]
    fn no_script_tag_returns_empty() {
        let dom = parse_html("<html><body><p>Hello</p></body></html>");
        let writes = run_scripts(&dom);
        assert!(writes.is_empty());
    }

    #[test]
    fn document_write_returns_written_text() {
        let dom = parse_html(r#"<html><body><script>document.write('hello')</script></body></html>"#);
        let writes = run_scripts(&dom);
        assert_eq!(writes, vec!["hello"]);
    }

    #[test]
    fn multiple_scripts_concatenate() {
        let dom = parse_html(r#"<html><body>
            <script>document.write('aaa')</script>
            <script>document.write('bbb')</script>
        </body></html>"#);
        let writes = run_scripts(&dom);
        assert_eq!(writes.len(), 2);
        assert_eq!(writes[0], "aaa");
        assert_eq!(writes[1], "bbb");
    }

    #[test]
    fn document_write_with_html_tags() {
        let dom = parse_html(r#"<html><body><script>document.write('<p>Generated</p>')</script></body></html>"#);
        let writes = run_scripts(&dom);
        assert_eq!(writes.len(), 1);
        assert!(writes[0].contains("<p>Generated</p>"));
    }

    #[test]
    fn empty_script_tag_returns_empty() {
        let dom = parse_html("<html><body><script></script></body></html>");
        let writes = run_scripts(&dom);
        assert!(writes.is_empty(), "empty script should produce no writes");
    }

    #[test]
    fn script_without_document_write_returns_empty() {
        let dom = parse_html(r#"<html><body><script>var x = 42;</script></body></html>"#);
        let writes = run_scripts(&dom);
        assert!(writes.is_empty(), "script with no document.write should produce no output");
    }

    #[test]
    fn parse_json_string_array_empty() {
        assert!(parse_json_string_array("[]").is_empty());
    }

    #[test]
    fn parse_json_string_array_single() {
        let result = parse_json_string_array(r#"["hello"]"#);
        assert_eq!(result, vec!["hello"]);
    }

    #[test]
    fn parse_json_string_array_multiple() {
        let result = parse_json_string_array(r#"["a","b","c"]"#);
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn parse_json_string_array_invalid_input() {
        assert!(parse_json_string_array("not json").is_empty());
        assert!(parse_json_string_array("").is_empty());
    }
}
