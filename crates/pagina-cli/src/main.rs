use std::fs;
use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
#[command(name = "pagina", about = "HTML + CSS Paged Media -> PDF")]
struct Cli {
    /// Input HTML file
    input: PathBuf,

    /// Output PDF file (default: <input>.pdf)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// External font files (TTF/OTF) to load
    #[arg(long = "font")]
    fonts: Vec<PathBuf>,

    /// Generate PDF/A-1b conformant output
    #[arg(long)]
    pdfa: bool,

    /// Document title (for PDF/A metadata)
    #[arg(long, default_value = "")]
    title: String,

    /// Document author (for PDF/A metadata)
    #[arg(long, default_value = "")]
    author: String,

    /// Generate Tagged PDF (PDF/UA accessibility)
    #[arg(long)]
    tagged: bool,
}

/// Write a message to stderr without using eprintln! macro.
fn stderr_msg(msg: &str) {
    use std::io::Write;
    let _ = writeln!(std::io::stderr(), "{msg}");
}

fn main() {
    let cli = Cli::parse();

    let html = fs::read_to_string(&cli.input).unwrap_or_else(|e| {
        stderr_msg(&format!("Error reading {}: {e}", cli.input.display()));
        std::process::exit(1);
    });

    let font_paths: Vec<String> = cli.fonts.iter().map(|p| p.display().to_string()).collect();
    let font_refs: Vec<&str> = font_paths.iter().map(|s| s.as_str()).collect();

    let pdfa_opts = if cli.pdfa {
        Some(pagina_core::pdfa::PdfAOptions {
            title: cli.title.clone(),
            author: cli.author.clone(),
            ..Default::default()
        })
    } else {
        None
    };

    let opts = pagina_core::ConvertOptions {
        font_paths: &font_refs,
        pdfa: pdfa_opts,
        tagged: cli.tagged,
    };

    let pdf_bytes = pagina_core::convert_with_options(&html, &opts);

    let output = cli.output.unwrap_or_else(|| cli.input.with_extension("pdf"));
    fs::write(&output, &pdf_bytes).unwrap_or_else(|e| {
        stderr_msg(&format!("Error writing {}: {e}", output.display()));
        std::process::exit(1);
    });

    stderr_msg(&format!("wrote {}", output.display()));
}
