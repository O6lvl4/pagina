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
