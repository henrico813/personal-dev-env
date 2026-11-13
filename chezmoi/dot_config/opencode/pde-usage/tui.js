// Shows subagent tokens, API price and context size above OpenCode's prompt,
// a total in the parent session and each subagent's own in its view, and
// lists each subagent and the Claude plan limits in the session sidebar.
import { homedir } from "node:os";
import { join } from "node:path";
import { fetchCatalogModels } from "./usage.js";
import { addUsageSlots } from "./view.jsx";

const dataHome = process.env.XDG_DATA_HOME || join(homedir(), ".local/share");

export default {
  id: "pde.usage",
  setup(api) {
    // The sidebar uses OpenCode's model list until the catalog arrives, and
    // picks the catalog up on its next redraw.
    let catalogModels = [];
    fetchCatalogModels().then((models) => {
      catalogModels = models;
    });
    addUsageSlots(api, {
      catalogModels: () => catalogModels,
      // The OpenCode Claude plugin saves its latest plan limit event here.
      planFile: process.env.OPENCODE_CLAUDE_RATE_LIMIT_STORE || join(dataHome, "opencode-claude/rate-limit.json"),
    });
  },
};
