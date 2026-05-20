/// PDF/UA (Universal Accessibility) support via Tagged PDF.
///
/// Adds a structure tree to the PDF that maps visual content to logical
/// document structure (headings, paragraphs, lists, tables, images).

/// Post-process PDF bytes to add Tagged PDF structure.
pub fn make_tagged_pdf(pdf_bytes: &[u8], structure: &[DocStructureNode]) -> Vec<u8> {
    let Ok(mut doc) = lopdf::Document::load_mem(pdf_bytes) else {
        return pdf_bytes.to_vec();
    };

    let mut struct_kids = Vec::new();

    for node in structure {
        let elem_ref = add_struct_elem(&mut doc, node);
        struct_kids.push(lopdf::Object::Reference(elem_ref));
    }

    let struct_tree = lopdf::Dictionary::from_iter(vec![
        ("Type", lopdf::Object::Name(b"StructTreeRoot".to_vec())),
        ("K", lopdf::Object::Array(struct_kids)),
    ]);
    let struct_tree_ref = doc.add_object(lopdf::Object::Dictionary(struct_tree));

    if let Ok(catalog) = doc.catalog_mut() {
        catalog.set("StructTreeRoot", lopdf::Object::Reference(struct_tree_ref));
        catalog.set("MarkInfo", lopdf::Object::Dictionary(
            lopdf::Dictionary::from_iter(vec![
                ("Marked", lopdf::Object::Boolean(true)),
            ]),
        ));
        catalog.set("Lang", lopdf::Object::String(b"en".to_vec(), lopdf::StringFormat::Literal));
    }

    let mut output = Vec::new();
    let _ = doc.save_to(&mut output);
    output
}

fn add_struct_elem(doc: &mut lopdf::Document, node: &DocStructureNode) -> lopdf::ObjectId {
    let mut kids = Vec::new();
    for child in &node.children {
        let child_ref = add_struct_elem(doc, child);
        kids.push(lopdf::Object::Reference(child_ref));
    }

    let mut dict = lopdf::Dictionary::from_iter(vec![
        ("Type", lopdf::Object::Name(b"StructElem".to_vec())),
        ("S", lopdf::Object::Name(node.role.pdf_name().into())),
    ]);

    if !kids.is_empty() {
        dict.set("K", lopdf::Object::Array(kids));
    }

    if let Some(alt) = &node.alt_text {
        dict.set("Alt", lopdf::Object::String(alt.as_bytes().to_vec(), lopdf::StringFormat::Literal));
    }

    if let Some(lang) = &node.lang {
        dict.set("Lang", lopdf::Object::String(lang.as_bytes().to_vec(), lopdf::StringFormat::Literal));
    }

    doc.add_object(lopdf::Object::Dictionary(dict))
}

/// Logical role of a structure element.
#[derive(Debug, Clone)]
pub enum StructureRole {
    Document,
    Part,
    Heading(u8), // H1-H6
    Paragraph,
    List,
    ListItem,
    Table,
    TableRow,
    TableHeader,
    TableData,
    Figure,
    BlockQuote,
    Code,
    Span,
}

impl StructureRole {
    fn pdf_name(&self) -> &[u8] {
        if let StructureRole::Heading(n) = self {
            return heading_pdf_name(*n);
        }
        non_heading_pdf_name(self)
    }
}

fn non_heading_pdf_name(role: &StructureRole) -> &'static [u8] {
    pdf_name_group_a(role).unwrap_or_else(|| pdf_name_group_b(role))
}

fn pdf_name_group_a(role: &StructureRole) -> Option<&'static [u8]> {
    Some(match role {
        StructureRole::Document => b"Document",
        StructureRole::Part => b"Part",
        StructureRole::Paragraph => b"P",
        StructureRole::List => b"L",
        StructureRole::ListItem => b"LI",
        StructureRole::Table => b"Table",
        StructureRole::TableRow => b"TR",
        _ => return None,
    })
}

fn pdf_name_group_b(role: &StructureRole) -> &'static [u8] {
    match role {
        StructureRole::TableHeader => b"TH",
        StructureRole::TableData => b"TD",
        StructureRole::Figure => b"Figure",
        StructureRole::BlockQuote => b"BlockQuote",
        StructureRole::Code => b"Code",
        _ => b"Span",
    }
}

