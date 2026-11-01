//! This module holds the kinds of User Feedback and picks the comment text
//! each Answered Request writes. Read the `choose_new_text(request, answer)`
//! function to understand the module.

use serde::{Deserialize, Serialize};

use super::comments::lay_out_plain_words;
use super::review_session::{FeedbackRequest, SuggestionKind};

/// Returns the comment's new text for the answer, or `None` to cut it. New
/// words are laid out like the comment, and words with nothing in them cut it.
pub fn choose_new_text(request: &FeedbackRequest, answer: &UserAnswer) -> Option<String> {
    let words = match answer {
        UserAnswer::Keep => return Some(request.text.clone()),
        UserAnswer::Cut => return None,
        UserAnswer::Accept => match request
            .suggestion
            .as_ref()
            .map(|suggestion| &suggestion.kind)
        {
            // With no Agent Suggestion to take, the comment stays as it is.
            None | Some(SuggestionKind::Keep) => return Some(request.text.clone()),
            Some(SuggestionKind::Cut) => return None,
            Some(SuggestionKind::Reword { text }) => text,
        },
        UserAnswer::Edited { text } => text,
    };
    let new_text = lay_out_plain_words(&request.file, words, &request.text);
    (!new_text.is_empty()).then_some(new_text)
}

/// Holds either an answer or a User Note for one Feedback Request.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum UserFeedback {
    Answer(UserAnswer),
    Note(UserNote),
}

/// Says what to do with the comment.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum UserAnswer {
    Keep,
    Cut,
    /// Takes the Agent Suggestion.
    Accept,
    /// Uses the user's own wording.
    Edited {
        text: String,
    },
}

/// Asks the agent to try again instead of answering.
#[derive(Debug, Serialize, Deserialize)]
pub struct UserNote(pub String);

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    /// Builds a Feedback Request for a Rust doc comment, with `suggestion` as its Agent Suggestion, or none when it is null.
    fn make_request(suggestion: Value) -> FeedbackRequest {
        let suggestion = if suggestion.is_null() {
            Value::Null
        } else {
            json!({"kind": suggestion, "reasoning": "", "also_at": []})
        };
        let request = json!({
            "id": 1,
            "file": "src/lib.rs",
            "name": "save",
            "reason": "new",
            "text": "    /// Saves it.\n",
            "can_reword": true,
            "line": 3,
            "suggestion": suggestion,
        });
        serde_json::from_value(request).expect("the request matches its saved shape")
    }

    /// `choose_new_text` must return the text the user chose to keep, cut, accept or edit, laid out as a comment.
    /// Otherwise an Answered Request could write words into the code that the user never chose.
    #[test]
    fn choose_new_text_writes_only_chosen_words() {
        let edited = |text: &str| UserAnswer::Edited {
            text: text.to_owned(),
        };
        let reword = json!({"reword": {"text": "Saves the answer."}});
        let cases = [
            (
                "keep",
                json!("cut"),
                UserAnswer::Keep,
                Some("    /// Saves it.\n"),
            ),
            ("cut", json!("keep"), UserAnswer::Cut, None),
            (
                "accept keep",
                json!("keep"),
                UserAnswer::Accept,
                Some("    /// Saves it.\n"),
            ),
            ("accept cut", json!("cut"), UserAnswer::Accept, None),
            (
                "accept with no suggestion",
                Value::Null,
                UserAnswer::Accept,
                Some("    /// Saves it.\n"),
            ),
            (
                "accept reword",
                reword.clone(),
                UserAnswer::Accept,
                Some("    /// Saves the answer.\n"),
            ),
            (
                "edited",
                reword.clone(),
                edited("Saves it\nat once."),
                Some("    /// Saves it at once.\n"),
            ),
            ("edited to nothing", reword, edited(" \n"), None),
        ];

        for (case, suggestion, answer, expected) in cases {
            let new_text = choose_new_text(&make_request(suggestion), &answer);

            assert_eq!(new_text.as_deref(), expected, "{case}");
        }
    }
}
