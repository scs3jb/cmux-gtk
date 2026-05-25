//! `cmux config` subcommand implementations (all local — no socket required).

use std::path::PathBuf;

const DOCS_URL: &str =
    "https://github.com/douglas/cmux-gtk/blob/main/README.md";

/// Return the path to the cmux settings.json file.
fn settings_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("cmux")
        .join("settings.json")
}

/// `cmux config path` — print the config file path.
pub fn run_path() -> anyhow::Result<()> {
    println!("{}", settings_path().display());
    Ok(())
}

/// `cmux config docs` — print the documentation URL.
pub fn run_docs() -> anyhow::Result<()> {
    println!("Documentation: {DOCS_URL}");
    Ok(())
}

/// `cmux config doctor` — validate the config file.
pub fn run_doctor() -> anyhow::Result<()> {
    let path = settings_path();

    // Check 1: file existence
    if !path.exists() {
        println!(
            "No config file found at {}. Using defaults.",
            path.display()
        );
        println!("✓ Config OK");
        return Ok(());
    }

    // Check 2: readable + valid JSON
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            println!("Error reading {}: {e}", path.display());
            std::process::exit(1);
        }
    };

    let value: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            println!("Invalid JSON in {}: {e}", path.display());
            std::process::exit(1);
        }
    };

    // Check 3: value range validation on known numeric fields
    let mut issues: Vec<String> = Vec::new();

    if let Some(size) = value
        .get("tab_bar_font_size")
        .and_then(|v| v.as_f64())
    {
        // 0.0 means "use system default" — only validate if explicitly set
        if size != 0.0 && !(6.0..=100.0).contains(&size) {
            issues.push(format!(
                "tab_bar_font_size = {size} is out of range (expected 0 for default, or 6–100)"
            ));
        }
    }

    if issues.is_empty() {
        println!("✓ Config OK ({})", path.display());
    } else {
        for issue in &issues {
            println!("  - {issue}");
        }
        std::process::exit(1);
    }

    Ok(())
}
