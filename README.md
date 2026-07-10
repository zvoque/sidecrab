# Sidecrab

**A Claude Code desktop pet** — a tiny always-on-top companion for macOS: a hand-drawn 8-bit pixel crab that floats
over your screen and reacts to what Claude Code is doing — thinking, running commands,
editing, awaiting permission, done. Inspired by the Codex pet in the ChatGPT desktop app
and the crab in [claude-status-bar](https://github.com/m1ckc3s/claude-status-bar).

<p align="center"><img src="docs/gifs/hero.gif" width="480" alt="Sidecrab on the desktop"></p>

## What he does

| | | | |
|:---:|:---:|:---:|:---:|
| ![idle](docs/gifs/idle.gif) | ![working](docs/gifs/working.gif) | ![thinking](docs/gifs/thinking.gif) | ![permission](docs/gifs/permission.gif) |
| **idle** | **working** — any tool, at his laptop | **thinking** — pondering | **needs permission** — flags you down |

…plus a bunch more moods and moves he'll show you himself.

Interactions: **drag** him anywhere (he remembers) · **double-click** to focus the app
running the session · **right-click** for settings. He also reacts to a few other
things — poke around.

**Wander when idle** (off by default, right-click to enable): when *you* go idle for
~30s, the crab takes little walks around the screen and scurries home when you're back.

### Hats

![hats](docs/gifs/hats.png)

Right-click → **Hat** — top hat, chef's hat, fedora, or helicopter hat (rotor spins).
The hat rides along through every animation.

## Requirements

Claude Code (CLI, desktop app, or IDE). Nothing else — no node, no python, no other
tools. The activity feed comes from a small bundled binary (`sidecrab-hook`).

## Install

```bash
brew install zvoque/tap/sidecrab
sidecrab
```

That's it — he appears bottom-right and your terminal is free. Right-click him for
settings, including **Launch at login**.

On first run he asks before adding a few hooks to `~/.claude/settings.json` so he can
tell when Claude is working (your file is backed up; remove them anytime from his menu).

Update later with `brew upgrade sidecrab`.

## Trademark & IP disclaimer

This is an unofficial, open-source side project. It is not affiliated with,
endorsed by, or sponsored by Anthropic. "Claude", "Clawd", and the Clawd crab
design are Anthropic's trademarks and intellectual property, referenced here
nominatively. The walk-cycle sprite frames derive from Anthropic's Clawd
artwork (by way of
[claude-status-bar](https://github.com/m1ckc3s/claude-status-bar)'s extraction).

This project is MIT licensed, but that covers the **source code only** and
conveys no rights to Anthropic's trademarks, brand, or artwork (see the scope
note in [LICENSE](LICENSE)). Original replacement art is maintained on the
`feat/original-art` branch.

If this project violates or impedes your trademark or copyright, open an issue
or reach me on X ([@zvoque](https://x.com/zvoque)) and it will be addressed
promptly. This is a free side project; it is not monetized.
