# Answer an X Watch Warning

On a full install with the i3 desktop, `pde-x-watch` starts when you log
in. Every 10 minutes it checks the X server. It sends a
desktop notice when it finds one of the two problems below, and sends it
again every 2 hours while the problem lasts.

## A Program Holds Too Many X Windows

The notice reads like "wezterm-gui holds 34941 X windows". It comes when
one program holds 5,000 or more X windows. Most programs hold fewer than
30. Each hidden window adds work for Xorg, so the whole desktop slows down
as they pile up.

The windows go away only when the program that holds them exits:

- `wezterm-gui`: close every wezterm window, so the wezterm-gui process
  exits. Then start wezterm again.
- `ibus-x11`: run `ibus restart`. PDE turns ibus off for the i3 session in
  `~/.xinputrc`, so this means ibus was started another way.
- Any other program: quit it and start it again.

wezterm 20240203 leaks these windows when it types through the ibus input
method. The bug is open upstream as
[wezterm issue 6925](https://github.com/wezterm/wezterm/issues/6925).

## Xorg Is Busy

The notice reads like "Xorg used 70% of a CPU core". It comes when Xorg
used 60% or more of one core in the 10 minutes between two checks. The
notice body names the program with the most X windows. When that program
holds thousands, restart it as the section above says.

When no program holds many windows, check that picom runs with the GPU
config. `pgrep -a picom` shows whether picom runs, and
`~/.config/picom/picom.conf` sets `backend = "glx"`. Restart picom:

```bash
pkill picom; picom -b
```

## Stop the Watch

To stop the watch until your next login, run:

```bash
pkill -x pde-x-watch
```

The watch also stops by itself when it cannot reach the X server, such as
after logout. It prints the reason on stderr, after `pde-x-watch:`.
