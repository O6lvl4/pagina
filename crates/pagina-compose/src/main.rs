use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser;
use serde::Deserialize;

#[derive(Parser)]
#[command(name = "pagina-compose", about = "Compose documents from clause templates")]
struct Cli {
    /// Document definition file (TOML)
    config: PathBuf,

    /// Output PDF file
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Template directory (default: ./templates/contract)
    #[arg(long, default_value = "templates/contract")]
    templates: PathBuf,

    /// External font files
    #[arg(long = "font")]
    fonts: Vec<PathBuf>,
}

// ─── Document definition schema ──────────────────────

#[derive(Deserialize)]
struct DocumentDef {
    document: DocumentMeta,
    parties: HashMap<String, Party>,
    #[serde(default)]
    clauses: Vec<ClauseDef>,
    #[serde(default)]
    signature: Option<SignatureDef>,
}

#[derive(Deserialize)]
struct DocumentMeta {
    title: String,
    #[serde(default)]
    date: String,
    #[serde(default = "default_style")]
    style: String,
}

fn default_style() -> String {
    "default".to_string()
}

#[derive(Deserialize)]
struct Party {
    name: String,
    #[serde(default)]
    address: String,
    #[serde(default)]
    representative: String,
}

#[derive(Deserialize)]
struct ClauseDef {
    #[serde(default)]
    title: String,
    #[serde(default)]
    template: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    params: HashMap<String, String>,
}

#[derive(Deserialize)]
struct SignatureDef {
    #[serde(default = "default_sig_style")]
    style: String,
}

fn default_sig_style() -> String {
    "seal".to_string()
}

// ─── Assembly ────────────────────────────────────────

fn assemble_html(def: &DocumentDef, templates_dir: &Path) -> String {
    let mut html = String::new();

    // Load CSS
    let css = load_style(&def.document.style, templates_dir);

    html.push_str("<!DOCTYPE html>\n<html>\n<head>\n<style>\n");
    html.push_str(&css);
    html.push_str("\n</style>\n</head>\n<body>\n");

    // Header
    html.push_str(&build_header(def));

    // Preamble
    html.push_str(&build_preamble(def));

    // Clauses
    for (i, clause) in def.clauses.iter().enumerate() {
        html.push_str(&build_clause(clause, i + 1, templates_dir, &def.parties));
    }

    // Signature
    if let Some(sig) = &def.signature {
        html.push_str(&build_signature(def, sig));
    }

    html.push_str("\n</body>\n</html>");
    html
}

fn load_style(style_name: &str, templates_dir: &Path) -> String {
    let path = templates_dir.join("styles").join(format!("{style_name}.css"));
    fs::read_to_string(&path).unwrap_or_else(|_| default_contract_css().to_string())
}

fn load_clause_template(template_name: &str, templates_dir: &Path) -> Option<String> {
    let path = templates_dir.join("clauses").join(format!("{template_name}.html"));
    fs::read_to_string(&path).ok()
}

fn build_header(def: &DocumentDef) -> String {
    format!(
        "<h1>{}</h1>\n",
        def.document.title,
    )
}

fn build_preamble(def: &DocumentDef) -> String {
    let party_names: Vec<String> = def.parties.iter()
        .map(|(role, p)| format!("{}（以下「{}」という）", p.name, role))
        .collect();

    let date_line = if def.document.date.is_empty() {
        String::new()
    } else {
        format!("<p class=\"date\">{}</p>\n", def.document.date)
    };

    format!(
        "{}<p class=\"preamble\">{}は、以下のとおり契約を締結する。</p>\n",
        date_line,
        party_names.join("と"),
    )
}

fn build_clause(
    clause: &ClauseDef,
    number: usize,
    templates_dir: &Path,
    parties: &HashMap<String, Party>,
) -> String {
    let title = if clause.title.is_empty() {
        format!("第{}条", number)
    } else {
        format!("第{}条（{}）", number, clause.title)
    };

    let body = if !clause.template.is_empty() {
        load_clause_template(&clause.template, templates_dir)
            .unwrap_or_else(|| clause.body.clone())
    } else {
        clause.body.clone()
    };

    // Replace {{param}} placeholders
    let body = replace_params(&body, &clause.params, parties);

    format!(
        "<div class=\"clause\">\n<h3>{title}</h3>\n{body}\n</div>\n",
    )
}

