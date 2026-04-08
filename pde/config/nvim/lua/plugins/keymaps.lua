return {
	"LazyVim/LazyVim",
	init = function()
		vim.keymap.set("t", "<C-q>", "<C-\\><C-n>", { desc = "Exit terminal mode" })
	end,
}
