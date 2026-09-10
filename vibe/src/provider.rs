//! Parses model requests and selects configured Pi providers.

use std::{collections::BTreeSet, fmt, str::FromStr};

const PREFERRED: [&str; 2] = ["openai-codex", "github-copilot"];
const EXPLICIT_ONLY: &str = "opencode-go";

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct Provider(String);

impl Provider {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        if value.is_empty()
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(format!("invalid provider name: {value:?}"));
        }
        Ok(Self(value.to_string()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct Selector {
    provider: Provider,
    model: String,
}

impl Selector {
    pub(crate) fn provider(&self) -> &Provider {
        &self.provider
    }

    pub(crate) fn model(&self) -> &str {
        &self.model
    }
}

impl fmt::Display for Selector {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.provider.as_str(), self.model)
    }
}

impl FromStr for Selector {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (provider, model) = value
            .split_once('/')
            .ok_or_else(|| format!("model selector requires provider/model: {value:?}"))?;
        if model.is_empty()
            || model.contains('/')
            || model.bytes().any(|byte| byte.is_ascii_whitespace())
        {
            return Err(format!("invalid model name: {model:?}"));
        }
        Ok(Self {
            provider: Provider::parse(provider)?,
            model: model.to_string(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Request {
    text: String,
    provider: Option<Provider>,
    model: String,
}

impl Request {
    pub(crate) fn model(&self) -> &str {
        &self.model
    }

    pub(crate) fn parse(model: &str, provider: Option<&str>) -> Result<Self, String> {
        if model.contains('/') {
            if provider.is_some() {
                return Err("--provider cannot accompany provider/model --model".to_string());
            }
            let selector: Selector = model.parse()?;
            return Ok(Self {
                text: model.to_string(),
                provider: Some(selector.provider),
                model: selector.model,
            });
        }
        if model.is_empty() || model.bytes().any(|byte| byte.is_ascii_whitespace()) {
            return Err(format!("invalid model name: {model:?}"));
        }
        Ok(Self {
            text: model.to_string(),
            provider: provider.map(Provider::parse).transpose()?,
            model: model.to_string(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Resolved {
    requested: String,
    selector: Selector,
}

impl Resolved {
    pub(crate) fn requested(&self) -> &str {
        &self.requested
    }

    pub(crate) fn selector(&self) -> &Selector {
        &self.selector
    }
}

pub(crate) fn select(
    request: Request,
    models: &BTreeSet<Selector>,
    configured: &BTreeSet<Provider>,
) -> Result<Resolved, String> {
    let matching: BTreeSet<_> = models
        .iter()
        .filter(|selector| selector.model() == request.model)
        .filter(|selector| configured.contains(selector.provider()))
        .cloned()
        .collect();
    let selected = match &request.provider {
        Some(provider) => matching
            .iter()
            .find(|selector| selector.provider() == provider)
            .cloned(),
        None => PREFERRED
            .iter()
            .find_map(|provider| {
                matching
                    .iter()
                    .find(|selector| selector.provider().as_str() == *provider)
                    .cloned()
            })
            .or_else(|| {
                matching
                    .iter()
                    .find(|selector| selector.provider().as_str() != EXPLICIT_ONLY)
                    .cloned()
            }),
    };
    let selector = selected.ok_or_else(|| {
        format!("no configured provider supports {}", request.text)
    })?;
    Ok(Resolved {
        requested: request.text,
        selector,
    })
}

#[cfg(test)]
mod tests {
    use super::{select, Provider, Request, Selector};
    use std::{collections::BTreeSet, str::FromStr};

    fn selectors(values: &[&str]) -> BTreeSet<Selector> {
        values
            .iter()
            .map(|value| Selector::from_str(value).expect("selector"))
            .collect()
    }

    fn providers(values: &[&str]) -> BTreeSet<Provider> {
        values
            .iter()
            .map(|value| Provider::parse(value).expect("provider"))
            .collect()
    }

    #[test]
    fn parses_provider_model_requests() {
        let request = Request::parse("openai-codex/gpt-5.4", None).expect("request");
        let resolved = select(
            request,
            &selectors(&["openai-codex/gpt-5.4"]),
            &providers(&["openai-codex"]),
        )
        .expect("resolved");

        assert_eq!(resolved.requested(), "openai-codex/gpt-5.4");
        assert_eq!(resolved.selector().to_string(), "openai-codex/gpt-5.4");
    }

    #[test]
    fn prefers_configured_providers_in_order() {
        let resolved = select(
            Request::parse("gpt-5.4", None).expect("request"),
            &selectors(&[
                "opencode-go/gpt-5.4",
                "github-copilot/gpt-5.4",
                "other/gpt-5.4",
                "openai-codex/gpt-5.4",
            ]),
            &providers(&[
                "opencode-go",
                "github-copilot",
                "other",
                "openai-codex",
            ]),
        )
        .expect("resolved");

        assert_eq!(resolved.selector().to_string(), "openai-codex/gpt-5.4");
    }

    #[test]
    fn excludes_opencode_go_from_implicit_fallback() {
        let resolved = select(
            Request::parse("gpt-5.4", None).expect("request"),
            &selectors(&["opencode-go/gpt-5.4"]),
            &providers(&["opencode-go"]),
        );

        assert!(resolved.is_err());
    }

    #[test]
    fn rejects_conflicting_provider_selector() {
        let error = Request::parse("openai-codex/gpt-5.4", Some("github-copilot"))
            .expect_err("conflicting provider");

        assert_eq!(error, "--provider cannot accompany provider/model --model");
    }
}
