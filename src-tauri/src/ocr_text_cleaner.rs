//! Conservative, local text cleanup. Whole-line menu noise is removed; identifiers are retained.
use std::collections::HashSet;
pub const MAX_CLEAN_CHARACTERS: usize = 4096;

pub fn clean_ocr_text(raw: &str) -> String {
    let mut seen = HashSet::new();
    let mut output = String::new();
    let mut remaining = MAX_CLEAN_CHARACTERS;
    for line in raw.lines() {
        let without_controls: String = line
            .chars()
            .filter(|c| {
                !matches!(c, '\u{200b}' | '\u{feff}') && (!c.is_control() || c.is_whitespace())
            })
            .collect();
        let line = without_controls
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if !line.chars().any(|c| c.is_alphanumeric())
            || matches!(
                line.to_ascii_lowercase().as_str(),
                "file" | "edit" | "view" | "help" | "tools" | "window"
            )
            || !seen.insert(line.clone())
        {
            continue;
        }
        if !output.is_empty() {
            if remaining < 2 {
                break;
            }
            output.push('\n');
            remaining -= 1;
        }
        let part: String = line.chars().take(remaining).collect();
        remaining -= part.chars().count();
        output.push_str(&part);
        if remaining == 0 {
            break;
        }
    }
    output.trim_end().to_owned()
}
