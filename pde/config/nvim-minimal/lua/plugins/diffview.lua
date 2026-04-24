require("diffview").setup({
  enhanced_diff_hl = true,
  view = {
    default = { layout = "diff2_horizontal" },
    merge_tool = { layout = "diff3_horizontal" },
  },
  file_history_panel = {
    log_options = {
      git = {
        single_file = { max_count = 256 },
        multi_file  = { max_count = 256 },
      },
    },
  },
})

local function default_branch()
  local out = vim.fn.system("git symbolic-ref --short refs/remotes/origin/HEAD 2>/dev/null")
  local branch = out:match("origin/(%S+)")
  if branch and branch ~= "" then return branch end
  for _, b in ipairs({ "main", "master" }) do
    if vim.fn.system("git rev-parse --verify " .. b .. " 2>/dev/null"):match("%S") then return b end
  end
  return "main"
end

local map = vim.keymap.set

-- PR review: current branch vs default base (what this branch changed)
map("n", "<leader>gp", function()
  local base = default_branch()
  vim.cmd("tabnew | DiffviewOpen " .. base .. "...HEAD")
end, { desc = "Diff current branch (PR view)" })

-- Prompt for any rev or range
map("n", "<leader>gr", function()
  vim.ui.input({ prompt = "Diff range (e.g. main..HEAD, HEAD~3, <sha>): " }, function(input)
    if input and input ~= "" then vim.cmd("tabnew | DiffviewOpen " .. input) end
  end)
end, { desc = "Diff arbitrary range" })

-- Run git or gh synchronously, return (stdout|nil, stderr)
local function run(cmd, cwd)
  local res = vim.system(cmd, { cwd = cwd, text = true }):wait()
  local out = (res.stdout or ""):gsub("\n$", "")
  local err = (res.stderr or ""):gsub("\n$", "")
  if res.code ~= 0 then return nil, err end
  return out, err
end

-- Pick a directory with fzf-lua, open diffview there
map("n", "<leader>gP", function()
  local cmd
  if vim.fn.executable("fd") == 1 then
    cmd = "fd --type d --hidden --exclude .git"
  else
    cmd = [[find . -type d -not -path '*/.git/*' -not -path '*/.git']]
  end

  require("fzf-lua").fzf_exec(cmd, {
    prompt = "Diffview dir> ",
    cwd = vim.fn.getcwd(),
    actions = {
      ["default"] = function(selected)
        if not selected or not selected[1] then return end
        local dir = selected[1]:gsub("^%s+", ""):gsub("%s+$", "")
        if not dir:match("^/") then dir = vim.fn.getcwd() .. "/" .. dir end
        vim.cmd("tabnew")
        vim.cmd("lcd " .. vim.fn.fnameescape(dir))
        vim.t.name = vim.fn.fnamemodify(dir, ":t")
        vim.cmd("DiffviewOpen")
      end,
    },
  })
end, { desc = "Diffview in picked dir" })

map("n", "<leader>gV", "<cmd>DiffviewClose<cr>",              { desc = "Diffview close" })
map("n", "<leader>gH", "<cmd>DiffviewFileHistory<cr>",        { desc = "Repo history" })
map("n", "<leader>gF", "<cmd>DiffviewFileHistory %<cr>",      { desc = "File history" })
