import { readFile } from "node:fs/promises";

export const LOG_LEVELS = ["debug", "info", "warn", "error"] as const;
export type LogLevel = (typeof LOG_LEVELS)[number];

export interface Config {
  readonly port: number;
  readonly logLevel: LogLevel;
}

const DEFAULT_PORT = 8080;
const DEFAULT_LOG_LEVEL: LogLevel = "info";

export interface ConfigErrorOptions extends ErrorOptions {
  readonly issues?: readonly string[];
}

/**
 * Thrown when a config cannot be read, parsed, or validated. `issues`
 * lists each invalid field, and `cause` keeps the lower-level error.
 */
export class ConfigError extends Error {
  override name = "ConfigError";
  readonly issues: readonly string[];

  constructor(message: string, options: ConfigErrorOptions = {}) {
    const { issues = [], ...errorOptions } = options;
    super(message, errorOptions);
    this.issues = issues;
  }
}

/**
 * Checks untrusted config data and fills defaults for missing fields.
 * Throws a `ConfigError` that lists every invalid field, not only the
 * first one.
 */
export function parseConfig(input: unknown): Config {
  if (typeof input !== "object" || input === null || Array.isArray(input)) {
    throw new ConfigError("Config must be a JSON object");
  }
  const port = "port" in input ? input.port : DEFAULT_PORT;
  const logLevel = "logLevel" in input ? input.logLevel : DEFAULT_LOG_LEVEL;
  if (isPort(port) && isLogLevel(logLevel)) {
    return { port, logLevel };
  }

  const issues: string[] = [];
  if (!isPort(port)) {
    issues.push("port must be an integer from 1 to 65535");
  }
  if (!isLogLevel(logLevel)) {
    issues.push(`logLevel must be one of ${LOG_LEVELS.join(", ")}`);
  }
  throw new ConfigError("Config has invalid fields", { issues });
}

/** Reads a JSON config file and checks it with `parseConfig`. */
export async function loadConfig(path: string): Promise<Config> {
  let text: string;
  try {
    text = await readFile(path, "utf8");
  } catch (error) {
    throw new ConfigError(`Cannot read config ${path}`, { cause: error });
  }

  let data: unknown;
  try {
    data = JSON.parse(text);
  } catch (error) {
    throw new ConfigError(`Config ${path} is not valid JSON`, {
      cause: error,
    });
  }
  return parseConfig(data);
}

function isPort(value: unknown): value is number {
  return (
    typeof value === "number" &&
    Number.isInteger(value) &&
    value >= 1 &&
    value <= 65_535
  );
}

function isLogLevel(value: unknown): value is LogLevel {
  // includes() would need a cast because its parameter type is LogLevel.
  return LOG_LEVELS.some((level) => level === value);
}
