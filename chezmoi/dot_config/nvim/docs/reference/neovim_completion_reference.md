# Neovim Completion Reference

This reference covers the configured GitHub Copilot FIM suggestions and blink
completion menu.

## Completion Components

| Component | Use |
|---|---|
| GitHub Copilot | Predict new code at the cursor as inline ghost text. |
| blink.cmp | Select existing LSP symbols, paths, and buffer words from a menu. |

## GitHub Copilot FIM

Copilot suggests inline ghost text while you type in an ordinary, listed buffer.
Suggestions trigger automatically and remain hidden while the blink menu is
open.

| Key or command | Action |
|---|---|
| `<C-l>` | Accept the visible Copilot suggestion. |
| `<C-]>` | Dismiss the visible Copilot suggestion. |
| `:Copilot auth` | Start GitHub authentication. |
| `:Copilot auth info` | Show the current authentication state. |

Copilot stores its authentication credentials at
`~/.config/github-copilot/auth.db`, outside this repository.

## blink Completion Menu

blink does not open automatically. In Insert mode, use `<C-Space>` to open the
menu after typing enough of a symbol, path, or word to narrow the choices.

| Key | Action |
|---|---|
| `<C-Space>` | Open the completion menu. |
| `<C-n>` / `<Down>` | Select the next item. |
| `<C-p>` / `<Up>` | Select the previous item. |
| `<Enter>` | Accept the selected item. |
| `<Enter>` without a selected item | Insert a newline. |
| `<C-e>` | Cancel the completion menu. |
| `<C-b>` / `<C-f>` | Scroll completion documentation. |
| `<C-k>` | Show or hide signature help. |
| `<Tab>` / `<S-Tab>` | Move through active snippet placeholders. |

The menu opens with no selected item. Choose an item before pressing `<Enter>`
to prevent an unintended completion from being accepted.

## Troubleshooting

`<C-Space>` only opens blink while Insert mode is active. If it does not open a
menu there, use these commands to inspect the mappings Neovim received:

```vim
:verbose imap <C-Space>
:verbose imap <C-@>
```

`<C-@>` is the terminal encoding that some terminal and tmux paths use for
`<C-Space>`.
