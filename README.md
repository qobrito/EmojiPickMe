# emoji-pick-me

A small native dmenu-style emoji picker for Wayland (tested with niri in mind).
Launch it, type a query, pick an emoji with the arrow keys or the mouse, press Enter, and the emoji is copied to the clipboard.

Written in Rust using GTK4. The emoji list and their names come from the [`emojis`](https://crates.io/crates/emojis) crate and are embedded in the binary. Names in other languages are not embedded: they can be downloaded in the settings from [Unicode CLDR](https://github.com/unicode-org/cldr).

## Features

- search by English name and shortcode (`:thumbs_up:`), and, with a locale applied, by its names and keywords as well (for Russian: «лайк», «огонь», «программист»); multiple words in a query work as AND, and «ё» and «е» are treated as the same letter
- name localizations can be downloaded, applied, unapplied and deleted right in the settings; with a locale applied, the list shows its name, with the English one below it
- list of recently used emoji (can be disabled and limited in number)
- keyboard and mouse control
- opens as an overlay above all windows in the center of the screen (via wlr-layer-shell, like dmenu/fuzzel) and grabs the keyboard; closes on `Esc` or after a selection is made

## Controls

| Keys | Action |
|---|---|
| typing text | search |
| `↑` / `↓`, `Ctrl+P` / `Ctrl+N` | select an emoji in the list |
| `Tab` | switch focus between list ⇄ input field |
| `←` / `→` | in the input field — move the text cursor |
| `Enter` or click | copy the selected emoji and close the window |
| `Ctrl+,` | open / close settings |
| `Esc` | close the window (in settings — go back) |

In list mode, regular characters and `Backspace` automatically return focus to the input field.

## Settings

Opened with `Ctrl+,` right in the window and applied immediately:

- dark theme (on/off; when turned off, whatever is selected in the system GTK settings is used)
- show recent emoji (on/off)
- maximum number of recent emoji (1–200, 24 by default)
- name localizations: a list of languages with the buttons "Download", "Apply", "Unapply", "Delete". Only one locale can be applied at a time. A language that is not in the list can be downloaded by its CLDR code (e.g. `sr_Latn`) via the field at the bottom. The files `common/annotations/<code>.xml` and `common/annotationsDerived/<code>.xml` are downloaded from the CLDR repository; this requires internet access and `curl`

Files:

- `~/.config/emoji-pick-me/config` — settings (`recent_enabled`, `recent_max`, `locale`, `dark`)
- `~/.local/share/emoji-pick-me/recent` — list of recent emoji, one emoji per line
- `~/.local/share/emoji-pick-me/locales/<code>.tsv` — downloaded localizations: emoji, name, keywords separated by `|`

## Structure

- `src/main.rs` — application entry point
- `src/ui.rs` — main window, emoji list, keyboard, clipboard
- `src/settings.rs` — settings page, dark theme, locale management
- `src/search.rs` — search and ranking
- `src/locale.rs` — downloading and parsing CLDR localizations
- `src/config.rs` — config and list of recent emoji

## Dependencies

Arch Linux:

```sh
sudo pacman -S --needed rust gtk4 gtk4-layer-shell wl-clipboard noto-fonts-emoji curl
```

- `rust` — compiler and cargo (can be installed via rustup)
- `gtk4` — graphics library
- `gtk4-layer-shell` — so that the window is an overlay rather than a regular window
- `wl-clipboard` — the `wl-copy` program through which the emoji is put into the clipboard
- `noto-fonts-emoji` — font with color emoji
- `curl` — for downloading localizations (usually already installed)

Building also requires `pkgconf` and a C compiler (usually already present in `base-devel`).

## Building

```sh
git clone https://github.com/qobrito/EmojiPickMe.git
cd EmojiPickMe
cargo build --release
```

The finished binary will be at `target/release/emoji-pick-me`.

## Installation

```sh
install -Dm755 target/release/emoji-pick-me ~/.local/bin/emoji-pick-me
```

Make sure `~/.local/bin` is in your `$PATH`. Or run without installing: `cargo run --release`.

## Configuring niri

Add to `~/.config/niri/config.kdl`:

```kdl
binds {
    Mod+Period { spawn "emoji-pick-me"; }
}
```

A `window-rule` is not needed: the application is not a regular window but a layer-shell surface (namespace `emoji-pick-me`), which niri does not place into columns. If desired, it can be styled via `layer-rule { match namespace="emoji-pick-me" ... }`.

## Troubleshooting

- **emoji is not copied or is copied empty** — check that `wl-clipboard` is installed (`which wl-copy`). Without it the application copies via GTK, but the contents disappear when the window closes unless there is a clipboard manager.
- **boxes instead of emoji** — install `noto-fonts-emoji` and restart the application.
- **opens as a regular window instead of an overlay** — this means the compositor does not support wlr-layer-shell (niri does), and the application fell back to a regular window. The `gtk4-layer-shell` package is still required in any case: without it the program will neither build nor run.
- **dark theme does not turn on** — the `GTK_THEME` environment variable sets the theme explicitly (e.g. `GTK_THEME=Adwaita` without `:dark`), or the theme has no dark variant.
