// cli.json turns Herdr's own plugin off ("-herdr.opencode.session-selection")
// and loads this one, so the pane has one reporter that counts subagents.
import herdr from "../herdr-tui-session.js";
import { keepWorkingForSubagents } from "./subagents.js";

export default {
  id: "pde.herdr-opencode",
  setup(api) {
    return herdr.setup(keepWorkingForSubagents(api));
  },
};
