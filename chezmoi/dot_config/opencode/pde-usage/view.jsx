// Draws the lines from usage.js in OpenCode's session sidebar and under the
// prompt. OpenCode compiles this JSX itself and redraws the lines when its
// session data changes.
import {
  describePlanUsage,
  describePromptLine,
  describeSubagents,
  listSubagents,
  priceSubagents,
  readJsonFile,
} from "./usage.js";

/** Adds the usage panel to the sidebar and the usage line under the prompt. */
export function addUsageSlots(api, { catalogModels, planFile }) {
  const sessions = api.data.session;
  // OpenCode's Context panel reads model limits from the same list. Its
  // entries come first, so the catalog only fills in missing prices.
  const listModels = (sessionID) => [
    ...(api.data.location.model.list(sessions.get(sessionID)?.location) ?? []),
    ...catalogModels(),
  ];
  const priceFor = (sessionID) =>
    priceSubagents(
      listSubagents(sessions, sessionID)
        .map((id) => {
          const info = sessions.get(id);
          return info && { ...info, messages: sessions.message.list(id) ?? [] };
        })
        .filter(Boolean),
      listModels(sessionID),
    );
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
  api.ui.slot({
    append: "prompt.footer.status",
    render: (props) => (
      <text fg={api.theme.text.muted}>
        {props.sessionID ? describePromptLine(priceFor(props.sessionID), readPlan()) : ""}
      </text>
    ),
  });
}
