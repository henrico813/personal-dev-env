// Shows subagent usage around OpenCode. A short line above a parent session's
// prompt counts its subagents with their cost or context left, and each
// subagent's own prompt shows its model, context bar, tokens and price. The
// sidebar and the /usage screen carry the group totals and the Claude plan,
// and a notice line reports each subagent as it finishes.
import { homedir } from "node:os";
import { join } from "node:path";
import { fetchCatalogModels, readLineMode } from "./usage.js";
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
      // Asks for the short line showing context left instead of cost.
      lineMode: readLineMode(api.options),
    });
  },
};
