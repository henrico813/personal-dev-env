require("fzf-lua").setup({
  keymap = {
    fzf = {
      ["ctrl-d"] = "half-page-down",
      ["ctrl-u"] = "half-page-up",
      ["ctrl-f"] = "page-down",
      ["ctrl-b"] = "page-up",
    },
  },
  defaults = {
    file_icons = false,
  },
  files = {
    cmd = "rg --files --hidden --follow --glob '!.git'",
  },
  grep = {
    rg_opts = "--column --line-number --no-heading --color=always --smart-case --hidden --glob '!.git'",
  },
})
require("fzf-lua").register_ui_select(function(ui_opts)
  if ui_opts.prompt == "Select model" then
    return { prompt = "Select model, " }
  end
end)
