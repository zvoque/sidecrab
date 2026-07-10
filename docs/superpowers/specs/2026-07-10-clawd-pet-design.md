# Clawd Pet — Design Spec

**Date:** 2026-07-10
**Status:** Approved for planning
**Project:** `~/Desktop/dev/clawd-pet`

## 1. Overview

An always-on-top, draggable desktop "pet" for macOS: a hand-drawn 8-bit pixel crab
(Clawd-inspired) that floats over the screen and animates in reaction to what Claude Code
is doing. Inspired by the Codex "pet" in the ChatGPT desktop app and by the animated crab
in `m1ckc3s/claude-status-bar` (CSB), but delivered as a free-floating desktop companion
instead of a menu-bar item.

The crab reacts to Claude Code's live state (thinking, running a tool, awaiting permission,
done, idle), has ambient micro-life when idle, and — when the *user* goes idle — can wander
around the screen.

## 2. Goals / Non-Goals

**Goals**
- Floating, always-on-top, transparent, draggable crab window.
- Animate in real time from Claude Code activity.
- **Fully standalone.** Only requirement is Claude Code (any surface: CLI, desktop app, IDE).
  We ship our own hook writer and install our own hooks — no dependency on CSB or any other
  tool, and no external runtime (node/python) needed.
- Authentic 8-bit look via hand-coded pixel sprites (our own art).
- Small footprint (Tauri, ~5MB) and negligible idle CPU.
- Delightful interactions: pet it, reposition it, right-click menu, activate host app.
- Optional wander-when-user-idle behavior.

**Non-Goals (v1)**
- Not a status *bar* — no menu-bar item.
- No telemetry/network calls.
- No use of Anthropic's trademarked "Clawd" name or its official (never-released) asset.
  Art is original, Clawd-*inspired*. Product is not named "Clawd".
- Deferred to later: session-count badge, auto-launch at login, per-tool animations beyond
  the two working variants, sound.

## 3. Architecture

Tauri v2 app plus a tiny bundled hook-writer binary. The Rust backend owns the OS window and
filesystem watching; the frontend is vanilla HTML/CSS/JS with a `<canvas>` for the pixel crab.
No frontend framework, no external runtime.

```
Claude Code (any surface) ──fires hooks──▶ clawd-pet-hook (bundled binary)
                                             │ writes state.json + sessions.d/
                                             ▼
                         <app-config>/clawd-pet/state.json
                                             │ (watched)
┌─────────────────────────────────────────────────────────┐
│ Rust backend (src-tauri)                                 │
│  • on first run: installs our hooks into                 │
│    ~/.claude/settings.json (idempotent, additive)        │
│  • creates the floating window (always-on-top,           │
│    transparent, borderless, all-Spaces, skip-taskbar)    │
│  • watches <app-config>/clawd-pet/state.json (notify)    │
│    → emits `claude-state` event to the webview           │
│  • polls macOS HIDIdleTime → emits `user-idle`/`user-active` │
│  • commands: set_ignore_cursor_events, resize, reveal    │
│    transcript, activate host app, persist/load config    │
└───────────────┬─────────────────────────────────────────┘
                │ events + IPC
┌───────────────▼─────────────────────────────────────────┐
│ Frontend (webview)                                       │
│  • sprite engine: pixel matrices → canvas (rAF loop)     │
│  • state machine: claude-state → animation               │
│  • behavior: micro-life, pet reaction, wander controller │
│  • input: drag, click, dblclick, contextmenu             │
│  • alpha hit-testing → toggles click-through             │
└──────────────────────────────────────────────────────────┘
```

### 3.1 Component boundaries

| Unit | Responsibility | Depends on |
|---|---|---|
| `clawd-pet-hook` (Rust bin) | Standalone hook handler: read hook JSON on stdin, map event→status, atomically write `state.json`; maintain `sessions.d/`; record `TERM_PROGRAM`/host at SessionStart | serde_json |
| `hook_installer` (Rust) | On first run, idempotently add our hook entries to `~/.claude/settings.json` (additive, preserves existing incl. CSB); command to remove them | serde_json |
| `state_watcher` (Rust) | Watch `<app-config>/clawd-pet/state.json`, debounce, parse, emit `claude-state` | notify, serde_json, Tauri emit |
| `idle_monitor` (Rust) | Poll `HIDIdleTime`, emit `user-idle`/`user-active` on threshold crossing | core-foundation/`ioreg`, Tauri emit |
| `os_actions` (Rust) | `reveal_transcript`, `activate_host` (Claude.app or terminal via recorded `TERM_PROGRAM`), `set_click_through`, `resize_window` commands | Tauri, `open`/AppleScript |
| `config` (Rust) | Load/save `{position, size, wanderEnabled}` JSON in app-config dir | tauri app dir, serde |
| `sprites` (JS) | Frame data (pixel matrices) + canvas renderer | none |
| `state-machine` (JS) | Map `claude-state` → active animation; handle transitions (e.g. done→idle) | sprites |
| `behavior` (JS) | Micro-life timers, pet reaction, wander pathing | state-machine, sprites |
| `input` (JS) | drag / click / dblclick / contextmenu; alpha hit-test → click-through | os_actions commands |

