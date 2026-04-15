local wezterm = require 'wezterm'

-- Saturation boost helpers
local function hex_to_rgb(hex)
  hex = hex:gsub('#', '')
  if #hex ~= 6 then return nil end
  return
    tonumber(hex:sub(1, 2), 16) / 255,
    tonumber(hex:sub(3, 4), 16) / 255,
    tonumber(hex:sub(5, 6), 16) / 255
end

local function rgb_to_hex(r, g, b)
  return string.format('#%02x%02x%02x',
    math.floor(r * 255 + 0.5),
    math.floor(g * 255 + 0.5),
    math.floor(b * 255 + 0.5))
end

local function rgb_to_hsl(r, g, b)
  local max, min = math.max(r, g, b), math.min(r, g, b)
  local l = (max + min) / 2
  if max == min then return 0, 0, l end
  local d = max - min
  local s = l > 0.5 and d / (2 - max - min) or d / (max + min)
  local h
  if max == r then h = (g - b) / d + (g < b and 6 or 0)
  elseif max == g then h = (b - r) / d + 2
  else h = (r - g) / d + 4 end
  return h / 6, s, l
end

local function hue2rgb(p, q, t)
  if t < 0 then t = t + 1 end
  if t > 1 then t = t - 1 end
  if t < 1 / 6 then return p + (q - p) * 6 * t end
  if t < 1 / 2 then return q end
  if t < 2 / 3 then return p + (q - p) * (2 / 3 - t) * 6 end
  return p
end

local function hsl_to_rgb(h, s, l)
  if s == 0 then return l, l, l end
  local q = l < 0.5 and l * (1 + s) or l + s - l * s
  local p = 2 * l - q
  return hue2rgb(p, q, h + 1 / 3), hue2rgb(p, q, h), hue2rgb(p, q, h - 1 / 3)
end

local function saturate(hex, mult)
  if not hex or #hex < 7 then return hex end
  local r, g, b = hex_to_rgb(hex)
  if not r then return hex end
  local h, s, l = rgb_to_hsl(r, g, b)
  s = math.min(s * mult, 1.0)
  return rgb_to_hex(hsl_to_rgb(h, s, l))
end

local SAT = 1.6

local scheme = wezterm.color.get_builtin_schemes()['Tokyo Night']

for _, key in ipairs({ 'foreground', 'background', 'cursor_bg', 'cursor_fg',
    'cursor_border', 'selection_fg', 'selection_bg', 'scrollbar_thumb', 'split' }) do
  if scheme[key] then scheme[key] = saturate(scheme[key], SAT) end
end

for _, arr in ipairs({ 'ansi', 'brights' }) do
  if scheme[arr] then
    for i, c in ipairs(scheme[arr]) do
      scheme[arr][i] = saturate(c, SAT)
    end
  end
end

return {
  color_schemes = { ['Tokyo Night Saturated'] = scheme },
  color_scheme = 'Tokyo Night Saturated',

  hide_tab_bar_if_only_one_tab = true,

  font = wezterm.font('JetBrainsMonoNL Nerd Font Mono', { weight = 'Regular' }),
  font_rules = {
    { intensity = 'Bold', font = wezterm.font('JetBrainsMonoNL Nerd Font Mono', { weight = 'Bold' }) },
    { italic = true, font = wezterm.font('JetBrainsMonoNL Nerd Font Mono', { italic = true }) },
  },
  font_size = 13.5,
  cell_width = 0.9,

  window_background_opacity = 0.95,
  text_background_opacity = 1.0,

  window_decorations = 'TITLE | RESIZE',

  default_prog = { '/bin/zsh' },

  window_frame = {
    active_titlebar_bg = '#1a1b26',
    inactive_titlebar_bg = '#16161e',
  },

  window_padding = {
    left = 5,
    right = 5,
    top = 5,
    bottom = 5,
  },

  -- Copy mode (CTRL+SHIFT+X) and Quick Select (CTRL+SHIFT+SPACE) are built-in.
  -- These keybinds add explicit mappings and a leader for pane splitting.
  keys = {
    { key = 'x', mods = 'CTRL|SHIFT', action = wezterm.action.ActivateCopyMode },
    { key = 'phys:Space', mods = 'CTRL|SHIFT', action = wezterm.action.QuickSelect },
  },

  -- Quick select: extend built-in patterns with extras
  quick_select_patterns = {
    '[0-9a-f]{7,40}',           -- git hashes
    '\\d+\\.\\d+\\.\\d+\\.\\d+', -- IP addresses
    '[\\w./~-]+/[\\w./~-]+',    -- file paths
  },

}
