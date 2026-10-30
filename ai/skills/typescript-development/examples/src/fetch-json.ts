/** Thrown when the server answers with a status outside 200 to 299. */
export class HttpError extends Error {
  override name = "HttpError";
  readonly status: number;

  constructor(url: string, status: number) {
    super(`GET ${url} failed with status ${status}`);
    this.status = status;
  }
}

export interface FetchJsonOptions {
  /** Defaults to 10 seconds. */
  readonly timeoutMs?: number;
  readonly signal?: AbortSignal;
}

/**
 * Fetches a URL and returns its parsed JSON body for the caller to
 * validate. Rejects with an `HttpError` for a failed status, a
 * `DOMException` named `TimeoutError` after `timeoutMs`, or the
 * caller's abort reason. Network and JSON errors pass through unchanged.
 */
export async function fetchJson(
  url: string | URL,
  options: FetchJsonOptions = {},
): Promise<unknown> {
  const { timeoutMs = 10_000, signal } = options;
  const timeout = AbortSignal.timeout(timeoutMs);
  const response = await fetch(url, {
    signal: signal ? AbortSignal.any([signal, timeout]) : timeout,
  });
  if (!response.ok) {
    // Cancel the unread body so it does not hold the connection.
    await response.body?.cancel();
    throw new HttpError(String(url), response.status);
  }
  return response.json();
}
