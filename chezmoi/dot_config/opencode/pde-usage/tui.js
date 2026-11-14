// Shows subagent usage and plan usage around OpenCode. A short line above a
// parent session's prompt counts its subagents with their cost or context
// left, and each subagent's own prompt shows its model, context bar, tokens
// and price. The sidebar and the /usage screen carry the group totals and the
// plan lines, and a notice line reports each subagent as it finishes. The
// plugin reads plan usage that `pde-quota` saves and never asks a provider itself.
import { claudeLimitPath } from "./quota/pick.js";
import { quotaDir } from "./quota/store.js";
import { fetchCatalogModels, readLineMode } from "./usage.js";
import { addUsageSlots } from "./view.jsx";

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
      quotaDir: quotaDir(),
      rateLimitFile: claudeLimitPath(),
      // Asks for the short line showing context left instead of cost.
      lineMode: readLineMode(api.options),
    });
  },
};
