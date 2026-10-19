require("fzf-lua").setup({
  -- Called each time a picker opens. On a phone the preview leaves too little
  -- room for results.
  winopts = function()
    if require("core.phone_view").is_narrow() then
      return { fullscreen = true, border = "none", preview = { hidden = true } }
    end
    return {}
  end,
  -- FZF_DEFAULT_OPTS in ~/.zshrc adds a margin and padding, which cost four
  -- of about 15 rows on a phone.
  fzf_opts = function()
    if require("core.phone_view").is_narrow() then
      return { ["--margin"] = "0", ["--padding"] = "0" }
    end
    return {}
  end,
  -- On a phone, Ctrl keys are awkward, so j and k move, q closes and / starts
  -- typing. After /, j and k type for the rest of that picker.
  keymap = function()
    local fzf = {
      ["ctrl-d"] = "half-page-down",
      ["ctrl-u"] = "half-page-up",
      ["ctrl-f"] = "page-down",
      ["ctrl-b"] = "page-up",
    }
    if require("core.phone_view").is_narrow() then
      fzf["j"] = "down"
      fzf["k"] = "up"
      fzf["q"] = "abort"
      fzf["/"] = "unbind(j,k,q,/)+change-prompt(/ )"
    end
    return { fzf = fzf }
  end,
  defaults = {
    file_icons = false,
    formatter = "path.filename_first",
  },
  files = {
    cmd = "rg --files --hidden --follow --glob '!.git'",
  },
  grep = {
    -- On a phone each row shows the match, unindented, then the file. Line
    -- and column are hidden, since fzf-lua always adds --column; Enter still
    -- opens the match.
    fzf_opts = function()
      local opts = { ["--multi"] = true }
      if require("core.phone_view").is_narrow() then
        opts["--delimiter"] = ":\\s*"
        opts["--with-nth"] = "4..,1"
      end
      return opts
    end,
    rg_opts = "--column --line-number --no-heading --color=always --smart-case --hidden --glob '!.git'",
  },
})
require("fzf-lua").register_ui_select(function(ui_opts)
  if ui_opts and ui_opts.prompt == "Select model" then
    return { prompt = "Select model, " }
  end
  return {}
end)
