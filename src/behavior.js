// Wander-when-idle: when the USER goes idle (and the toggle is on) the crab takes
// random walks around the current display; on user input it scurries back to its
// resting spot. Claude activity always preempts — the crab teleports home and reacts.

const STEP_MS = 28;
const WANDER_SPEED = 3; // px per step
const HOME_SPEED = 6;   // scurry home faster

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export function attachBehavior({ renderer, sm }) {
  const { invoke } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;

  let enabled = false;
  let mode = "off"; // off | wander | home
  let driving = false;
  let home = null; // {x, y} resting position to return to

  invoke("get_config").then((c) => (enabled = c.wanderEnabled));
  listen("wander-changed", (e) => {
    enabled = !!e.payload;
    if (!enabled) abortToHome(true);
  });
  listen("user-active", () => {
    if (mode === "wander") mode = "home";
  });
  // Poll instead of relying on the one-shot user-idle transition event: the crab is
  // often mid-animation (Claude busy) at that exact moment and must re-check later.
  setInterval(async () => {
    if (!enabled || mode !== "off" || driving || sm.current() !== "idle") return;
    if (await invoke("user_is_idle")) {
      mode = "wander";
      drive();
    }
  }, 3000);

  /// Claude went busy mid-wander: snap home immediately so the reaction is visible
  /// where the user expects the crab to live.
  function onClaudeState(payload) {
    if (payload?.state && payload.state !== "idle" && mode !== "off") abortToHome(true);
  }

  function abortToHome(teleport) {
    mode = "off";
    if (teleport && home) invoke("set_window_pos", { x: home.x, y: home.y });
    renderer.setFacing("right");
  }

  async function walkTo(tx, ty, speed, live) {
    renderer.play("walk");
    const g = await invoke("get_geometry");
    if (!g) return;
    let { winX: x, winY: y } = g;
    renderer.setFacing(tx < x ? "left" : "right");
    while (live() && Math.hypot(tx - x, ty - y) > speed) {
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
    const g = await invoke("get_geometry");
    if (!g) {
      driving = false;
      mode = "off";
      return;
    }
    home = { x: g.winX, y: g.winY };

    while (mode === "wander") {
      const tx = g.monX + Math.random() * Math.max(1, g.monW - g.winW);
      const ty = g.monY + Math.random() * Math.max(1, g.monH - g.winH);
      await walkTo(tx, ty, WANDER_SPEED, () => mode === "wander");
      if (mode !== "wander") break;
      renderer.play("rest");
      // Pause between walks; bail early if anything changes.
      for (let t = 0; t < 2000 + Math.random() * 3000 && mode === "wander"; t += 100) {
        await sleep(100);
      }
    }

    if (mode === "home" && home) {
      await walkTo(home.x, home.y, HOME_SPEED, () => mode === "home");
      invoke("set_window_pos", { x: home.x, y: home.y });
    }
    mode = "off";
    driving = false;
    renderer.setFacing("right");
    sm.apply({ state: sm.current() }); // restore the proper animation
  }

  return { onClaudeState };
}
