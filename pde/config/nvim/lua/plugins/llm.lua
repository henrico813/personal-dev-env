return {
  "huggingface/llm.nvim",
  opts = {
    backend = "openai",
    model = "devstral-small",
    url = "http://devlab:8090/v1/completions",
    api_token = nil,
    request_body = {
      max_tokens = 64,
      temperature = 0.2,
    },
    fim = {
      enabled = true,
      prefix = "<fim_prefix>",
      middle = "<fim_middle>",
      suffix = "<fim_suffix>",
    },
    tokens_to_clear = { "<|endoftext|>" },
    debounce_ms = 200,
    accept_keymap = "<Tab>",
    dismiss_keymap = "<S-Tab>",
    enable_suggestions_on_startup = true,
  },
}
