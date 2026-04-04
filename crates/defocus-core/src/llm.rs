/// Trait for LLM completion providers.
/// Implementations handle the actual API call to an LLM service.
pub trait LlmProvider: Send + Sync {
    fn complete(&self, prompt: &str) -> Result<String, String>;
}

/// A mock LLM provider for testing. Returns canned responses based on prompt content.
pub struct MockProvider {
    /// List of (needle, response) pairs. The first needle found in the prompt
    /// determines the response. If no needle matches, `default` is returned.
    pub responses: Vec<(String, String)>,
    pub default: String,
}

impl MockProvider {
    pub fn new(default: impl Into<String>) -> Self {
        MockProvider {
            responses: Vec::new(),
            default: default.into(),
        }
    }

    pub fn with_response(
        mut self,
        needle: impl Into<String>,
        response: impl Into<String>,
    ) -> Self {
        self.responses.push((needle.into(), response.into()));
        self
    }
}

impl LlmProvider for MockProvider {
    fn complete(&self, prompt: &str) -> Result<String, String> {
        for (needle, response) in &self.responses {
            if prompt.contains(needle.as_str()) {
                return Ok(response.clone());
            }
        }
        Ok(self.default.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_provider_default() {
        let provider = MockProvider::new("I don't know.");
        assert_eq!(provider.complete("anything").unwrap(), "I don't know.");
    }

    #[test]
    fn test_mock_provider_matched() {
        let provider = MockProvider::new("default")
            .with_response("hello", "Hi there!")
            .with_response("bye", "Goodbye!");
        assert_eq!(
            provider.complete("say hello to me").unwrap(),
            "Hi there!"
        );
        assert_eq!(
            provider.complete("time to say bye").unwrap(),
            "Goodbye!"
        );
        assert_eq!(
            provider.complete("something else").unwrap(),
            "default"
        );
    }
}
