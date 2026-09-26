require("copilot").setup({
  panel = { enabled = false },
  suggestion = {
    enabled = true,
    auto_trigger = true,
    hide_during_completion = true,
  },
})

local suggestion_events = vim.api.nvim_create_augroup("PDECopilotSuggestion", { clear = true })

vim.api.nvim_create_autocmd("User", {
  group = suggestion_events,
  pattern = "BlinkCmpMenuOpen",
  callback = function()
    vim.b.copilot_suggestion_hidden = true
  end,
})

vim.api.nvim_create_autocmd("User", {
  group = suggestion_events,
  pattern = "BlinkCmpMenuClose",
  callback = function()
    vim.b.copilot_suggestion_hidden = false
  end,
})
