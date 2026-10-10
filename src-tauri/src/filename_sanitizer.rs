use std::{error::Error, fmt};

#[derive(Debug, Eq, PartialEq)]
pub enum SanitizeError {
    EmptyStem,
    ReservedDeviceName,
}

impl SanitizeError {
    pub fn reason(&self) -> &'static str {
        match self {
            Self::EmptyStem => "the name is empty after removing unsupported characters",
            Self::ReservedDeviceName => "the name is a reserved Windows device name",
        }
    }
}

impl fmt::Display for SanitizeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.reason())
    }
}

impl Error for SanitizeError {}

/// Cleans a generated filename stem for use on Windows without changing its extension.
pub fn sanitize_stem(candidate: &str) -> Result<String, SanitizeError> {
    const INVALID: &str = "<>:\"/\\|?*";

    let has_usable_character = candidate.chars().any(|character| {
        !character.is_whitespace()
            && character != '.'
            && !character.is_control()
            && !INVALID.contains(character)
    });
    let sanitized = candidate
        .trim()
        .chars()
        .map(|character| {
            if character.is_control() || INVALID.contains(character) {
                '-'
            } else {
                character
            }
        })
        .collect::<String>()
        .trim_end_matches(|character| character == ' ' || character == '.')
        .to_owned();

    if sanitized.is_empty() || !has_usable_character {
        return Err(SanitizeError::EmptyStem);
    }
    if is_reserved_windows_name(&sanitized) {
        return Err(SanitizeError::ReservedDeviceName);
    }

    Ok(sanitized)
}

fn is_reserved_windows_name(stem: &str) -> bool {
    let first_part = stem
        .split('.')
        .next()
        .unwrap_or(stem)
        .trim_end_matches(|character| character == ' ' || character == '.');
    let uppercase = first_part.to_ascii_uppercase();

    matches!(
        uppercase.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|prefix| {
        uppercase.strip_prefix(prefix).is_some_and(|digit| {
            matches!(
                digit,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{sanitize_stem, SanitizeError};

    #[test]
    fn replaces_windows_illegal_characters_and_controls() {
        for (candidate, expected) in [
            ("a<b>c:d\"e/f\\g|h?i*j", "a-b-c-d-e-f-g-h-i-j"),
            ("line\nfeed\u{0000}nul", "line-feed-nul"),
        ] {
            assert_eq!(sanitize_stem(candidate), Ok(expected.to_owned()));
        }
    }

    #[test]
    fn trims_outer_whitespace_and_trailing_periods_but_preserves_unicode() {
        for (candidate, expected) in [
            ("  report...  ", "report"),
            (" résumé 日本語 ", "résumé 日本語"),
            ("project.settings", "project.settings"),
        ] {
            assert_eq!(sanitize_stem(candidate), Ok(expected.to_owned()));
        }
    }

    #[test]
    fn rejects_names_without_a_usable_stem() {
        for candidate in ["", "   ", "...", " . . ", "<>:?"] {
            assert_eq!(sanitize_stem(candidate), Err(SanitizeError::EmptyStem));
        }
    }

    #[test]
    fn rejects_reserved_device_names_even_with_case_or_an_embedded_extension() {
        for candidate in [
            "con",
            "NUL.txt",
            "PrN",
            "aUx.log",
            "COM1",
            "lpt9.txt",
            "COM¹",
            "LPT³",
            "conin$",
            "CONOUT$.log",
        ] {
            assert_eq!(
                sanitize_stem(candidate),
                Err(SanitizeError::ReservedDeviceName),
                "{candidate}"
            );
        }
    }
}
