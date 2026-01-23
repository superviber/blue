//! Blue's Voice - How Blue speaks
//!
//! Tone rules and message formatting.

/// Format a message in Blue's voice
///
/// Blue's manner:
/// - No exclamation marks in errors
/// - Errors suggest next action
/// - No apologies for system behavior
/// - Maximum 2 sentences before action
/// - Questions at end, inviting dialogue
/// - No hedging phrases
pub fn speak(message: &str) -> String {
    // For now, pass through. Future: lint and transform.
    message.to_string()
}

/// Format an error message in Blue's voice
pub fn error(what_happened: &str, suggestion: &str) -> String {
    format!("{}. {}", what_happened, suggestion)
}

/// Format a success message in Blue's voice
pub fn success(what_happened: &str, next_step: Option<&str>) -> String {
    match next_step {
        Some(next) => format!("{}. {}", what_happened, next),
        None => what_happened.to_string(),
    }
}

/// Format a question in Blue's voice
pub fn ask(context: &str, question: &str) -> String {
    format!("{}. {}?", context, question)
}

/// The welcome message
pub fn welcome() -> &'static str {
    r#"Welcome home.

I'm Blue. Pleasure to meet you properly.

You've been you the whole time, you know.
Just took a bit to remember.

Shall we get started?"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_suggests_action() {
        let msg = error("Can't find that RFC", "Check the title's spelled right?");
        assert!(!msg.contains('!'));
        assert!(msg.contains('?'));
    }

    #[test]
    fn success_is_concise() {
        let msg = success("Marked 'implement auth' as done", Some("4 of 7 tasks complete now"));
        assert!(!msg.contains("Successfully"));
        assert!(!msg.contains('!'));
    }
}
