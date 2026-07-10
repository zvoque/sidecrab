// Wander-when-idle: when the USER goes idle (and the toggle is on) the crab takes
// random walks around the current display; on user input it walks back home.
//
// Invariants:
// - The window NEVER moves unless the walk animation is playing.
// - Hover PAUSES any excursion (crouch in place); it never cancels it.
// - `home` is stable: the dragged/persisted spot (or the startup position), never
//   a mid-wander location. A displaced idle crab always walks itself home.

const STEP_MS = 28;
const WANDER_SPEED = 3; // px per step
const HOME_SPEED = 6;   // scurry home faster
const HOME_EPS = 8;     // px: closer than this counts as home

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export function attachBehavior({ renderer, sm }) {
  const { invoke } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;

  // Awaited IPC with a timeout. The native context menu runs a nested run loop on
  // the main thread and can drop an invoke response — a bare await then hangs
  // forever and wedges the driver (crab stranded, ticker dead). Bounded await
  // means the loops always re-check their conditions and recover.
  const call = (cmd, args) =>
    Promise.race([invoke(cmd, args), sleep(2000).then(() => null)]);

  let enabled = false;
  let mode = "off"; // off | wander | home
  let driving = false;
  let hovering = false;
  let home = null; // {x, y} stable resting position

  // Capture the startup spot as home until a drag defines one.
  call("get_geometry").then((g) => {
    if (g && !home) home = { x: g.winX, y: g.winY };
  });
  call("get_config").then((c) => c && (enabled = c.wanderEnabled));
  listen("wander-changed", (e) => {
    enabled = !!e.payload;
    if (!enabled && mode === "wander") mode = "home"; // toggled off mid-excursion
  });
  listen("crab-hover", (e) => (hovering = !!e.payload));
  listen("user-active", () => {
    if (mode === "wander") mode = "home";
  });

  // Ticker replaces one-shot events: re-checks wander eligibility, syncs home with
  // the persisted (dragged) position, and walks a stranded crab back home.
  // Homing does NOT wait for Claude to go idle — during an active session the
  // crab is almost never state-idle, and a stranded crab must still come home.
  setInterval(async () => {
    if (driving || hovering) return;
    const c = await call("get_config");
    if (c?.position) home = { x: c.position[0], y: c.position[1] };
    const g = await call("get_geometry");
    if (!g) return;
    if (!home) home = { x: g.winX, y: g.winY };
    if (Math.hypot(g.winX - home.x, g.winY - home.y) > HOME_EPS) {
      mode = "home"; // stranded (hover mid-wander, Claude preemption, …) → go home
      drive();
      return;
    }
    if (
      enabled &&
      mode === "off" &&
      sm.current() === "idle" &&
      (await call("user_is_idle")) === true
    ) {
      mode = "wander";
      drive();
    }
  }, 3000);

  /// Claude went busy mid-wander: abandon sightseeing and head home (walking —
  /// reactions play at home after arrival). Homing itself is never interrupted.
  function onClaudeState(payload) {
    if (payload?.state && payload.state !== "idle" && mode === "wander") mode = "home";
  }

  /// Step the window toward (tx,ty). Hover and a mid-pet hop pause in place;
  /// otherwise the walk claims the animation immediately and moves. Arrival
  /// snaps to the exact target so "home" is pixel-accurate.
  async function walkTo(tx, ty, speed, live) {
    const g = await call("get_geometry");
    if (!g) return;
    let { winX: x, winY: y } = g;
    let hopWait = 0; // bounded pause for a pet hop; can't wedge the journey
    while (live()) {
      if (hovering || (renderer.anim === "celebrate" && hopWait < 1200)) {
        hopWait += hovering ? 0 : 150;
        await sleep(150); // crouching under the cursor / finishing a pet hop
        continue;
      }
      if (renderer.anim !== "walk") {
        renderer.play("walk");
        renderer.setFacing(tx < x ? "left" : "right");
        hopWait = 0;
      }
      if (Math.hypot(tx - x, ty - y) <= speed) {
        invoke("set_window_pos", { x: Math.round(tx), y: Math.round(ty) });
        return;
      }
      const ang = Math.atan2(ty - y, tx - x);
      x += Math.cos(ang) * speed;
      y += Math.sin(ang) * speed;
      invoke("set_window_pos", { x: Math.round(x), y: Math.round(y) });
      await sleep(STEP_MS);
    }
  }

  async function drive() {
    if (driving) return;
    driving = true;
    sm.pauseIdleLife();
    sm.setTraveling(true); // hook events track state but leave the walk anim alone

    while (mode === "wander") {
      const g = await call("get_geometry");
      if (!g) break;
      const tx = g.monX + Math.random() * Math.max(1, g.monW - g.winW);
      const ty = g.monY + Math.random() * Math.max(1, g.monH - g.winH);
      await walkTo(tx, ty, WANDER_SPEED, () => mode === "wander");
      if (mode !== "wander") break;
      renderer.play("rest");
      for (let t = 0; t < 2000 + Math.random() * 3000 && mode === "wander"; t += 100) {
        await sleep(100);
      }
    }

    if (mode === "home" && home) {
      await walkTo(home.x, home.y, HOME_SPEED, () => mode === "home");
    }
    mode = "off";
    driving = false;
    renderer.setFacing("right");
    sm.setTraveling(false);
    sm.resumeIdleLife();
    sm.apply({ state: sm.current(), tool: sm._lastTool }); // arrival: react properly
  }

  return { onClaudeState };
}
