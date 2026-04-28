require("fzf-lua").setup({
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