fn heading_pdf_name(level: u8) -> &'static [u8] {
    match level {
        1 => b"H1",
        2 => b"H2",
        3 => b"H3",
        4 => b"H4",
        5 => b"H5",
        _ => b"H6",
    }
}

/// A node in the document structure tree.
#[derive(Debug, Clone)]
pub struct DocStructureNode {
    pub role: StructureRole,
    pub alt_text: Option<String>,
    pub lang: Option<String>,
    pub children: Vec<DocStructureNode>,
}

/// Build document structure from a styled tree.
pub fn build_structure(tree: &crate::style::StyledNode) -> Vec<DocStructureNode> {
    let mut nodes = Vec::new();
    build_structure_recursive(tree, &mut nodes);
    nodes
}

fn build_structure_recursive(node: &crate::style::StyledNode, out: &mut Vec<DocStructureNode>) {
    if node.tag == "img" {
        build_img_structure(node, out);
        return;
    }

    let Some(role) = tag_to_role(&node.tag) else {
        recurse_children(node, out);
        return;
    };

    let children = collect_child_structures(node);
    out.push(DocStructureNode { role, alt_text: None, lang: None, children });
}

fn build_img_structure(node: &crate::style::StyledNode, out: &mut Vec<DocStructureNode>) {
    let alt = node.attrs.iter()
        .find(|(k, _)| k == "alt")
        .map(|(_, v)| v.clone());
    out.push(DocStructureNode {
        role: StructureRole::Figure,
        alt_text: alt,
        lang: None,
        children: Vec::new(),
    });
}

fn tag_to_role(tag: &str) -> Option<StructureRole> {
    heading_tag_to_role(tag).or_else(|| block_tag_to_role(tag))
}

fn heading_tag_to_role(tag: &str) -> Option<StructureRole> {
    let level = match tag {
        "h1" => 1, "h2" => 2, "h3" => 3, "h4" => 4, "h5" => 5, "h6" => 6,
        _ => return None,
    };
    Some(StructureRole::Heading(level))
}

fn block_tag_to_role(tag: &str) -> Option<StructureRole> {
    block_tag_group_a(tag).or_else(|| block_tag_group_b(tag))
}

fn block_tag_group_a(tag: &str) -> Option<StructureRole> {
    Some(match tag {
        "p" => StructureRole::Paragraph,
        "ul" | "ol" => StructureRole::List,
        "li" => StructureRole::ListItem,
        "table" => StructureRole::Table,
        "tr" => StructureRole::TableRow,
        _ => return None,
    })
}

fn block_tag_group_b(tag: &str) -> Option<StructureRole> {
    Some(match tag {
        "th" => StructureRole::TableHeader,
        "td" => StructureRole::TableData,
        "blockquote" => StructureRole::BlockQuote,
        "pre" | "code" => StructureRole::Code,
        "figure" => StructureRole::Figure,
        _ => return None,
    })
}

fn collect_child_structures(node: &crate::style::StyledNode) -> Vec<DocStructureNode> {
    let mut children = Vec::new();
    recurse_children(node, &mut children);
    children
}

