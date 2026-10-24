// Replaces secret values in every finished tool result before OpenCode saves
// it or sends it to the model, so `cat .env` or `docker inspect` cannot leak.
const MARK = "[redacted]";

// Key names such as jwt_secret_key, GITEA_TOKEN, DB_PASS or apiKey.
const SECRET_NAME = /(?:secret|token|passw(?:or)?d|passphrase|pass(?![a-z])|pwd|jwt|salt|api[_-]?key|private[_-]?key|access[_-]?key|credential|[_.-]key)/i;
const ASSIGNMENT = /(?<![\w.-])([\w.-]+)(["']?[ \t]*[:=][ \t]*["']?)([^\s"',;}\]]+)/g;
const AUTH_HEADER = /\b((?:Bearer|Basic)[ \t]+)([\w.~+/-]{8,}=*)/gi;
const URL_PASSWORD = /(\b[a-z][a-z0-9+.-]*:\/\/[^\s:/@]+:)([^\s/@]+)(@)/gi;
const PRIVATE_KEY = /(-----BEGIN [A-Z ]*PRIVATE KEY-----)[\s\S]*?(-----END [A-Z ]*PRIVATE KEY-----)/g;

// Placeholders, templates and paths, such as ${DB_PASSWORD}, %s, {file:...} or /run/secrets/db.
function isReference(value) {
  return /^[$%{<~/.[`]/.test(value);
}

// Skip code syntax and plain names, such as cfg.Token or GITHUB_TOKEN, so agents can still edit around them.
function looksSecret(value) {
  if (value.length < 8 || isReference(value)) return false;
  // Counts and IDs, such as a max_tokens of 1234567890.
  if (/^\d+$/.test(value)) return false;
  if (/[()[\]<>?]/.test(value) || /^[\p{L}\p{M}_.!-]+$/u.test(value)) return false;
  // Versions, such as ^3.893.0 in a lockfile.
  if (/^[\^~<>=]*\d+(\.\d+)+[\w.+-]*$/.test(value)) return false;
  return !/^https?:\/\/[^@\s]*$/.test(value);
}

function redactAssignments(text) {
  let output = "";
  let cursor = 0;
  ASSIGNMENT.lastIndex = 0;
  let match;
  while ((match = ASSIGNMENT.exec(text)) !== null) {
    const [assignment, key, separator, value] = match;
    if (!SECRET_NAME.test(key) || !looksSecret(value)) {
      ASSIGNMENT.lastIndex = match.index + key.length;
      continue;
    }
    output += text.slice(cursor, match.index) + key + separator + MARK;
    cursor = match.index + assignment.length;
  }
  return output ? output + text.slice(cursor) : text;
}

function redactText(text) {
  return redactAssignments(text.replace(PRIVATE_KEY, `$1\n${MARK}\n$2`))
    .replace(AUTH_HEADER, `$1${MARK}`)
    .replace(URL_PASSWORD, (match, head, password, at) => (isReference(password) ? match : head + MARK + at));
}

// Only strings, arrays and plain objects change; images and class instances stay.
function redactValue(value) {
  if (typeof value === "string") return value.startsWith("data:") ? value : redactText(value);
  if (Array.isArray(value)) return value.map(redactValue);
  if (value !== null && typeof value === "object" && Object.getPrototypeOf(value) === Object.prototype) {
    return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, redactValue(item)]));
  }
  return value;
}

export default {
  id: "pde.secret-redaction",
  async setup(ctx) {
    await ctx.tool.hook("execute.after", (event) => {
      // OpenCode error events carry no command output; see tool.ts lines 131-149.
      if (event.status === "completed") event.result = redactValue(event.result);
    });
  },
};
