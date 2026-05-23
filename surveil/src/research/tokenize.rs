use std::collections::HashSet;

pub(super) fn create_search_tokens(terms: &[String], question: &str) -> Vec<String> {
    let question_tokens = create_question_tokens(question);
    let question_token_set: HashSet<String> = question_tokens
        .iter()
        .flat_map(|token| create_token_variants(token))
        .collect();

    let matching_terms: Vec<String> = terms
        .iter()
        .filter_map(|term| {
            let token = term.trim().to_lowercase();
            if token.is_empty() {
                return None;
            }
            let variants = create_token_variants(&token);
            if variants
                .iter()
                .any(|variant| question_token_set.contains(variant))
            {
                Some(token)
            } else {
                None
            }
        })
        .collect();

    let source = if matching_terms.is_empty() {
        question_tokens
    } else {
        matching_terms
    };

    let mut tokens = Vec::new();
    for token in source {
        push_token(&mut tokens, &token);
        if token.contains('-') {
            push_token(&mut tokens, &token.replace('-', "_"));
        } else if token.contains('_') {
            push_token(&mut tokens, &token.replace('_', "-"));
        }
    }

    tokens
}

fn create_question_tokens(question: &str) -> Vec<String> {
    question
        .split_whitespace()
        .filter_map(|raw| {
            let token = raw
                .trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-' && ch != '_')
                .to_lowercase();
            if token.is_empty() || check_generic_question_token(&token) {
                None
            } else {
                Some(token)
            }
        })
        .collect()
}

fn create_token_variants(token: &str) -> Vec<String> {
    let mut variants = vec![token.to_string()];

    if token.contains('-') {
        let variant = token.replace('-', "_");
        if variant != token {
            variants.push(variant);
        }
    } else if token.contains('_') {
        let variant = token.replace('_', "-");
        if variant != token {
            variants.push(variant);
        }
    }

    variants
}

fn push_token(tokens: &mut Vec<String>, token: &str) {
    if token.len() < 3 {
        return;
    }
    if !tokens.iter().any(|existing| existing == token) {
        tokens.push(token.to_string());
    }
}

fn check_generic_question_token(token: &str) -> bool {
    matches!(
        token,
        "what"
            | "where"
            | "when"
            | "why"
            | "how"
            | "who"
            | "whom"
            | "which"
            | "whose"
            | "should"
            | "would"
            | "could"
            | "can"
            | "may"
            | "might"
            | "do"
            | "does"
            | "did"
            | "is"
            | "are"
            | "was"
            | "were"
            | "be"
            | "been"
            | "being"
            | "the"
            | "a"
            | "an"
            | "to"
            | "of"
            | "and"
            | "or"
            | "for"
            | "in"
            | "on"
            | "at"
            | "by"
            | "with"
            | "from"
            | "into"
            | "this"
            | "that"
            | "these"
            | "those"
    )
}

#[cfg(test)]
mod tests {
    use super::create_search_tokens;

    struct TokenCase {
        name: &'static str,
        terms: Vec<&'static str>,
        question: &'static str,
        expected_tokens: Vec<&'static str>,
    }

    #[test]
    fn token_case_tables() {
        let cases = vec![
            TokenCase {
                name: "declared-term-wins",
                terms: vec!["tree-sitter", "attach"],
                question: "Where should Tree-sitter attach?",
                expected_tokens: vec!["tree-sitter", "tree_sitter", "attach"],
            },
            TokenCase {
                name: "matching-term-order-stays-stable",
                terms: vec!["attach", "tree-sitter"],
                question: "Where should Tree-sitter attach?",
                expected_tokens: vec!["attach", "tree-sitter", "tree_sitter"],
            },
            TokenCase {
                name: "fallback-question-token",
                terms: vec!["build"],
                question: "Where should Tree-sitter attach?",
                expected_tokens: vec!["tree-sitter", "tree_sitter", "attach"],
            },
        ];

        for case in &cases {
            let tokens = create_search_tokens(
                &case.terms.iter().map(|term| term.to_string()).collect::<Vec<_>>(),
                case.question,
            );
            assert_eq!(tokens, case.expected_tokens, "case: {}", case.name);
        }
    }
}
