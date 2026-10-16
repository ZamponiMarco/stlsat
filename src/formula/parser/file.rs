//! Convenience reader for formula files in the resources directory.

use super::parse_formula;
use std::{fs, path::Path};

pub fn parse_stl_file(filename: &str) {
    let path = Path::new("resources").join(filename);
    println!("\n=== Parsing {filename} ===");

    match fs::read_to_string(&path) {
        Ok(content) => {
            for (line_num, line) in content.lines().enumerate() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue; // Skip empty lines and comments
                }

                println!("\nLine {}: {}", line_num + 1, line);
                match parse_formula(line) {
                    Ok((remaining, formula)) => {
                        println!("  ✓ Parsed: {formula:?}");
                        if !remaining.is_empty() {
                            println!("  ⚠ Remaining: '{remaining}'");
                        }
                    }
                    Err(e) => {
                        println!("  ✗ Parse error: {e:?}");
                    }
                }
            }
        }
        Err(e) => {
            println!("Error reading file {filename}: {e}");
        }
    }
}
