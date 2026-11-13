// Draws the lines from usage.js in OpenCode's session sidebar and in the row
// above the prompt. OpenCode compiles this JSX itself and redraws the lines
// when its session data changes.
import {
  describePlanUsage,
  describePromptLine,
  describeSubagentRow,
  describeSubagents,
  listSubagents,
  priceSubagents,
  readJsonFile,
} from "./usage.js";

/** Adds the usage panel to the sidebar and the usage row above the prompt. */
export function addUsageSlots(api, { catalogModels, planFile }) {
  const sessions = api.data.session;
  // OpenCode's Context panel reads model limits from the same list. Its
  // entries come first, so the catalog only fills in missing prices.
  const listModels = (sessionID) => [
    ...(api.data.location.model.list(sessions.get(sessionID)?.location) ?? []),
    ...catalogModels(),
  ];
  const withMessages = (id) => {
    const info = sessions.get(id);
    return info && { ...info, messages: sessions.message.list(id) ?? [] };
  };
  const priceFor = (sessionID) =>
    priceSubagents(listSubagents(sessions, sessionID).map(withMessages).filter(Boolean), listModels(sessionID));
  // The file is small, and reading it on each redraw picks up the Claude
  // plugin's latest save.
  const readPlan = () => readJsonFile(planFile);
  const sidebarLines = (sessionID) => {
    const priced = priceFor(sessionID);
    const plan = describePlanUsage(readPlan());
    const lines = [...(priced.rows.length > 0 ? describeSubagents(priced) : []), ...(plan ? [`Claude plan ${plan}`] : [])];
    // The first line is the panel's bold title, so an empty panel draws nothing.
    return lines.length > 0 ? ["Usage", ...lines] : [];
  };

  api.ui.slot({
    append: "sidebar.content",
    render: (props) => (
      <box>
        {sidebarLines(props.sessionID).map((line, index) =>
          index === 0 ? (
            <text fg={api.theme.text.base}>
              <b>{line}</b>
            </text>
          ) : (
            <text fg={api.theme.text.muted}>{line}</text>
          ),
        )}
      </box>
    ),
  });
  // A parent session shows its subagents' total, and a subagent its own row.
  const rowFor = (sessionID) => {
    const info = withMessages(sessionID);
    if (!info) return "";
    if (!info.parentID) return describePromptLine(priceFor(sessionID), readPlan());
    return describeSubagentRow(priceSubagents([info], listModels(sessionID)).rows[0]);
  };
  // The footer row under the prompt shares its width with OpenCode's spinner,
  // which cut the line off at phone width, so the row goes above the prompt.
  api.ui.slot({
    append: "session.composer.top",
    render: (props) => {
      const row = props.sessionID ? rowFor(props.sessionID) : "";
      return row ? (
        <box paddingLeft={2}>
          <text fg={api.theme.text.muted}>{row}</text>
        </box>
      ) : null;
    },
  });
}
