//! Holds the kinds of User Feedback, an answer or a User Note.

use serde::{Deserialize, Serialize};

use super::comments::lay_out_plain_words;
use super::review_session::{FeedbackRequest, SuggestionKind};

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
            // Step 5 refuses a request with no suggestion, so this keeps the comment.
            None | Some(SuggestionKind::Keep) => return Some(request.text.clone()),
            Some(SuggestionKind::Cut) => return None,
            Some(SuggestionKind::Reword { text }) => text,
        },
        UserAnswer::Edited { text } => text,
    };
    let new_text = lay_out_plain_words(&request.file, words, &request.text);
    (!new_text.is_empty()).then_some(new_text)
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    fn make_request(suggestion: Value) -> FeedbackRequest {
        let request = json!({
            "id": 1,
            "file": "src/lib.rs",
            "name": "save",
            "reason": "new",
            "text": "    /// Saves it.\n",
            "can_reword": true,
            "line": 3,
            "suggestion": {"kind": suggestion, "reasoning": "", "also_at": []},
        });
        serde_json::from_value(request).expect("the request matches its saved shape")
    }

    /// Without it, an answer could write words the user never chose.
    #[test]
    fn new_text_follows_each_answer() {
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
