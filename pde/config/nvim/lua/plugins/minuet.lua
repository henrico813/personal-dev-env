-- minuet-ai.nvim: FiM inline completion via local qwen2.5-coder
-- Backend: openai_fim_compatible → localhost:8111
-- Virtual text: Tab to accept, A-] / A-[ to cycle, A-e to dismiss
-- cmp source: also surfaces suggestions in the popup (Tab to accept from menu)
return {
  {
    "milanglacier/minuet-ai.nvim",
    dependencies = { "nvim-lua/plenary.nvim" },
    config = function()
      require("minuet").setup({
        provider = "openai_fim_compatible",
        n_completions = 1,
        context_window = 1024,
        throttle = 800,
        debounce = 400,
        provider_options = {
          openai_fim_compatible = {
            api_key = "TERM", -- placeholder; local server needs no auth
            name = "qwen2.5-coder",
            end_point = "http://localhost:8111/v1/completions",
            model = "qwen2.5-coder-7b-fim",
            optional = {
              max_tokens = 128,
              top_p = 0.9,
            },
            -- Qwen2.5-Coder FIM token format
            template = {
              prompt = function(before, after, _)
                return "<|fim_prefix|>" .. before .. "<|fim_suffix|>" .. after .. "<|fim_middle|>"
              end,
              suffix = false,
            },
          },
        },
        virtualtext = {
          auto_trigger_ft = { "*" },
          keymap = {
            accept = "<Tab>",
            accept_line = "<A-a>",
            next = "<A-]>",
            prev = "<A-[>",
            dismiss = "<A-e>",
          },
        },
      })
    end,
  },
  -- surface minuet suggestions in the cmp popup as well
  {
    "hrsh7th/nvim-cmp",
    optional = true,
    opts = function(_, opts)
      table.insert(opts.sources, 1, { name = "minuet", group_index = 1, priority = 100 })
      opts.performance = vim.tbl_extend("force", opts.performance or {}, { fetching_timeout = 3000 })
    end,
  },
}
