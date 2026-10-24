import assert from "node:assert/strict";
import test from "node:test";
import plugin from "../../chezmoi/dot_config/opencode/plugins/pde-secret-redaction.js";

// Runs the plugin's hook on one finished shell result and returns the text
// OpenCode would save and send to the model.
async function redactShellOutput(text) {
  let hook;
  await plugin.setup({
    tool: {
      hook: async (name, callback) => {
        assert.equal(name, "execute.after");
        hook = callback;
      },
    },
  });
  const event = { tool: "shell", status: "completed", result: { content: [{ type: "text", text }] } };
  await hook(event);
  return event.result.content[0].text;
}

// Fake secrets are built from pieces, so this file stays readable when an
// agent reads it through the plugin.
const fake = (label) => `fake-${label}-` + "0123456789";
const BEGIN_KEY = "-----BEGIN OPENSSH PRIVATE " + "KEY-----";
const END_KEY = "-----END OPENSSH PRIVATE " + "KEY-----";

test("hides secret values in tool output", async (t) => {
  const cases = [
    ["yaml", "  jwt_secret_key: " + fake("jwt"), "  jwt_secret_key: [redacted]"],
    ["json", `  "api_key": "${fake("sk")}",`, `  "api_key": "[redacted]",`],
    ["env", "GITEA_TOKEN=" + fake("token"), "GITEA_TOKEN=[redacted]"],
    ["docker inspect", `      "POSTGRES_PASSWORD=${fake("pass")}",`, `      "POSTGRES_PASSWORD=[redacted]",`],
    ["read tool line", "12: DB_PASS=" + fake("pass"), "12: DB_PASS=[redacted]"],
    ["bearer header", "> Authorization: Bearer " + fake("bearer"), "> Authorization: Bearer [redacted]"],
    ["basic header", "> Authorization: Basic " + "dXNlcjpm" + "YWtlLXBhc3M=", "> Authorization: Basic [redacted]"],
    ["URL password", "url: postgres://stash:" + fake("pw") + "@db/stash", "url: postgres://stash:[redacted]@db/stash"],
    ["private key", `${BEGIN_KEY}\nb3BlbnNzaC1rZXktdjE\n${END_KEY}`, `${BEGIN_KEY}\n[redacted]\n${END_KEY}`],
  ];
  for (const [name, input, want] of cases) {
    await t.test(name, async () => {
      assert.equal(await redactShellOutput(input), want);
    });
  }
});

// An agent edits by matching text it read, so code must come back unchanged.
test("keeps code, placeholders and paths", async (t) => {
  const cases = [
    ["field access", "token: cfg.Token,"],
    ["environment lookup", `api_key = os.environ["API_KEY"]`],
    ["shell variable", "password: ${DB_PASSWORD}"],
    ["file reference", `"apiKey": "{file:~/.config/opencode/goog.key}"`],
    ["secret path", "secret_file: /run/secrets/stash_jwt"],
    ["format string", `fmt.Sprintf("postgres://%s:%s@%s", user, pass, host)`],
    ["variable name", "token_env: GITHUB_TOKEN"],
    ["token count", "max_tokens=1234567890"],
    ["lockfile version", `"@aws-sdk/token-providers": "^3.893.0",`],
  ];
  for (const [name, input] of cases) {
    await t.test(name, async () => {
      assert.equal(await redactShellOutput(input), input);
    });
  }
});

test("redacts large output within one second", async () => {
  const text = "plainwordtext".repeat(161_319) + " api_key: " + fake("large");

  const started = performance.now();
  const output = await redactShellOutput(text);
  const elapsed = performance.now() - started;

  assert.ok(output.endsWith("api_key: [redacted]"));
  assert.ok(elapsed < 1_000, `redaction took ${elapsed.toFixed(1)} ms`);
});
