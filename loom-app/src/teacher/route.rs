//! What the VIBE box does with its text, like the browser: a text starting
//! with `?` is a question for the teacher, anything else is a VIBE request.
//! A question first goes to the offline knowledge base (`prof::lookup`);
//! only when it knows nothing is Claude asked (`prof::ask`).

use crate::prof::{self, Lookup};

/// Where the VIBE box's text goes.
#[derive(Debug, Clone, PartialEq)]
pub enum Route {
    /// Nothing typed.
    Empty,
    /// Words for VIBE (Claude rewrites the code).
    Vibe(String),
    /// `?…`: a question for the teacher, without the `?`.
    Question(String),
}

/// Sort the typed text.
pub fn route(input: &str) -> Route {
    let text = input.trim();
    match text.strip_prefix('?') {
        _ if text.is_empty() => Route::Empty,
        Some(question) => Route::Question(question.trim().to_string()),
        None => Route::Vibe(text.to_string()),
    }
}

/// How a question gets answered.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// The offline teacher knows it (maybe with an example to play).
    Known(Lookup),
    /// Unknown offline: ask Claude this.
    AskClaude(String),
}

/// The lookup → ask rule.
pub fn answer(question: &str) -> Answer {
    match prof::lookup(question) {
        Some(found) => Answer::Known(found),
        None => Answer::AskClaude(question.trim().to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn question_mark_means_teacher() {
        assert_eq!(route("  "), Route::Empty);
        assert_eq!(route(" ?quinte "), Route::Question("quinte".into()));
        assert_eq!(route("? comment rendre ça plus triste"), Route::Question("comment rendre ça plus triste".into()));
        assert_eq!(route("?"), Route::Question(String::new()));
        assert_eq!(route("fais un groove afro"), Route::Vibe("fais un groove afro".into()));
        assert_eq!(route("plus sombre ?"), Route::Vibe("plus sombre ?".into()), "only a leading ? is a question");
    }

    #[test]
    fn lookup_first_then_claude() {
        match answer("quinte") {
            Answer::Known(found) => {
                assert!(!found.lines.is_empty());
                assert!(found.demo.is_some(), "an interval can be heard");
            }
            other => panic!("quinte is known offline: {other:?}"),
        }
        assert!(matches!(answer("raga"), Answer::Known(_)));
        // an empty question shows the help offline
        assert!(matches!(answer(""), Answer::Known(Lookup { demo: None, .. })));
        assert_eq!(answer("pourquoi le ciel est bleu zzz"), Answer::AskClaude("pourquoi le ciel est bleu zzz".into()));
    }
}
