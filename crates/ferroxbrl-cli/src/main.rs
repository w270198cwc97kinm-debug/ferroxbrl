//! ferroxbrl — fast inline XBRL fact extractor.
//!
//! Usage:
//!   ferroxbrl extract <file.htm> [-o out.json] [--pretty]
//!
//! Prints a summary (fact/context/unit counts, elapsed time) to stderr and
//! the extracted facts as JSON to stdout or -o.

use std::io::Write;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" {
        eprintln!("usage: ferroxbrl extract <file.htm> [-o out.json] [--pretty]");
        std::process::exit(2);
    }
    let mut files: Vec<String> = Vec::new();
    let mut out: Option<String> = None;
    let mut pretty = false;
    let mut i = 0;
    let rest: Vec<String> = if args[0] == "extract" {
        args[1..].to_vec()
    } else {
        args.clone()
    };
    while i < rest.len() {
        match rest[i].as_str() {
            "-o" | "--output" => {
                i += 1;
                out = rest.get(i).cloned();
            }
            "--pretty" => pretty = true,
            f => files.push(f.to_string()),
        }
        i += 1;
    }
    if files.is_empty() {
        eprintln!("error: no input file");
        std::process::exit(2);
    }

    let start = Instant::now();
    let mut all = Vec::new();
    let mut total_warnings = 0usize;
    for file in &files {
        let path = std::path::Path::new(file);
        match ferroxbrl_core::extract_path(path) {
            Ok(doc) => {
                total_warnings += doc.warnings.len();
                for w in &doc.warnings {
                    eprintln!("warn[{file}]: {w}");
                }
                let elapsed = start.elapsed();
                eprintln!(
                    "{}: {} facts, {} contexts, {} units ({} warnings) in {:.1}ms",
                    file,
                    doc.facts.len(),
                    doc.contexts.len(),
                    doc.units.len(),
                    doc.warnings.len(),
                    elapsed.as_secs_f64() * 1000.0
                );
                all.push(serde_json::json!({
                    "source": file,
                    "fact_count": doc.facts.len(),
                    "context_count": doc.contexts.len(),
                    "unit_count": doc.units.len(),
                    "warnings": doc.warnings,
                    "facts": doc.flat_facts(),
                }));
            }
            Err(e) => {
                eprintln!("error[{file}]: {e}");
                std::process::exit(1);
            }
        }
    }
    let _ = total_warnings;
    let value = if all.len() == 1 {
        all.pop().unwrap()
    } else {
        serde_json::Value::Array(all)
    };
    let rendered = if pretty {
        serde_json::to_string_pretty(&value).unwrap()
    } else {
        serde_json::to_string(&value).unwrap()
    };
    match out {
        Some(path) => {
            std::fs::write(&path, rendered).unwrap_or_else(|e| {
                eprintln!("error: write {path}: {e}");
                std::process::exit(1);
            });
        }
        None => {
            let mut stdout = std::io::stdout().lock();
            let _ = stdout.write_all(rendered.as_bytes());
            let _ = stdout.write_all(b"\n");
        }
    }
}
