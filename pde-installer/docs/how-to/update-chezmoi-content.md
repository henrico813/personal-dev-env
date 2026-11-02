# Update Chezmoi Content

1. Edit files under the repository's `../chezmoi/` source directory.
2. For an external archive, update its URL and pinned SHA-256 value in
   `../chezmoi/.chezmoiexternal.toml.tmpl`.
3. When changing files under `../ai/`, do not update a checksum; local
   `file://` externals intentionally omit hand-maintained hashes. A new
   file under `../ai/skills/` needs no template change once it is added
   to Git: the template installs every skill file that Git tracks.
4. Preview the complete saved-selection reconciliation:

   ```bash
   go run . install --dry-run --repo-root ..
   ```

5. Apply them:

   ```bash
   go run . install --repo-root ..
   ```

Every install reconciles tools as well as managed home configuration. The
installer renders selection-aware templates with the profile's tiers in `.pde`
data, passed with `--override-data`, and the color profile in the
`PDE_COLOR_PROFILE` environment variable, so use it rather than applying
chezmoi directly. Test a tier from `.pde.tiers`, such as
`{{ if has "development" .pde.tiers }}`, rather than a profile name.
Shared palette and application-theme data belongs in
`../chezmoi/.chezmoidata.json`; keep the supported names aligned with
`internal/colorprofile`. For examples, run `pde-installer install --help`.

Desktop files go under the folders that `../chezmoi/.chezmoiignore.tmpl` skips
on machines without that desktop. Files both desktops use go in
`.config/pde-desktop/` or the other shared paths that file lists:
`.config/rofi/`, `.config/dunst/`, `.config/gtk-3.0/settings.ini` and
`.config/autostart/xfce4-notifyd.desktop`. i3's files go in `.config/i3/`,
`.config/polybar/` and `.config/cava/`. A desktop file anywhere else installs on
every machine, so add a new folder to the ignore list and to
`TestIgnoreListCoversEveryDesktopFile`. Configs call scripts by full path, such
as `~/.config/pde-desktop/scripts/wallpaper.sh`, because those folders are not
on `PATH`.

Every install overwrites managed files. For a file the user edits, such as
`~/.config/i3/local.conf`, use chezmoi's `create_` prefix, which writes the
file only when it is missing.

Removing a managed file only stops chezmoi from writing it; it does not delete
the live copy. Add the home-relative target path to `../chezmoi/.chezmoiremove`,
the list of files chezmoi deletes from your home directory. When the file has a
live side effect such as an enabled systemd unit, add a `run_once_before_<name>`
script; chezmoi runs these once, before it applies changes, so the script can
reverse the effect before the file is removed. Do not also list the target in
`.chezmoiignore`; ignored targets are skipped by the `.chezmoiremove` list.