Each unit is independently testable: sprite frames render deterministically; the state
machine is a pure map from state string → animation id; config round-trips JSON.

## 4. State feed — our own, standalone

We own the whole feed. The bundled `clawd-pet-hook` binary is invoked by our hooks and
atomically writes `<app-config>/clawd-pet/state.json`:

```json
{
  "state": "idle | thinking | tool | permission | done",
  "label": "Running command",
  "tool":  "Bash",
  "project": "doomgeneric",
  "sessionId": "f76126bb-…",
  "transcript": "/Users/…/<sessionId>.jsonl",
  "host": "iTerm.app | Apple_Terminal | vscode | Claude | …",
  "startedAt": 1782689277,
  "ts": 1782689297
}
```

**Hook → status mapping** (in `clawd-pet-hook`, mirrors the proven CSB logic):

| Hook event | state | label |
|---|---|---|
| `UserPromptSubmit` | `thinking` | "Thinking…" |
| `PreToolUse` | `tool` | tool-specific verb (Bash→"Running command", Edit/Write→"Editing", Read→"Reading", …); unknown→"Using tool" |
| `PostToolUse` | `thinking` | "Thinking…" |
| `Notification`/`PermissionRequest` (permission only) | `permission` | "Awaiting permission" |
| `Stop` | `done` | "Done" (decays to `idle`) |
| `SessionStart` | — | create `sessions.d/<sid>`, record `host` from `TERM_PROGRAM` (or "Claude" if a desktop session) |
| `SessionEnd` | — | remove `sessions.d/<sid>` |

- **First-run consent (disclaimer):** before any edit, the pet shows a one-time dialog:
  *"Clawd Pet needs to add hooks to `~/.claude/settings.json` so it can tell when Claude Code
  is working. Your file is backed up to `settings.json.bak` first, existing hooks are kept,
  and you can remove ours anytime from the right-click menu."* → **Enable / Not now.** No edit
  happens without consent; declining runs the app inert (idle crab) until enabled from the menu.
- **Hook install:** on consent, `hook_installer` writes a backup (`settings.json.bak`) then
  adds our entries — additive and idempotent (keyed on our binary path), preserving any
  existing hooks (CSB included; both run side by side). Menu item **"Remove hooks"** reverses
  it (restores/strips only ours), mirroring CSB's `uninstall.js`.
