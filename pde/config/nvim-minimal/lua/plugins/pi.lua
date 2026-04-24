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

vim.api.nvim_create_autocmd("SessionLoadPost", {
  callback = function()
    require("pi").connect(function(success)
      if not success then return end

      -- pi is running: wipe stale session buffers so the new chat is the only one
      for _, buf in ipairs(vim.api.nvim_list_bufs()) do
        local tail = vim.fn.fnamemodify(vim.api.nvim_buf_get_name(buf), ":t")
        if tail:match("^Pi") then
          pcall(vim.api.nvim_buf_delete, buf, { force = true })
        end
      end

      vim.cmd("PiChat")
    end)
  end,
})
