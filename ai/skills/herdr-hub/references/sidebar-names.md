# Sidebar names

Read this when your agent's name is wrong, or when a card's
worker needs a workspace or a rename.

- A card's worker gets its own workspace, named
  `<card key> <one word>`, such as `demo-007 approvals`. Cards that
  belong together share one workspace, named `<project> <topic>`, such
  as `demo review-tools`. When you start a card's worker with
  `herdr-handoff`, make its place by this rule, not as a tab in your
  workspace. Work with no card gets a tab in your workspace.
- Name each tab after its agent. Name a card's agent
  `<card key>-<role>`, such as `demo-007-impl`, so `hub status`
  maps it to its card. This holds for every worker, adopted sessions
  included. Name a hub's own agent `<word>-hub-<n>` instead. When a
  name is wrong, rename the agent and its tab to the right one. The
  tab ID is `.result.agent.tab_id` of `herdr agent get <old name>`:

      herdr agent rename <old name> <new name>
      herdr tab rename <tab ID> <new name>
