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
- Animate in real time from Claude Code activity, with **no new hook installation** — reuse
  the state feed that CSB already writes on this machine.
- Authentic 8-bit look via hand-coded pixel sprites (our own art).
- Small footprint (Tauri, ~5MB) and negligible idle CPU.
- Delightful interactions: pet it, reposition it, right-click menu, jump to session.
- Optional wander-when-user-idle behavior.

**Non-Goals (v1)**
- Not a status *bar* — no menu-bar item (CSB already does that).
- No telemetry/network calls.
- No use of Anthropic's trademarked "Clawd" name or its official (never-released) asset.
  Art is original, Clawd-*inspired*. Product is not named "Clawd".
- Deferred to later: session-count badge, auto-launch at login, per-tool animations beyond
  the two working variants, sound.

## 3. Architecture

Tauri v2 app. Rust backend owns the OS window and filesystem watching; the frontend is
vanilla HTML/CSS/JS with a `<canvas>` for the pixel crab. No frontend framework.

```
┌─────────────────────────────────────────────────────────┐
│ Rust backend (src-tauri)                                 │
│  • creates the floating window (always-on-top,           │
│    transparent, borderless, all-Spaces, skip-taskbar)    │
│  • watches ~/.claude/statusbar/state.json (notify crate) │
│    → emits `claude-state` event to the webview           │
│  • polls macOS HIDIdleTime → emits `user-idle` / `user-active` │
│  • commands: set_ignore_cursor_events, resize, reveal    │
│    transcript, focus terminal, persist/load config       │
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
| `state_watcher` (Rust) | Watch `state.json`, debounce, parse, emit `claude-state` | notify, serde_json, Tauri emit |
| `idle_monitor` (Rust) | Poll `HIDIdleTime`, emit `user-idle`/`user-active` on threshold crossing | core-foundation/`ioreg`, Tauri emit |
| `os_actions` (Rust) | `reveal_transcript`, `focus_terminal`, `set_click_through`, `resize_window` commands | Tauri, `open`/AppleScript |
| `config` (Rust) | Load/save `{position, size, wanderEnabled}` JSON in app-config dir | tauri app dir, serde |
| `sprites` (JS) | Frame data (pixel matrices) + canvas renderer | none |
| `state-machine` (JS) | Map `claude-state` → active animation; handle transitions (e.g. done→idle) | sprites |
| `behavior` (JS) | Micro-life timers, pet reaction, wander pathing | state-machine, sprites |
| `input` (JS) | drag / click / dblclick / contextmenu; alpha hit-test → click-through | os_actions commands |

Each unit is independently testable: sprite frames render deterministically; the state
machine is a pure map from state string → animation id; config round-trips JSON.

## 4. State feed contract (already exists)

CSB atomically writes `~/.claude/statusbar/state.json` on every Claude Code hook:

```json
{
  "state": "idle | thinking | tool | permission | done",
  "label": "Running command",
  "tool":  "Bash",
  "project": "doomgeneric",
  "sessionId": "f76126bb-…",
  "transcript": "/Users/…/<sessionId>.jsonl",
  "startedAt": 1782689277,
  "ts": 1782689297
}
```

- The pet is a **pure consumer**. It never writes this file and installs no hooks.
- `sessions.d/` contains one empty file per live session (count reserved for later badge).
- `tool` is used to pick the working-animation variant (see §5).
- `transcript` powers double-click → reveal.
- Staleness guard: if `state === "done"` or the file's `ts` is older than a small window
  and no newer event arrives, treat as `idle` after the done-animation completes.

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
  `Quit`. (Room to add `Open current session` later.)
- **Double-click** — `reveal_transcript` (`open -R <transcript>`; reveals the `.jsonl` in
  Finder) and best-effort `focus_terminal` (AppleScript to activate Terminal.app or iTerm).
  Cross-terminal session targeting is not guaranteed; reveal is the reliable part.

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
│  │  ├─ state_watcher.rs
│  │  ├─ idle_monitor.rs
│  │  ├─ os_actions.rs
│  │  └─ config.rs
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
- **Reuse CSB's `state.json`:** the exact activity feed already runs on this machine (hooks
  in `settings.json`). Consuming it means zero setup for the user and one source of truth.
  Trade-off: hard dependency on CSB being installed and its schema. Mitigation: schema is
  simple and stable; document the dependency; degrade to `idle` if the file is missing.
- **Hand-coded pixels:** authentic 8-bit, tiny, frame-consistent, no AI-art cleanup, full
  control. Matches how CSB and `clawd-mochi` did it.

## 11. IP / legal note

The crab is **original art**, Clawd-inspired, and the product is **not** named "Clawd".
Anthropic treats the "Clawd" name and image as trademark/IP and has enforced renames
(Clawdbot → Moltbot, Jan 2026). For personal/local use this is a non-issue; if ever
distributed, keep the name and art "inspired-by," not a copy, and avoid the Clawd wordmark.

## 12. Verification plan

- **Sprite/state machine:** headless unit check that each feed state maps to the expected
  animation id and that `done` decays to `idle`.
- **State feed:** manually write test states into a temp `state.json`, confirm the watcher
  emits and the crab switches animation within one debounce window.
- **Window behavior:** launch the app, confirm always-on-top over other windows, transparent
  corners click through, drag persists across restart, resize S/M/L works.
- **Interactions:** click → pet reaction; right-click → menu items function; double-click →
  Finder reveals the current transcript.
- **Wander:** enable toggle, force user-idle (wait past threshold), confirm the crab wanders
  within display bounds and returns to rest on input; confirm Claude-state changes preempt.
- Real end-to-end: run a real Claude Code session and watch the crab track
  thinking→tool→done live.

## 13. Open dependencies

- Requires CSB installed (writes `~/.claude/statusbar/state.json`). Already present on this
  machine. If a fresh machine lacks it, document installing CSB, or (future) ship our own
  minimal hook writer as a fallback.
- `tauri-cli` not yet installed locally (`cargo install tauri-cli` or `npm create tauri-app`).
  `cargo`, `node`, `npm` are present.