- **Standalone runtime:** the hook is a compiled binary — no node/python, no version-pinned
  paths (the failure mode that currently breaks CSB's hook on this machine).
- **Stale-state clearing (stolen from CSB `lifecycle.js`):** force-quit fires `SessionEnd`
  with no `Stop`, freezing an animation. On `SessionStart`/`SessionEnd`, if the current
  `state` belongs to *this* `sessionId` and is `thinking`/`tool`/`permission`, reset to
  `idle`. The `sessionId` gate is load-bearing — warmup churn from another session must not
  wipe a live turn.
- `sessions.d/` = one file per live session (count reserved for a later badge).
- `tool` picks the working-animation variant (§5); `host` powers double-click activation (§6);
  `transcript` powers double-click reveal.
- Staleness guard: on `done`, or if `ts` is older than a small window with no newer event,
  settle to `idle` after the done animation.

## 5. Animation states

Frontend state machine maps the feed to a crab animation. Each animation is a small loop of
hand-coded pixel frames.

| Feed `state` | Animation | Behavior |
|---|---|---|
| `idle` | **rest** | Breathing loop (subtle vertical scale/offset sine). Random blink every 4–8s. Rare skitter-in-place every 20–40s. |
| `thinking` | **think** | Looks up, one claw taps, gentle bob. Continuous. |
| `tool` | **work** | Busy loop. Two variants chosen from `tool`: **run** (Bash/*command* → legs scurry) and **type** (Edit/Write/MultiEdit/NotebookEdit → claws typing). Any other tool → `type` as default. |
| `permission` | **alert** | Turns to face user, both claws raised, "!" speech bubble, soft pulse. Attention-grabbing, not frantic. |
| `done` | **celebrate** | Happy hop for ~1.5s, then transitions to `rest`. |

Transitions: `done` auto-decays to `idle`. Any new feed state interrupts immediately. `pet`
reaction (§6) and `wander` (§7) are behavior overlays on top of `idle`.

### 5.1 Sprite system

- Each frame is a 2D matrix of palette indices (e.g. `0` = transparent, `1` = shell,
  `2` = shadow, `3` = eye/outline, `4` = highlight). Palette is a small array of hex colors:
  a warm Clawd-style orange base, darker orange shadow, near-black outline/eyes, light
  highlight.
- Renderer draws the active frame to `<canvas>` at 1× logical pixels, upscaled with
  `image-rendering: pixelated`. Crab base grid ~24×24 logical px; window scale sets S/M/L.
- `requestAnimationFrame` loop advances frames on per-animation timing (ms per frame).
- Facing direction (left/right) is a horizontal flip of the same frames — used by wander and
  `alert` (face the user/center).

## 6. Window & interactions

Window flags (Tauri v2 `WebviewWindowBuilder`):
`decorations(false)`, `transparent(true)`, `always_on_top(true)`, `shadow(false)`,
`skip_taskbar(true)`, `resizable(false)`, `visible_on_all_workspaces(true)`. Window sized
tightly to the crab at current scale (S ≈ 96px, M ≈ 128px, L ≈ 160px square-ish).

**Click-through for empty pixels:** on `pointermove`, JS reads the canvas alpha at the
cursor. Over transparent pixels → call `set_click_through(true)` (`set_ignore_cursor_events`);
over the crab → `set_click_through(false)`. This lets clicks in the transparent corners pass
to whatever is behind the window.

Interactions:
- **Drag** — press-and-move on the crab repositions the window (Tauri `start_dragging` or a
  drag region gated to opaque pixels). Absolute screen position persisted on drop.
- **Click (single)** — "pet": play a quick wiggle/bounce reaction, then resume prior state.
- **Right-click** — native context menu: `Size ▸ S / M / L`, `Wander when idle ✓`,
  `Activity detection ▸ Enable hooks… / Remove hooks`, `Quit`. (Room to add `Open current
  session` later.)
- **Double-click** — `activate_host`: bring the session's host app to the front. If the
  session is a Claude Desktop session (`host = "Claude"`), activate `Claude.app`; otherwise
  activate the terminal recorded in `host` (`iTerm.app`, `Terminal.app`, VS Code, …) via
  `open -a`. No exact-session targeting — just focus the app running Claude Code. If `host`
  is unknown, fall back to activating Claude.app if running, else no-op. (The `.jsonl`
  transcript path is still recorded and can back a "reveal in Finder" menu item if wanted.)

## 7. Wander-when-idle (v1, behind toggle)

Off by default; toggled in the right-click menu; persisted.

- **User-idle detection:** `idle_monitor` reads macOS `HIDIdleTime`
  (`ioreg -c IOHIDSystem` HIDIdleTime, or core-graphics `CGEventSourceSecondsSinceLastEventType`).
  Crossing the threshold (~60s) emits `user-idle`; the next input emits `user-active`.
- **On `user-idle` (and toggle on):** the wander controller moves the *window* along a
  random walk within the current display's visible bounds, crab plays the `work/run` walk
  cycle, facing its travel direction. Occasional pauses (idle `rest`) between walks.
- **On `user-active`:** crab scurries back to its persisted resting position, then returns to
  the normal reactive state machine.
- Claude-state changes take priority over wander: if the feed goes `thinking`/`tool`/etc.
  while wandering, the crab returns to rest position and reacts.
- Wander never overrides `permission` (must stay visible/attention-grabbing at rest).

## 8. Persistence & config

`config` stores JSON in Tauri's app-config dir:

```json
{ "position": {"x": 1234, "y": 88}, "size": "M", "wanderEnabled": false }
```

Loaded on startup (fallback: bottom-right of primary display, size M, wander off). Saved on
drag-drop, resize, and toggle changes.

## 9. Project structure

```
clawd-pet/
├─ src/                     # frontend
│  ├─ index.html
│  ├─ main.js               # bootstrap, event wiring
│  ├─ sprites.js            # frame matrices + palette + renderer
│  ├─ state-machine.js      # feed state → animation
│  ├─ behavior.js           # micro-life, pet, wander controller
│  ├─ input.js              # drag/click/dblclick/contextmenu + hit-test
│  └─ style.css
├─ src-tauri/
│  ├─ src/
│  │  ├─ main.rs            # window build + wiring
│  │  ├─ hook_installer.rs  # add/remove our hooks in ~/.claude/settings.json
│  │  ├─ state_watcher.rs
│  │  ├─ idle_monitor.rs
│  │  ├─ os_actions.rs
│  │  ├─ config.rs
│  │  └─ bin/
│  │     └─ clawd_pet_hook.rs  # standalone hook handler (bundled binary)
│  ├─ tauri.conf.json
│  └─ Cargo.toml
├─ docs/superpowers/specs/  # this spec
└─ README.md
```

## 10. Tech decisions & rationale

- **Tauri over Electron/Swift:** ~5MB vs ~150MB; web frontend lets us hand-code pixel
  sprites in canvas quickly; Rust gives a proper transparent always-on-top window. Swift
  (CSB's choice) is a fine alternative but slower to build and we don't need native perf for
  a 24×24 sprite.
- **Standalone via a bundled hook binary:** we write our own state feed by installing our own
  Claude Code hooks that call a compiled `clawd-pet-hook` binary. This makes the pet depend on
  nothing but Claude Code itself, needs no node/python, and avoids the version-pinned-`node`
  path that currently breaks CSB's hook on this machine. Trade-off: we auto-edit the user's
  `~/.claude/settings.json`. Mitigation: edits are additive and idempotent (keyed on our
  binary path), preserve all existing hooks, and are cleanly removable from the menu.
- **Own state file, not CSB's:** we read our own `<app-config>/clawd-pet/state.json`, so the
  pet coexists with CSB without coupling to its schema or install state.
- **Hand-coded pixels:** authentic 8-bit, tiny, frame-consistent, no AI-art cleanup, full
  control. Matches how CSB and `clawd-mochi` did it.

## 11. IP / legal note

The crab is **original art**, Clawd-inspired, and the product is **not** named "Clawd".
Anthropic treats the "Clawd" name and image as trademark/IP and has enforced renames
(Clawdbot → Moltbot, Jan 2026). For personal/local use this is a non-issue; if ever
distributed, keep the name and art "inspired-by," not a copy, and avoid the Clawd wordmark.

## 12. Verification plan

- **First-run consent:** confirm no `settings.json` edit occurs before the user clicks
  Enable; declining leaves the file untouched and the crab inert; a backup `settings.json.bak`
  is written on the first accepted edit.
- **Hook install:** run first-launch install against a temp `settings.json` (empty, and one
  already containing CSB hooks); confirm our entries are added once, existing hooks preserved,
  and re-running is a no-op; confirm removal deletes only ours.
- **Hook binary:** pipe sample hook JSON payloads into `clawd-pet-hook` for each event; assert
  the written `state.json` and `sessions.d/` match the §4 mapping (incl. `host` from
  `TERM_PROGRAM`).
- **Sprite/state machine:** headless unit check that each feed state maps to the expected
  animation id and that `done` decays to `idle`.
- **State feed:** manually write test states into `state.json`, confirm the watcher emits and
  the crab switches animation within one debounce window.
- **Window behavior:** launch the app, confirm always-on-top over other windows, transparent
  corners click through, drag persists across restart, resize S/M/L works.
- **Interactions:** click → pet reaction; right-click → menu items function; double-click →
  Finder reveals the current transcript.
- **Wander:** enable toggle, force user-idle (wait past threshold), confirm the crab wanders
  within display bounds and returns to rest on input; confirm Claude-state changes preempt.
- Real end-to-end: run a real Claude Code session and watch the crab track
  thinking→tool→done live.

## 13. Dependencies

- **Only Claude Code** (any surface that reads `~/.claude/settings.json` and fires hooks:
  CLI, desktop app, IDE). No CSB, no node, no python — the hook handler is a bundled binary.
- Writes to `~/.claude/settings.json` only after first-run consent (backup + additive +
  idempotent + removable), and to `<app-config>/clawd-pet/`.
- Build toolchain: `cargo` present; `tauri-cli` not yet installed locally
  (`cargo install tauri-cli` or `npm create tauri-app`). `node`/`npm` present but only for the
  Tauri dev tooling, not at runtime.
