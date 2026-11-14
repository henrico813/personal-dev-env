# Choose a model and its credentials

Pass the model to `vibe run` as `--model provider/model`. The provider
part decides which credentials Vibe forwards, as
[Runtime inputs](../reference/runtime-inputs.md#provider-credentials)
lists.

## OpenAI models

Use `openai-codex/<model>`, such as `openai-codex/gpt-5.6-luna`. With no
`OPENAI_API_KEY` set, Vibe uses the login of Pi, the coding agent that
runs inside Vibe, in `~/.pi/agent/auth.json`. Do not send OpenAI models
through OpenRouter; OpenRouter bills its own credits, and a run stops
when they run out.

## Models only OpenRouter serves

Use OpenRouter for a model no other provider here serves, such as
DeepSeek at `openrouter/~deepseek/deepseek-flash-latest`. Vibe reads
`OPENROUTER_API_KEY`, which the shell does not set. Export it from
OpenCode's login file without printing it:

```sh
export OPENROUTER_API_KEY="$(jq -r .openrouter.key \
  ~/.local/share/opencode/auth.json)"
```

## When a run stops for missing credentials

Set the credentials as this page says and run the same command again.
This needs no question to the user. Never print a credential's value.
