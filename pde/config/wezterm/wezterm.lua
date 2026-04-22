local wezterm = require 'wezterm'

return {
  color_scheme = 'Tokyo Night',

  hide_tab_bar_if_only_one_tab = true,

  font = wezterm.font('JetBrainsMono Nerd Font Mono', { weight = 'Regular' }),
  font_rules = {
    { intensity = 'Bold', font = wezterm.font('JetBrainsMono Nerd Font Mono', { weight = 'Bold' }) },
    { italic = true, font = wezterm.font('JetBrainsMono Nerd Font Mono', { italic = true }) },
  },
  font_size = 13.5,
  cell_width = 0.9,

  window_background_opacity = 0.95,
  text_background_opacity = 1.0,

  window_decorations = 'TITLE | RESIZE',

  default_prog = { '/bin/zsh' },

  window_frame = {
    active_titlebar_bg = '#000000',
    inactive_titlebar_bg = '#000000',
  },

  window_padding = {
    left = 0,
    right = 0,
    top = 0,
    bottom = 0,
  },

  keys = {
    { key = 'x', mods = 'CTRL|SHIFT', action = wezterm.action.ActivateCopyMode },
    { key = 'phys:Space', mods = 'CTRL|SHIFT', action = wezterm.action.QuickSelect },
  },

  quick_select_patterns = {
    '[0-9a-f]{7,40}',
    '\\d+\\.\\d+\\.\\d+\\.\\d+',
    '[\\w./~-]+/[\\w./~-]+',
  },
}
