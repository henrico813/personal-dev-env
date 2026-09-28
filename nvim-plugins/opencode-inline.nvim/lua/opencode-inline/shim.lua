-- Owns the detached opencode-inline-shim process. A healthy shim is reused
-- across Neovim sessions; the PID file lets a later session replace it.
local M = {}

local shim_job
local pidfile = vim.fn.stdpath("state") .. "/opencode-inline-shim.pid"

local function read_pidfile()
  local file = io.open(pidfile, "r")
  if not file then
    return nil
  end

  local contents = file:read("*a")
  file:close()
  return tonumber(vim.trim(contents or ""))
end

local function write_pidfile(pid)
  if not pid or pid <= 0 then
    return
  end

  local file = io.open(pidfile, "w")
  if not file then
    return
  end

  file:write(tostring(pid), "\n")
  file:close()
end

local function clear_pidfile()
  pcall(vim.fn.delete, pidfile)
end

local function process_command(pid)
  if not pid or pid <= 0 then
    return nil
  end

  local ok, result = pcall(function()
    return vim.system({ "ps", "-o", "command=", "-p", tostring(pid) }, { text = true }):wait()
  end)
  if not ok or not result or result.code ~= 0 or not result.stdout then
    return nil
  end

  local command = vim.trim(result.stdout)
  if command == "" then
    return nil
  end
  return command
end

-- Only signal processes that are still the shim; PIDs can be reused.
local function inline_shim_pid(pid)
  local command = process_command(pid)
  if not command or not command:find("opencode%-inline%-shim", 1, false) then
    return nil
  end
  return pid
end

local function kill_inline_shim_pid(pid)
  pid = inline_shim_pid(pid)
  if not pid then
    return
  end

  pcall(function()
    vim.system({ "kill", tostring(pid) }):wait()
  end)
end

local function port_listener_output(port)
  local ok, result = pcall(function()
    return vim.system({ "ss", "-ltnp", "( sport = :" .. port .. " )" }, { text = true }):wait()
  end)
  if not ok or not result or result.code ~= 0 or not result.stdout then
    return nil
  end

  return result.stdout
end

local function listener_exists(port)
  local output = port_listener_output(port)
  if not output then
    return false
  end

  local lines = 0
  for line in output:gmatch("[^\n]+") do
    if vim.trim(line) ~= "" then
      lines = lines + 1
    end
  end
  return lines > 1
end

local function listener_pid(port)
  local output = port_listener_output(port)
  if not output then
    return nil
  end

  return inline_shim_pid(tonumber(output:match("pid=(%d+)")))
end

local function cleanup_stale(port, opts)
  opts = opts or {}
  local tracked_pid = read_pidfile()
  if opts.kill_tracked ~= false then
    kill_inline_shim_pid(tracked_pid)
  end

  local pid = listener_pid(port)
  if pid and opts.kill_listener ~= false and pid ~= tracked_pid then
    kill_inline_shim_pid(pid)
  end

  if opts.wait_for_release then
    vim.wait(1500, function()
      return not listener_exists(port)
    end, 50)
  end

  clear_pidfile()
end

local function healthcheck(spec, callback)
  vim.system({ spec.bin, "--healthcheck", "--port", spec.port }, { text = true }, function(result)
    vim.schedule(function()
      callback(result.code == 0)
    end)
  end)
end

local function wait_for_ready(spec, attempts, callback)
  healthcheck(spec, function(ready)
    if ready or attempts <= 1 then
      return callback(ready)
    end
    vim.defer_fn(function()
      wait_for_ready(spec, attempts - 1, callback)
    end, 150)
  end)
end

local function tracked_running()
  return shim_job and vim.fn.jobwait({ shim_job }, 0)[1] == -1
end

---Stop the tracked shim and any stale shim still listening on the port.
---@param spec { port: string }
function M.stop(spec)
  if shim_job then
    pcall(vim.fn.jobstop, shim_job)
    shim_job = nil
  end
  cleanup_stale(spec.port, { wait_for_release = true })
end

---Reuse a healthy shim on the port or launch a new detached one.
---@param spec { bin: string, port: string, args: string[], env: table<string, string> }
---@param on_ready? fun(ready: boolean)
function M.start(spec, on_ready)
  local function done(ready)
    if on_ready then
      on_ready(ready)
    end
  end

  local function launch()
    cleanup_stale(spec.port, { wait_for_release = true })
    shim_job = vim.fn.jobstart(vim.list_extend({ spec.bin }, spec.args), { detach = true, env = spec.env })
    if shim_job <= 0 then
      shim_job = nil
      vim.notify("failed to start opencode-inline-shim", vim.log.levels.ERROR)
      return done(false)
    end

    local pid = vim.fn.jobpid(shim_job)
    if pid and pid > 0 then
      write_pidfile(pid)
    else
      clear_pidfile()
    end

    wait_for_ready(spec, 10, function(ready)
      if not ready then
        vim.notify("opencode-inline-shim did not become ready", vim.log.levels.ERROR)
      end
      done(ready)
    end)
  end

  healthcheck(spec, function(ready)
    if ready then
      return done(true)
    end
    if tracked_running() then
      M.stop(spec)
    end
    launch()
  end)
end

return M
