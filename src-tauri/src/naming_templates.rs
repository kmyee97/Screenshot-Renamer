//! Tokens: app, topic, detail, timestamp (capture time in Unix milliseconds).
//! Missing tokens render empty; the engine collapses separators and uses a timestamp fallback.
//! Double braces escape literal braces. All output goes through the engine's shared length/safety policy.
use crate::{
    naming_engine::{tokens, NamingError},
    screenshot_metadata::ScreenshotMetadata,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NamingPreset {
    #[default]
    Descriptive,
    Minimal,
    Coding,
    WorkResearch,
    ReceiptsOrders,
}
impl NamingPreset {
    pub fn template(self) -> &'static str {
        match self {
            Self::Descriptive => "{app}-{topic}-{detail}",
            Self::Minimal => "{topic}",
            Self::Coding => "{app}-{topic}",
            Self::WorkResearch => "{app}-{topic}-{timestamp}",
            Self::ReceiptsOrders => "{topic}-{detail}-{timestamp}",
        }
    }
}
enum Segment {
    Literal(String),
    Token(String),
}
fn parse(template: &str) -> Result<Vec<Segment>, NamingError> {
    if template.trim().is_empty() || template.chars().count() > 512 {
        return Err(NamingError(
            "Template must contain 1–512 characters.".into(),
        ));
    }
    let mut chars = template.chars().peekable();
    let mut segments = Vec::new();
    let mut literal = String::new();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                literal.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                literal.push('}');
            }
            '{' => {
                if !literal.is_empty() {
                    segments.push(Segment::Literal(std::mem::take(&mut literal)));
                }
                let mut token = String::new();
                let mut closed = false;
                for c in chars.by_ref() {
                    if c == '}' {
                        closed = true;
                        break;
                    }
                    token.push(c);
                }
                if !closed {
                    return Err(NamingError(
                        "Unclosed template token. Use {app}, {topic}, {detail}, or {timestamp}."
                            .into(),
                    ));
                }
                if !["app", "topic", "detail", "timestamp"].contains(&token.as_str()) {
                    return Err(NamingError(format!(
                        "Unknown token: {token}. Use app, topic, detail, or timestamp."
                    )));
                }
                segments.push(Segment::Token(token));
            }
            '}' => {
                return Err(NamingError(
                    "Unexpected closing brace. Use }} for a literal brace.".into(),
                ))
            }
            c => literal.push(c),
        }
    }
    if !literal.is_empty() {
        segments.push(Segment::Literal(literal));
    }
    Ok(segments)
}
pub fn validate_template(template: &str) -> Result<(), NamingError> {
    parse(template).map(|_| ())
}
pub(crate) fn render(template: &str, metadata: &ScreenshotMetadata) -> Result<String, NamingError> {
    let values = tokens(metadata);
    let mut rendered = String::new();
    for segment in parse(template)? {
        match segment {
            Segment::Literal(text) => rendered.push_str(&text),
            Segment::Token(token) => rendered.push_str(match token.as_str() {
                "app" => &values.app,
                "topic" => &values.topic,
                "detail" => &values.detail,
                "timestamp" => &values.timestamp,
                _ => unreachable!(),
            }),
        }
    }
    Ok(rendered)
}

pub fn preview(rules: &crate::naming_engine::NamingRules) -> Result<String, NamingError> {
    use crate::{screenshot_metadata::WindowContext, ScreenshotFile};
    let file = ScreenshotFile::new(
        "preview.png".into(),
        Some(std::time::UNIX_EPOCH + std::time::Duration::from_millis(1791720000000)),
    )
    .expect("static preview path");
    let mut metadata = ScreenshotMetadata::from_file(&file);
    metadata.window_context = Some(WindowContext {
        app_name: "Code".into(),
        window_title: Some("main.rs".into()),
    });
    metadata.set_ocr_text(
        "main.rs TypeError import E0432".into(),
        "main.rs TypeError import E0432".into(),
    );
    crate::naming_engine::name(&metadata, rules).map(|stem| format!("{stem}.png"))
}