fn recurse_children(node: &crate::style::StyledNode, out: &mut Vec<DocStructureNode>) {
    for child in &node.children {
        if let crate::style::StyledContent::Element(child_node) = child {
            build_structure_recursive(child_node, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{ComputedStyle, StyledContent, StyledNode};

    /// Helper: build a minimal StyledNode.
    fn make_node(tag: &str, children: Vec<StyledContent>) -> StyledNode {
        StyledNode {
            tag: tag.to_string(),
            id: None,
            classes: Vec::new(),
            style: ComputedStyle::default(),
            children,
            attrs: Vec::new(),
        }
    }

    fn elem(node: StyledNode) -> StyledContent {
        StyledContent::Element(node)
    }

    fn text(s: &str) -> StyledContent {
        StyledContent::Text(s.to_string())
    }

    // ─── build_structure extracts headings ─────────────

    #[test]
    fn build_structure_extracts_h1() {
        let h1 = make_node("h1", vec![text("Title")]);
        let body = make_node("body", vec![elem(h1)]);
        let nodes = build_structure(&body);
        assert_eq!(nodes.len(), 1);
        assert!(matches!(nodes[0].role, StructureRole::Heading(1)));
    }

    #[test]
    fn build_structure_extracts_multiple_headings() {
        let h1 = make_node("h1", vec![text("Title")]);
        let h2 = make_node("h2", vec![text("Section")]);
        let h3 = make_node("h3", vec![text("Subsection")]);
        let body = make_node("body", vec![elem(h1), elem(h2), elem(h3)]);
        let nodes = build_structure(&body);
        assert_eq!(nodes.len(), 3);
        assert!(matches!(nodes[0].role, StructureRole::Heading(1)));
        assert!(matches!(nodes[1].role, StructureRole::Heading(2)));
        assert!(matches!(nodes[2].role, StructureRole::Heading(3)));
    }

    // ─── build_structure extracts paragraphs ───────────

    #[test]
    fn build_structure_extracts_paragraphs() {
        let p1 = make_node("p", vec![text("First")]);
        let p2 = make_node("p", vec![text("Second")]);
        let body = make_node("body", vec![elem(p1), elem(p2)]);
        let nodes = build_structure(&body);
        assert_eq!(nodes.len(), 2);
        assert!(matches!(nodes[0].role, StructureRole::Paragraph));
        assert!(matches!(nodes[1].role, StructureRole::Paragraph));
    }

    // ─── build_structure extracts lists ────────────────

    #[test]
    fn build_structure_extracts_list() {
        let li1 = make_node("li", vec![text("Item 1")]);
        let li2 = make_node("li", vec![text("Item 2")]);
        let ul = make_node("ul", vec![elem(li1), elem(li2)]);
        let body = make_node("body", vec![elem(ul)]);
        let nodes = build_structure(&body);
        assert_eq!(nodes.len(), 1, "should have one list node");
        assert!(matches!(nodes[0].role, StructureRole::List));
        assert_eq!(nodes[0].children.len(), 2, "list should have 2 children");
        assert!(matches!(nodes[0].children[0].role, StructureRole::ListItem));
        assert!(matches!(nodes[0].children[1].role, StructureRole::ListItem));
    }

    // ─── Structure role mapping ────────────────────────

    #[test]
    fn role_mapping_h1_to_h6() {
        for level in 1u8..=6 {
            let tag = format!("h{}", level);
            let node = make_node(&tag, vec![text("Heading")]);
            let body = make_node("body", vec![elem(node)]);
            let nodes = build_structure(&body);
            assert_eq!(nodes.len(), 1);
            match &nodes[0].role {
                StructureRole::Heading(l) => assert_eq!(*l, level),
                other => panic!("expected Heading({level}), got {:?}", other),
            }
        }
    }

    #[test]
    fn role_mapping_p_to_paragraph() {
        let p = make_node("p", vec![text("text")]);
        let body = make_node("body", vec![elem(p)]);
        let nodes = build_structure(&body);
        assert!(matches!(nodes[0].role, StructureRole::Paragraph));
    }

    #[test]
    fn role_mapping_table_to_table() {
        let td = make_node("td", vec![text("cell")]);
        let tr = make_node("tr", vec![elem(td)]);
        let table = make_node("table", vec![elem(tr)]);
        let body = make_node("body", vec![elem(table)]);
        let nodes = build_structure(&body);
        assert_eq!(nodes.len(), 1);
        assert!(matches!(nodes[0].role, StructureRole::Table));
        assert!(matches!(nodes[0].children[0].role, StructureRole::TableRow));
        assert!(matches!(nodes[0].children[0].children[0].role, StructureRole::TableData));
    }

    #[test]
    fn role_mapping_blockquote_to_blockquote() {
        let bq = make_node("blockquote", vec![text("quote")]);
        let body = make_node("body", vec![elem(bq)]);
        let nodes = build_structure(&body);
        assert!(matches!(nodes[0].role, StructureRole::BlockQuote));
    }

    #[test]
    fn role_mapping_pre_to_code() {
        let pre = make_node("pre", vec![text("code")]);
        let body = make_node("body", vec![elem(pre)]);
        let nodes = build_structure(&body);
        assert!(matches!(nodes[0].role, StructureRole::Code));
    }

    #[test]
    fn role_mapping_code_to_code() {
        let code = make_node("code", vec![text("x = 1")]);
        let body = make_node("body", vec![elem(code)]);
        let nodes = build_structure(&body);
        assert!(matches!(nodes[0].role, StructureRole::Code));
    }

    #[test]
    fn role_mapping_ol_to_list() {
        let li = make_node("li", vec![text("item")]);
        let ol = make_node("ol", vec![elem(li)]);
        let body = make_node("body", vec![elem(ol)]);
        let nodes = build_structure(&body);
        assert!(matches!(nodes[0].role, StructureRole::List));
    }

    #[test]
    fn role_mapping_th_to_table_header() {
        let th = make_node("th", vec![text("header")]);
        let tr = make_node("tr", vec![elem(th)]);
        let table = make_node("table", vec![elem(tr)]);
        let body = make_node("body", vec![elem(table)]);
        let nodes = build_structure(&body);
        let row = &nodes[0].children[0];
        assert!(matches!(row.children[0].role, StructureRole::TableHeader));
    }

    // ─── PDF name mapping ──────────────────────────────

    #[test]
    fn pdf_name_heading_levels() {
        assert_eq!(StructureRole::Heading(1).pdf_name(), b"H1");
        assert_eq!(StructureRole::Heading(2).pdf_name(), b"H2");
        assert_eq!(StructureRole::Heading(3).pdf_name(), b"H3");
        assert_eq!(StructureRole::Heading(4).pdf_name(), b"H4");
        assert_eq!(StructureRole::Heading(5).pdf_name(), b"H5");
        assert_eq!(StructureRole::Heading(6).pdf_name(), b"H6");
    }

    #[test]
    fn pdf_name_common_roles() {
        assert_eq!(StructureRole::Document.pdf_name(), b"Document");
        assert_eq!(StructureRole::Paragraph.pdf_name(), b"P");
        assert_eq!(StructureRole::List.pdf_name(), b"L");
        assert_eq!(StructureRole::ListItem.pdf_name(), b"LI");
        assert_eq!(StructureRole::Table.pdf_name(), b"Table");
        assert_eq!(StructureRole::TableRow.pdf_name(), b"TR");
        assert_eq!(StructureRole::TableHeader.pdf_name(), b"TH");
        assert_eq!(StructureRole::TableData.pdf_name(), b"TD");
        assert_eq!(StructureRole::Figure.pdf_name(), b"Figure");
        assert_eq!(StructureRole::BlockQuote.pdf_name(), b"BlockQuote");
        assert_eq!(StructureRole::Code.pdf_name(), b"Code");
        assert_eq!(StructureRole::Span.pdf_name(), b"Span");
    }

    // ─── img element ───────────────────────────────────

    #[test]
    fn build_structure_img_with_alt() {
        let mut img = make_node("img", vec![]);
        img.attrs = vec![("alt".to_string(), "A photo".to_string())];
        let body = make_node("body", vec![elem(img)]);
        let nodes = build_structure(&body);
        assert_eq!(nodes.len(), 1);
        assert!(matches!(nodes[0].role, StructureRole::Figure));
        assert_eq!(nodes[0].alt_text.as_deref(), Some("A photo"));
    }

    #[test]
    fn build_structure_img_without_alt() {
        let img = make_node("img", vec![]);
        let body = make_node("body", vec![elem(img)]);
        let nodes = build_structure(&body);
        assert_eq!(nodes.len(), 1);
        assert!(matches!(nodes[0].role, StructureRole::Figure));
        assert!(nodes[0].alt_text.is_none());
    }

    // ─── Unknown tags are skipped ──────────────────────

    #[test]
    fn unknown_tag_does_not_produce_structure_node() {
        let span = make_node("span", vec![text("inline text")]);
        let body = make_node("body", vec![elem(span)]);
        let nodes = build_structure(&body);
        // <span> is not a mapped structural tag, so it should not appear
        assert!(nodes.is_empty(), "unknown tags should not create structure nodes");
    }

    // ─── Mixed content ─────────────────────────────────

    #[test]
    fn build_structure_mixed_content() {
        let h1 = make_node("h1", vec![text("Title")]);
        let p = make_node("p", vec![text("Text")]);
        let li1 = make_node("li", vec![text("One")]);
        let li2 = make_node("li", vec![text("Two")]);
        let ul = make_node("ul", vec![elem(li1), elem(li2)]);
        let body = make_node("body", vec![elem(h1), elem(p), elem(ul)]);
        let nodes = build_structure(&body);
        assert_eq!(nodes.len(), 3, "should have h1, p, and ul");
        assert!(matches!(nodes[0].role, StructureRole::Heading(1)));
        assert!(matches!(nodes[1].role, StructureRole::Paragraph));
        assert!(matches!(nodes[2].role, StructureRole::List));
    }
}
