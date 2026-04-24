function _G.pi_session_name()
  local ok, data = pcall(function() return require("pi.state").get("sessions.current") end)
  if ok and data then return "session: " .. (data.sessionName or "unnamed") end
  return ""
end

local map = vim.keymap.set

-- no-arg commands
map("n", "<leader>ps", "<cmd>PiSession<cr>",       { desc = "Session info" })
map("n", "<leader>pn", "<cmd>PiSessionNew<cr>",    { desc = "New session" })
map("n", "<leader>pf", "<cmd>PiSessionFork<cr>",   { desc = "Fork session (picker)" })
map("n", "<leader>pS", "<cmd>PiSessionStats<cr>",  { desc = "Session stats" })

-- commands that need args — prompt the user
map("n", "<leader>pN", function()
  vim.ui.input({ prompt = "Session name: " }, function(input)
    if input and input ~= "" then
      vim.cmd("PiSessionName " .. input)
      vim.defer_fn(function()
        pcall(function() require("pi.rpc.session").current(require("pi.state").get("rpc_client"), function() end) end)
        vim.cmd("redrawstatus!")
      end, 200)
    end
  end)
end, { desc = "Rename session" })

local function read_session_meta(path)
  local f = io.open(path, "r")
  if not f then return nil end
  local name, timestamp, msg_count = nil, nil, 0
  for line in f:lines() do
    local ok, entry = pcall(vim.json.decode, line)
    if ok and entry then
      if entry.type == "session" then
        timestamp = entry.timestamp
      elseif entry.type == "session_info" and entry.name then
        name = entry.name
      elseif entry.type == "user_message" or entry.type == "prompt" then
        msg_count = msg_count + 1
      end
    end
  end
  f:close()
  return { name = name or "unnamed", timestamp = timestamp or "?", msg_count = msg_count }
end

map("n", "<leader>pw", function()
  local client = require("pi.state").get("rpc_client")
  if not client then
    vim.notify("Pi: Not connected", vim.log.levels.ERROR)
    return
  end
  require("pi.rpc.session").current(client, function(result)
    local dir = result and result.success and result.data and result.data.sessionFile
      and vim.fn.fnamemodify(result.data.sessionFile, ":h")
    if not dir then
      vim.notify("Pi: no session dir (send a message first)", vim.log.levels.WARN)
      return
    end

    local files = vim.fn.globpath(dir, "*.jsonl", false, true)
    local entries = {}
    local path_by_display = {}
    for _, path in ipairs(files) do
      local meta = read_session_meta(path)
      if meta then
        local date = meta.timestamp:sub(1, 16):gsub("T", " ")
        local display = string.format("%s | %3d msgs | %s", date, meta.msg_count, meta.name)
        table.insert(entries, display)
        path_by_display[display] = path
      end
    end
    table.sort(entries, function(a, b) return a > b end)

    vim.schedule(function()
      require("fzf-lua").fzf_exec(entries, {
        prompt = "Pi session> ",
        actions = {
          ["default"] = function(selected)
            if selected and selected[1] then
              local path = path_by_display[selected[1]]
              if path then
                vim.cmd("PiSessionSwitch " .. vim.fn.fnameescape(path))
                vim.defer_fn(function()
                  pcall(vim.cmd, "PiChat")
                  pcall(vim.cmd, "PiChat")
                end, 200)
              end
            end
          end,
        },
      })
    end)
  end)
end, { desc = "Switch session (fzf)" })

map("n", "<leader>pe", function()
  vim.ui.input({ prompt = "Export to (leave empty for default): ", completion = "file" }, function(input)
    vim.cmd("PiSessionExport" .. (input and input ~= "" and " " .. input or ""))
  end)
end, { desc = "Export session (HTML)" })

require("pi").setup({
  auto_connect = false,
  auto_open_panel = false,
  approval_mode = true,
  keymaps = {
    toggle_panel = "<leader>pt",
    toggle_logs  = "<leader>pl",
    toggle_chat  = "<leader>pc",
    approve      = "<leader>pa",
    reject       = "<leader>pr",
  },
})

local pi_session_cache = vim.fn.stdpath("data") .. "/pi-last-session.txt"

-- Save current pi session path on exit
vim.api.nvim_create_autocmd("VimLeavePre", {
  callback = function()
    local ok, data = pcall(function() return require("pi.state").get("sessions.current") end)
    if ok and data and data.sessionFile then
      local f = io.open(pi_session_cache, "w")
      if f then f:write(data.sessionFile); f:close() end
    end
  end,
})

local function wipe_pi_buffers()
  for _, buf in ipairs(vim.api.nvim_list_bufs()) do
    local tail = vim.fn.fnamemodify(vim.api.nvim_buf_get_name(buf), ":t")
    if tail:match("^Pi") then
      pcall(vim.api.nvim_buf_delete, buf, { force = true })
    end
  end
end

local function restore_pi_from_cache()
  local f = io.open(pi_session_cache, "r")
  if not f then
    pcall(vim.cmd, "PiChat")
    return
  end
  local path = f:read("*l")
  f:close()

  local client = require("pi.state").get("rpc_client")
  if path and path ~= "" and vim.fn.filereadable(path) == 1 and client then
    require("pi.rpc.session").switch(client, path, function()
      vim.schedule(function() pcall(vim.cmd, "PiChat") end)
    end)
  else
    pcall(vim.cmd, "PiChat")
  end
end

-- Restore pi chat after persistence.nvim finishes loading a session
vim.api.nvim_create_autocmd("User", {
  pattern = "PersistenceLoadPost",
  callback = function()
    wipe_pi_buffers()

    local client = require("pi.state").get("rpc_client")
    if client and client.connected then
      restore_pi_from_cache()
    else
      require("pi").connect(function(success)
        if not success then return end
        vim.schedule(restore_pi_from_cache)
      end)
    end
  end,
})