fn replace_params(
    template: &str,
    params: &HashMap<String, String>,
    parties: &HashMap<String, Party>,
) -> String {
    let mut result = template.to_string();

    // Replace {{key}} with value
    for (key, value) in params {
        result = result.replace(&format!("{{{{{key}}}}}"), value);
    }

    // Replace {{party.role.field}} patterns
    for (role, party) in parties {
        result = result.replace(&format!("{{{{{role}.name}}}}"), &party.name);
        result = result.replace(&format!("{{{{{role}.address}}}}"), &party.address);
        result = result.replace(&format!("{{{{{role}.representative}}}}"), &party.representative);
    }

    // Replace {{key|default}} patterns (use default if key not found)
    while let Some(start) = result.find("{{") {
        let rest = &result[start + 2..];
        let Some(end) = rest.find("}}") else { break };
        let token = &rest[..end];

        let replacement = if let Some((key, default)) = token.split_once('|') {
            params.get(key).map(|s| s.as_str()).unwrap_or(default)
        } else {
            params.get(token).map(|s| s.as_str()).unwrap_or(token)
        };

        result = format!(
            "{}{}{}",
            &result[..start],
            replacement,
            &result[start + 2 + end + 2..],
        );
    }

    result
}

fn build_signature(def: &DocumentDef, sig: &SignatureDef) -> String {
    let num_parties = def.parties.len();
    let mut html = String::from("<div class=\"sig-area\">\n");

    html.push_str(&format!(
        "<p class=\"sig-closing\">以上、本契約の成立を証するため、本書{}通を作成し、各自記名押印の上、各1通を保有する。</p>\n",
        num_parties
    ));

    if !def.document.date.is_empty() {
        html.push_str(&format!("<p class=\"sig-date\">{}</p>\n", def.document.date));
    }

    // Collect parties in stable order (sorted by key)
    let mut parties: Vec<(&String, &Party)> = def.parties.iter().collect();
    parties.sort_by(|(a, _), (b, _)| a.cmp(b));

    // Full-width spaces for alignment (consistent width with CJK font)
    let indent = "\u{3000}\u{3000}"; // 2 full-width spaces

    for (role, party) in &parties {
        html.push_str("<div class=\"sig-party\">\n");

        // Address line: role + 住所 + value
        if !party.address.is_empty() {
            html.push_str(&format!(
                "<p>{}\u{3000}住所\u{3000}{}</p>\n",
                role, party.address
            ));
        } else {
            html.push_str(&format!("<p>{}</p>\n", role));
        }

        // Name line with seal
        if sig.style == "seal" {
            html.push_str(&format!(
                "<p>{indent}氏名\u{3000}{}</p>\n",
                party.name
            ));
            html.push_str(&format!(
                "<p class=\"sig-seal\">{indent}\u{3000}\u{3000}\u{3000}\u{3000}\u{3000}\u{3000}\u{3000}\u{3000}\u{3000}\u{3000}\u{3000}\u{3000}\u{3000}\u{3000}\u{3000}\u{3000}印</p>\n"
            ));
        } else {
            html.push_str(&format!(
                "<p>{indent}氏名\u{3000}{}</p>\n",
                party.name
            ));
            html.push_str(&format!(
                "<p>{indent}\u{3000}\u{3000}\u{3000}署名 ________________________</p>\n"
            ));
        }

        // Representative
        if !party.representative.is_empty() {
            html.push_str(&format!(
                "<p>{indent}\u{3000}\u{3000}\u{3000}{}</p>\n",
                party.representative
            ));
        }

        html.push_str("</div>\n");
    }

    html.push_str("</div>\n");
    html
}

fn default_contract_css() -> &'static str {
    r#"
