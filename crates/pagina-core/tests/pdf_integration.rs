/// Integration tests for the full HTML-to-PDF conversion pipeline.
/// These exercise `pagina_core::convert()` end-to-end with real font
/// rendering (builtin Helvetica/Courier) and verify the PDF output.

// ─── Basic output ─────────────────────────────────────

#[test]
fn convert_returns_non_empty_bytes() {
    let pdf = pagina_core::convert("<html><body><p>Hello</p></body></html>");
    assert!(!pdf.is_empty(), "convert should return non-empty output");
}

#[test]
fn output_starts_with_pdf_header() {
    let pdf = pagina_core::convert("<html><body><p>Hello</p></body></html>");
    assert!(
        pdf.starts_with(b"%PDF"),
        "PDF output should start with %PDF magic bytes, got: {:?}",
        &pdf[..pdf.len().min(10)]
    );
}

#[test]
fn basic_html_produces_valid_pdf_structure() {
    let pdf = pagina_core::convert("
        <html><body>
        <h1>Title</h1>
        <p>A paragraph of text.</p>
        </body></html>
    ");
    // A valid PDF must contain %%EOF at or near the end
    let pdf_str = String::from_utf8_lossy(&pdf);
    assert!(pdf_str.contains("%%EOF"), "PDF should contain %%EOF marker");
    // And should contain obj/endobj pairs
    assert!(pdf_str.contains("endobj"), "PDF should contain object definitions");
}

// ─── CSS @page size ───────────────────────────────────

#[test]
fn page_size_affects_output() {
    let pdf_a4 = pagina_core::convert("
        <html><head><style>@page { size: a4; }</style></head>
        <body><p>A4</p></body></html>
    ");
    let pdf_letter = pagina_core::convert("
        <html><head><style>@page { size: letter; }</style></head>
        <body><p>Letter</p></body></html>
    ");
    // Both should produce valid PDF
    assert!(pdf_a4.starts_with(b"%PDF"));
    assert!(pdf_letter.starts_with(b"%PDF"));
    // They should differ because page dimensions differ
    // (A4=210x297mm vs Letter=215.9x279.4mm -> different MediaBox)
    assert_ne!(pdf_a4, pdf_letter, "different page sizes should produce different output");
}

// ─── Multiple pages ───────────────────────────────────

#[test]
fn multiple_pages_produce_larger_output() {
    let single = pagina_core::convert("
        <html><body><p>One page</p></body></html>
    ");
    let multi = pagina_core::convert("
        <html><head><style>
        .bp { break-before: page; }
        </style></head><body>
        <p>Page one</p>
        <p class=\"bp\">Page two</p>
        <p class=\"bp\">Page three</p>
        </body></html>
    ");
    assert!(single.starts_with(b"%PDF"));
    assert!(multi.starts_with(b"%PDF"));
    assert!(
        multi.len() > single.len(),
        "3-page PDF ({} bytes) should be larger than 1-page PDF ({} bytes)",
        multi.len(),
        single.len()
    );
}
