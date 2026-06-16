# Producing the README screenshots (runbook for Claude / future sessions)

The screenshots in `docs/screenshots/*.png` are generated **headlessly and
privacy-safely** by [`docs/autocapture.sh`](autocapture.sh). This file explains
how it works, the hard-won gotchas, and how to regenerate or extend it.

## TL;DR

```bash
# needs: sway, grim, imagemagick (magick), poppler (pdftocairo), a release build
sudo pacman -S --needed sway grim
cargo build --release --features cmux/link-ghostty
bash docs/autocapture.sh        # writes docs/screenshots/*.png
```

Then **review every image** (e.g. read them back) before committing — privacy
is the whole point.

## The approach

cmux is GPU-accelerated GTK4 + ghostty (GLArea). On Wayland (esp. KDE) you can't
screenshot or control another app's window, and you don't want demo windows
flashing on the user's screen. So:

1. Run a **headless, GPU-backed compositor**: `WLR_BACKENDS=headless
   WLR_LIBINPUT_NO_DEVICES=1 sway` on a virtual 1600×1000 output. wlroots uses
   the real render node, so ghostty's GL renders correctly. Nothing appears on
   the user's screen.
2. Launch `target/release/cmux-app` as a Wayland client of that sway, in a
   **clean `env -i`** (no inherited secrets) with a **sandboxed `$HOME`**.
3. Drive every scene through the **cmux socket** (`cmux/bin/cmux` with
   `CMUX_SOCKET=$RT/cmux.sock`).
4. Capture each scene with **`grim`** (works with sway's wlr-screencopy).

Modals (overview, palette, task-manager) can't be dismissed via `cmux send`
(that goes to the terminal, not the dialog), so **relaunch cmux fresh per
scene** — sway stays up, cmux is killed and restarted for each capture. This
also guarantees no state bleeds between scenes.

## Privacy — the non-negotiable part

Everything must be synthetic. Real leaks that bit us and how they're avoided:

- **Login shell leaks `user@host` and `$HOME`.** ghostty spawns the user's real
  login shell via `getpwuid`, so a plain terminal shows `[jbriggs@radd-surfer …]`
  and a title `@radd-surfer:/home/jbriggs`. **Fix:** never show a plain shell.
  Every demo pane runs an **authored script** (`bash $SB/bin/<x>.sh`) that
  `clear`s, prints fake agent output, sets a clean title with
  `printf '\033]0;cmux\007'`, then `sleep`s. No prompt, no env, ever visible.
- **cmux launches TWO default workspaces** — a clean `Terminal` and a leaky
  `@radd-surfer:/home/jbriggs` (the shell's OSC title). After staging the demo
  workspaces, **close every workspace whose title isn't a demo name**
  (`web`/`api`/`docs`) via `cmux close <id>` (close-by-id is reliable;
  select-then-close is not).
- **Closing those leaky workspaces pollutes the History pane** (they land in the
  closed-stack). Before the history scene, run `cmux clear-closed`, then close a
  benign workspace so only that shows.
- **Vault** scans `~/.claude/projects` / `~/.codex/sessions`. The sandbox has
  **fake** sessions with `/home/demo` cwds and benign titles — never the real
  `~/.claude`.
- Paths shown are under `/tmp/cmux-demo` (no username). Acceptable.

## Look & feel (match the user's theme)

Read the user's theme prefs (theme prefs only — never their session/config with
real data) and replicate in the sandbox:

- `~/.config/cmux/settings.json` → `theme` (e.g. `dark`). Write a **minimal**
  `{"theme":"dark"}` to `$SB/.config/cmux/settings.json`. (A partial `sidebar:{…}`
  silently fails the parse and reverts to light — keep it minimal.)
- `~/.config/ghostty/config` → `theme = <name>` (e.g. `Adventure Time`). Write it
  to `$SB/.config/ghostty/config` **and copy the theme file** into
  `$SB/.config/ghostty/themes/` (the sandbox doesn't search `/usr/share/ghostty`).
- GTK font/dark → `$SB/.config/gtk-4.0/settings.ini`
  (`gtk-font-name`, `gtk-application-prefer-dark-theme`). libadwaita ignores the
  dark hint (use cmux `theme=dark`); the font does apply.

**Known gap:** libadwaita reads its **accent color** from the desktop settings
portal, which isn't running in headless sway — so the accent falls back to blue
instead of the user's. Matching it would require standing up a fake
`org.freedesktop.portal.Settings` advertising `accent-color`.

## The settings-vs-commands file trap

`~/.config/cmux/cmux.json` is **both** the settings file (strict,
`deny_unknown_fields`) **and** read for custom `commands`. You can't put both in
it. So:

- **Settings** → `$SB/.config/cmux/settings.json` (and do NOT create
  `$SB/.config/cmux/cmux.json`, or it wins and the `commands` key breaks the
  settings parse → defaults).
- **Commands** → `cmux.json` copied into **every** workspace dir's `.cmux/`
  (`$SB/.cmux/`, `$SB/web/.cmux/`, …). `custom_commands::load` searches the
  *selected* workspace's dir, which changes as you create workspaces, so the
  file must exist in all of them.

Multi-pane demo workspaces are built with a `commands[].workspace` layout
(recursive `split`/`pane`/`surfaces`) and launched via `cmux run <name>`.

## Scene list

`hero`, `pane-overview`, `command-palette`, `dock`, `vault-pane`,
`task-manager`, `pdf-preview`, `history-pane`. Triggers: `cmux overview` /
`palette` / `dock` / `top` / `vault` / `history`, `cmux open <pdf>`, and the
multi-pane `cmux run demo`. The dock needs a `dock.json`; the PDF a sample
generated with `magick … sample.pdf`.

## GIFs

`docs/demos/*.gif` (drag tabs / move panes) are **placeholders**. Scripted
pointer drags don't work here: headless sway with `WLR_LIBINPUT_NO_DEVICES=1`
exposes no usable pointer, so `swaymsg seat … cursor press/move` doesn't reach
GTK (a test click didn't change selection). Recording these needs a real
pointer — record them by hand with `docs/capture.sh gif <name>` while using
cmux, or build a virtual-pointer (`zwlr_virtual_pointer_v1`) driver and verify
GTK drag-and-drop actually fires before relying on it.

## Always do last

Read back **every** generated PNG and confirm: no real username/hostname, no
`/home/<user>`, no real session content, the leaky default workspace is gone,
and the History pane shows only benign closed entries.
