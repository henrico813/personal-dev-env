// pde-gh-write records this ID so approval prompts can name the session that asked.
export const PdeApprovalSessionPlugin = async () => ({
  "shell.env": async ({ sessionID }, output) => {
    if (sessionID) output.env.OPENCODE_SESSION_ID = sessionID;
  },
});

export default PdeApprovalSessionPlugin;
