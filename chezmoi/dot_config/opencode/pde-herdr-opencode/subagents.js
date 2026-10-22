const RECHECK_INTERVAL_MS = 250;
const WAKE_GRACE_MS = 1000;
const TURN_END_EVENTS = new Set(["session.execution.succeeded", "session.execution.interrupted"]);

// Herdr's V2 plugin marks the pane idle when the root session's turn ends,
// even while background subagents of that root still run. This returns a view
// of the TUI api in which such a root still counts as running: its status reads
// "running", and its turn-end event is held back until no subagent runs.
// Other events pass through, so a subagent's permission or form still blocks.
// Tests pass a fake clock and timer as the second argument.
export function keepWorkingForSubagents(
  api,
  { now = Date.now, setInterval: every = setInterval, clearInterval: stopEvery = clearInterval } = {},
) {
  const sessions = api.data.session;

  function hasRunningSubagents(id) {
    const info = sessions.get(id);
    return !!info && !info.parentID &&
      sessions.family(id).some((member) => member !== id && sessions.status(member) === "running");
  }

  function status(id) {
    return hasRunningSubagents(id) ? "running" : sessions.status(id);
  }

  function listen(callback) {
    const heldTurnEnds = new Map();
    function releaseFinished() {
      for (const [id, held] of heldTurnEnds) {
        if (hasRunningSubagents(id)) {
          held.quietSince = undefined;
        } else if (held.quietSince === undefined) {
          held.quietSince = now();
        } else if (now() - held.quietSince >= WAKE_GRACE_MS) {
          // OpenCode starts a wake-up turn on the root soon after a subagent
          // finishes. The grace lets that turn drop the end instead of the
          // pane flashing idle in between.
          heldTurnEnds.delete(id);
          callback(held.message);
        }
      }
    }
    const unsubscribe = api.data.listen((message) => {
      const event = message?.details;
      const id = event?.data?.sessionID;
      if (typeof event?.type === "string" && event.type.startsWith("session.execution.")) {
        // A later start or failure replaces a held end; sending the end
        // afterwards would turn the pane idle under a running turn.
        heldTurnEnds.delete(id);
        if (TURN_END_EVENTS.has(event.type) && hasRunningSubagents(id)) {
          heldTurnEnds.set(id, { message });
          return;
        }
      }
      callback(message);
      releaseFinished();
    });
    // The grace has to run out with no event, and a reconnect reloads session
    // data without sending events.
    const recheck = every(releaseFinished, RECHECK_INTERVAL_MS);
    return () => {
      stopEvery(recheck);
      heldTurnEnds.clear();
      unsubscribe();
    };
  }

  const session = Object.create(sessions, { status: { value: status } });
  const data = Object.create(api.data, { session: { value: session }, listen: { value: listen } });
  return Object.create(api, { data: { value: data } });
}
