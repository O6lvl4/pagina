use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};

pub fn parse_html(html: &str) -> RcDom {
    parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut html.as_bytes())
        .expect("HTML parsing failed")
}

/// Extract CSS text from all `<style>` elements.
pub fn extract_styles(handle: &Handle) -> Vec<String> {
    let mut out = Vec::new();
    collect_styles(handle, &mut out);
    out
}

fn collect_styles(handle: &Handle, out: &mut Vec<String>) {
    if is_element_with_tag(handle, "style") {
        let css = collect_child_text(handle);
        if !css.is_empty() {
            out.push(css);
        }
        return;
    }
    for child in handle.children.borrow().iter() {
        collect_styles(child, out);
    }
}

fn is_element_with_tag(handle: &Handle, tag: &str) -> bool {
    matches!(&handle.data, NodeData::Element { name, .. } if name.local.as_ref() == tag)
}

fn collect_child_text(handle: &Handle) -> String {
    let mut text = String::new();
    for child in handle.children.borrow().iter() {
        if let NodeData::Text { ref contents } = child.data {
            text.push_str(&contents.borrow());
        }
    }
    text
}

#[derive(Debug)]
pub struct TextBlock {
    pub tag: String,
    pub text: String,
}

/// Extract block-level text from the DOM body.
pub fn extract_text_blocks(handle: &Handle) -> Vec<TextBlock> {
    let mut out = Vec::new();
    collect_blocks(handle, &mut out);
    out
}

/// Tags that should be treated as leaf blocks (inline content collected).
const LEAF_BLOCK_TAGS: &[&str] = &[
    "p", "h1", "h2", "h3", "h4", "h5", "h6", "li", "pre", "blockquote",
    "figcaption", "td", "th", "dt", "dd",
];

/// Tags to skip entirely during block collection.
const SKIP_TAGS: &[&str] = &["style", "script", "head", "link", "meta", "title"];

fn collect_blocks(handle: &Handle, out: &mut Vec<TextBlock>) {
    let NodeData::Element { ref name, .. } = handle.data else {
        for child in handle.children.borrow().iter() {
            collect_blocks(child, out);
        }
        return;
    };

    let tag = name.local.as_ref();

    if SKIP_TAGS.contains(&tag) {
        return;
    }

    if LEAF_BLOCK_TAGS.contains(&tag) {
        collect_leaf_block(handle, tag, out);
    } else {
        for child in handle.children.borrow().iter() {
            collect_blocks(child, out);
        }
    }
}

fn collect_leaf_block(handle: &Handle, tag: &str, out: &mut Vec<TextBlock>) {
    let mut text = String::new();
    collect_inline_text(handle, &mut text);
    let text = text.trim().to_string();
    if !text.is_empty() {
        out.push(TextBlock {
            tag: tag.to_string(),
            text,
        });
    }
}

fn collect_inline_text(handle: &Handle, buf: &mut String) {
    match handle.data {
        NodeData::Text { ref contents } => buf.push_str(&contents.borrow()),
        NodeData::Element { ref name, .. } => {
            if name.local.as_ref() == "br" {
                buf.push('\n');
            }
            collect_inline_children(handle, buf);
        }
        _ => collect_inline_children(handle, buf),
    }
}

fn collect_inline_children(handle: &Handle, buf: &mut String) {
    for child in handle.children.borrow().iter() {
        collect_inline_text(child, buf);
    }
}
