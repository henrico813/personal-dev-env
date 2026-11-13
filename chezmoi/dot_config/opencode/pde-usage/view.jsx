// Draws usage for one OpenCode session in a short line above the prompt, in a
// subagent's own row, in a sidebar panel and on the screen /usage opens, and
// adds a notice line above the prompt when a subagent finishes. The lines are
// read as the JSX renders, so they follow the run while a screen is open.
// OpenCode compiles this JSX itself, and the plugin entry in tui.js calls
// addUsageSlots once at startup.
import { createSignal } from "solid-js";
import {
  describeFinishGroup,
  describeFinishNotice,
  describeGroupLines,
  describeShortLine,
  describeSubagentLines,
  listSubagents,
  priceSubagents,
  readJsonFile,
} from "./usage.js";

const NOTICE_MS = 10_000;

/** Adds the usage slots, the /usage screen and the finish notices to OpenCode. */
export function addUsageSlots(api, { catalogModels, planFile, lineMode }) {
  const [notices, setNotices] = createSignal([]);
  const sessions = api.data.session;
  // OpenCode's Context panel reads model limits from the same list. Its
  // entries come first, so the catalog only fills in missing prices.
  const listModels = (sessionID) => [
    ...(api.data.location.model.list(sessions.get(sessionID)?.location) ?? []),
    ...catalogModels(),
  ];
  const withMessages = (sessionID) => {
    const info = sessions.get(sessionID);
    return (
      info && {
        ...info,
        messages: sessions.message.list(sessionID) ?? [],
        running: sessions.status(sessionID) === "running",
      }
    );
  };
  const priceFor = (sessionID) =>
    priceSubagents(listSubagents(sessions, sessionID).map(withMessages).filter(Boolean), listModels(sessionID));

  // The sidebar, screen and finish notices keep the last root session, because a plugin
  // screen has no session of its own to ask.
  let rootID;
  const rootOf = (startID) => {
    let sessionID = startID;
    for (let info = sessions.get(sessionID); info?.parentID; info = sessions.get(info.parentID)) {
      sessionID = info.parentID;
    }
    return sessionID;
  };
  const rememberRoot = () => {
    const current = api.ui.router.current();
    if (current?.type === "session" && current.sessionID) rootID = rootOf(current.sessionID);
    return rootID;
  };
  const goBack = () => {
    if (rootID) api.ui.router.navigate({ type: "session", sessionID: rootID });
  };

  // A parent shows finish notices and the short line, a subagent its own row.
  const composerLines = (sessionID) => {
    const info = sessionID ? sessions.get(sessionID) : undefined;
    if (!info) return [];
    const infoColor = api.theme.text.feedback.info.base;
    if (info.parentID) {
      // Pricing only this subagent keeps its lines matched to its own session.
      const self = priceSubagents([withMessages(sessionID)], listModels(sessionID));
      return describeSubagentLines(self.rows[0]).map((text) => ({ text, fg: infoColor }));
    }
    const line = describeShortLine(priceFor(sessionID), lineMode);
    // Many subagents can finish at once, so all recent finishes share one line.
    const finished = notices();
    const notice =
      finished.length === 0
        ? []
        : [finished.length === 1 ? describeFinishNotice(finished[0].row, finished[0].seconds) : describeFinishGroup(finished.map((item) => item.row))];
    return [
      ...notice.map((text) => ({ text, fg: api.theme.text.feedback.success.base })),
      ...(line === "" ? [] : [{ text: line, fg: infoColor }]),
    ];
  };

  api.ui.slot({
    append: "session.composer.top",
    // The lines are read inside the JSX, so a new finish notice redraws them.
    render: (props) => (
      <box paddingLeft={2} flexDirection="column">
        {composerLines(props.sessionID).map((line) => (
          <text fg={line.fg}>{line.text}</text>
        ))}
      </box>
    ),
  });

  // A subagent's own sidebar shows its root's data, so both sessions agree.
  api.ui.slot({
    append: "sidebar.content",
    render: (props) => {
      if (!props.sessionID) return null;
      const lines = describeGroupLines(priceFor(rootOf(props.sessionID)), readJsonFile(planFile));
      return lines.length === 0 ? null : (
        <box paddingLeft={2} flexDirection="column">
          <text fg={api.theme.text.base}>
            <b>Usage</b>
          </text>
          {lines.map((line) => (
            <text fg={api.theme.text.muted}>{line}</text>
          ))}
        </box>
      );
    },
  });

  // Every text part is read when the screen renders, so the lines follow the
  // run while the screen is open.
  const usageLines = () =>
    describeGroupLines(rootID ? priceFor(rootID) : { rows: [], tokens: 0, dollars: 0 }, readJsonFile(planFile));
  api.ui.router.register({
    name: "usage",
    render: () => (
      <box flexDirection="column" paddingTop={1} paddingLeft={3}>
        <text fg={api.theme.text.base}>
          <b>Subagent usage  (esc back)</b>
        </text>
        {usageLines().map((line) => (
          <text fg={api.theme.text.muted}>{line}</text>
        ))}
      </box>
    ),
  });

  api.keymap.layer(() => ({
    mode: "global",
    commands: [
      {
        id: "pde.usage.open",
        title: "Subagent usage",
        group: "PDE",
        palette: true,
        slash: { name: "usage" },
        run: () => {
          rememberRoot();
          api.ui.router.navigate({ type: "plugin", name: "usage" });
        },
      },
    ],
  }));

  api.keymap.layer(() => ({
    mode: "global",
    priority: 2,
    enabled: api.ui.router.current()?.type === "plugin" && api.ui.router.current()?.name === "usage",
    commands: [{ bind: "escape", title: "Back", group: "PDE", run: goBack }],
  }));

  // Each subagent id to when OpenCode first ran it, so one that already
  // finished before the plugin started never shows a notice.
  const startedAt = new Map();
  // A toast draws over the chat, so finish notices get a line above the
  // prompt, and each finish leaves it after NOTICE_MS.
  const showNotice = (row, seconds) => {
    const item = { row, seconds };
    setNotices((list) => [...list, item]);
    setTimeout(() => setNotices((list) => list.filter((other) => other !== item)), NOTICE_MS).unref?.();
  };
  const watchSubagents = () => {
    const root = rememberRoot();
    if (!root) return;
    const models = listModels(root);
    for (const sessionID of listSubagents(sessions, root)) {
      const info = withMessages(sessionID);
      if (!info) continue;
      if (info.running) {
        if (!startedAt.has(sessionID)) startedAt.set(sessionID, Date.now());
        continue;
      }
      const first = startedAt.get(sessionID);
      startedAt.delete(sessionID);
      if (first === undefined) continue;
      // Pricing only this subagent keeps its row matched to its own session.
      const row = priceSubagents([info], models).rows[0];
      showNotice(row, (Date.now() - first) / 1000);
    }
  };
  setInterval(watchSubagents, 1000).unref?.();
}