@page {
    size: A4;
    margin: 25mm 20mm 30mm 20mm;
    @top-center {
        content: string(doc-title);
        font-size: 8pt;
        color: #888;
    }
    @bottom-center {
        content: counter(page) " / " counter(pages);
        font-size: 8pt;
        color: #888;
    }
}
@page :first {
    @top-center { content: none; }
}

body { font-size: 10.5pt; line-height: 1.8; color: #222; font-family: "NotoSansCJKjp-Regular", Helvetica; }

h1 {
    font-size: 18pt;
    text-align: center;
    margin-top: 20mm;
    margin-bottom: 8mm;
    string-set: doc-title content();
}

.date { text-align: right; margin-bottom: 5mm; }
.preamble { margin-bottom: 8mm; }

.clause { margin-bottom: 5mm; }
.clause h3 { font-size: 11pt; margin-bottom: 2mm; }
.clause p { margin-bottom: 2mm; }
.clause ol, .clause ul { margin-bottom: 2mm; }

.sig-area { break-before: page; }
.sig-closing { margin-bottom: 12mm; }
.sig-date { text-align: right; margin-bottom: 18mm; }
.sig-party { margin-bottom: 16mm; }
.sig-seal { color: #c00; }
"#
}

// ─── Font auto-download ──────────────────────────────

#[derive(Deserialize)]
struct FontsConfig {
    #[serde(default)]
    fonts: Vec<FontEntry>,
}

#[derive(Deserialize)]
struct FontEntry {
    #[allow(dead_code)]
    family: String,
    url: String,
    file: String,
}

fn font_cache_dir() -> PathBuf {
    let dir = dirs_cache().join("pagina").join("fonts");
    let _ = fs::create_dir_all(&dir);
    dir
}

fn dirs_cache() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        return PathBuf::from(xdg);
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".cache");
    }
    PathBuf::from("/tmp")
}

fn ensure_fonts(templates_dir: &Path) -> Vec<PathBuf> {
    let fonts_toml = templates_dir.join("fonts.toml");
    let Ok(text) = fs::read_to_string(&fonts_toml) else {
        return Vec::new();
    };
    let Ok(config) = toml::from_str::<FontsConfig>(&text) else {
        return Vec::new();
    };

    let cache = font_cache_dir();
    let mut paths = Vec::new();

    for entry in &config.fonts {
        let path = cache.join(&entry.file);
        if !path.exists() {
            eprintln!("Downloading font {}...", entry.file);
            if download_file(&entry.url, &path) {
                eprintln!("  -> {}", path.display());
            } else {
                eprintln!("  -> FAILED");
                continue;
            }
        }
        paths.push(path);
    }

    paths
}

fn download_file(url: &str, dest: &Path) -> bool {
    let status = std::process::Command::new("curl")
        .args(["-fsSL", "-o"])
        .arg(dest)
        .arg(url)
        .status();
    matches!(status, Ok(s) if s.success())
}

// ─── Main ────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();

    let config_text = fs::read_to_string(&cli.config).unwrap_or_else(|e| {
        eprintln!("Error reading {}: {e}", cli.config.display());
        std::process::exit(1);
    });

    let def: DocumentDef = toml::from_str(&config_text).unwrap_or_else(|e| {
        eprintln!("Error parsing {}: {e}", cli.config.display());
        std::process::exit(1);
    });

    let html = assemble_html(&def, &cli.templates);

    // Resolve fonts: CLI flags + auto-downloaded from fonts.toml
    let auto_fonts = ensure_fonts(&cli.templates);
    let mut all_font_paths: Vec<String> = auto_fonts.iter()
        .map(|p| p.display().to_string())
        .collect();
    for p in &cli.fonts {
        all_font_paths.push(p.display().to_string());
    }
    let font_refs: Vec<&str> = all_font_paths.iter().map(|s| s.as_str()).collect();

    let pdf_bytes = pagina_core::convert_with_fonts(&html, &font_refs);

    let output = cli.output.unwrap_or_else(|| {
        cli.config.with_extension("pdf")
    });

    fs::write(&output, &pdf_bytes).unwrap_or_else(|e| {
        eprintln!("Error writing {}: {e}", output.display());
        std::process::exit(1);
    });

    eprintln!("wrote {}", output.display());
}
