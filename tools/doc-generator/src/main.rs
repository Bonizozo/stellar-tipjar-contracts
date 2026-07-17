//! doc-generator — parse a Soroban contract source file and emit API docs.
//!
//! Usage:
//!   cargo run -p tipjar-doc-generator -- \
//!     --input contracts/tipjar/src/lib.rs \
//!     --out-dir docs/api \
//!     --contract TipJarContract

mod parser;
mod templates;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

#[cfg(test)]
mod tests {
    use crate::{parser::parse_contract, templates};

    const SAMPLE: &str = r#"
        use soroban_sdk::{Env, Address};
        #[contracterror]
        pub enum MyError { BadAmount = 1 }
        pub struct MyContract;
        #[contractimpl]
        impl MyContract {
            /// Initialize.
            pub fn init(env: Env, admin: Address) {}
            /// Tip creator.
            pub fn tip(env: Env, sender: Address, creator: Address, amount: i128) {}
            /// Get total.
            pub fn get_total(env: Env, creator: Address) -> i128 { 0 }
        }
    "#;

    #[test]
    fn markdown_contains_function_names() {
        let doc = parse_contract(SAMPLE, "MyContract").unwrap();
        let md = templates::render_markdown(&doc);
        assert!(md.contains("### `tip`"));
        assert!(md.contains("### `init`"));
        assert!(md.contains("BadAmount"));
    }

    #[test]
    fn html_contains_search_and_functions() {
        let doc = parse_contract(SAMPLE, "MyContract").unwrap();
        let html = templates::render_html(&doc);
        assert!(html.contains(r#"id="search""#));
        assert!(html.contains("fn-tip"));
        assert!(html.contains("fn-init"));
    }

    #[test]
    fn json_roundtrip() {
        let doc = parse_contract(SAMPLE, "MyContract").unwrap();
        let json = serde_json::to_string(&doc).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["contract"], "MyContract");
        assert!(v["functions"].as_array().unwrap().len() >= 3);
    }
}

#[derive(Parser)]
#[command(about = "Generate API docs from a Soroban contract source file")]
struct Cli {
    /// Path to the contract lib.rs
    #[arg(long, default_value = "contracts/tipjar/src/lib.rs")]
    input: PathBuf,

    /// Output directory for generated docs
    #[arg(long, default_value = "docs/api")]
    out_dir: PathBuf,

    /// Contract name used in doc headings
    #[arg(long, default_value = "TipJarContract")]
    contract: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let source = std::fs::read_to_string(&cli.input)
        .with_context(|| format!("cannot read {}", cli.input.display()))?;

    let doc = parser::parse_contract(&source, &cli.contract)
        .context("failed to parse contract")?;

    std::fs::create_dir_all(&cli.out_dir)
        .with_context(|| format!("cannot create {}", cli.out_dir.display()))?;

    // Markdown
    let md = templates::render_markdown(&doc);
    let md_path = cli.out_dir.join("README.md");
    std::fs::write(&md_path, &md)?;
    eprintln!("✅ Markdown → {}", md_path.display());

    // HTML
    let html = templates::render_html(&doc);
    let html_path = cli.out_dir.join("index.html");
    std::fs::write(&html_path, &html)?;
    eprintln!("✅ HTML     → {}", html_path.display());

    // JSON (machine-readable, useful for search indexing / CI)
    let json = serde_json::to_string_pretty(&doc)?;
    let json_path = cli.out_dir.join("api.json");
    std::fs::write(&json_path, &json)?;
    eprintln!("✅ JSON     → {}", json_path.display());

    eprintln!(
        "\nDocumented {} functions, {} error variants.",
        doc.functions.len(),
        doc.errors.len()
    );

    Ok(())
}
