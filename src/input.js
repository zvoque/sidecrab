// Pointer interactions (Tauri only): native OS drag (buttery, no IPC per move),
// click-to-pet, double-click → activate host app, right-click → native menu.
// Click-through for empty pixels is handled by a Rust-side cursor poller fed with
// the sprite's opaque rect via set_opaque_rect.

const CLICK_MS = 250; // single-click fires only if no second click lands in time
const DRAG_PX = 4;    // movement below this is a click, not a drag
const MOVE_SETTLE_MS = 400; // after the last `moved` event, persist + unlock

export function attachInput({ renderer, sm, getHost }) {
  const { invoke } = window.__TAURI__.core;
  const appWindow = window.__TAURI__.window.getCurrentWindow();
  const stage = document.getElementById("stage");

  // Being carried: full-speed panic scramble, randomly flipping direction.
  let flipTimer = null;
  const startPanic = () => {
    sm.setTraveling(true); // hook events must not swap the panic anim mid-drag
    sm.pauseIdleLife();
    renderer.play("panic");
    flipTimer = setInterval(
      () => renderer.setFacing(Math.random() < 0.5 ? "left" : "right"),
      300 + Math.random() * 300
    );
  };
  const endPanic = () => {
    clearInterval(flipTimer);
    flipTimer = null;
    renderer.setFacing("right");
    sm.setTraveling(false);
    sm.resumeIdleLife();
    sm.apply({ state: sm.current(), tool: sm._lastTool });
  };

  const pushBounds = () => {
    const b = renderer.bounds();
    invoke("set_opaque_rect", { x0: b.x0, y0: b.y0, x1: b.x1, y1: b.y1 });
  };
  pushBounds();

  let down = null; // pointer-down origin, null once a drag starts
  let clickTimer = null;
  let dragActive = false;
  let settleTimer = null;
  let manual = null; // fallback drag state when native startDragging is unavailable

  // Native drag emits window `moved` events; once they settle, save the new home.
  appWindow.onMoved(() => {
    if (!dragActive || manual) return; // wander/manual moves handled elsewhere
    clearTimeout(settleTimer);
    settleTimer = setTimeout(() => {
      dragActive = false;
      endPanic();
      invoke("persist_position"); // wherever you leave him = his new home
      invoke("set_drag_lock", { locked: false });
    }, MOVE_SETTLE_MS);
  });

  stage.addEventListener("pointerdown", (e) => {
    if (e.button !== 0) return;
    down = { x: e.screenX, y: e.screenY };
    stage.setPointerCapture(e.pointerId);
  });

  stage.addEventListener("pointermove", (e) => {
    if (manual) {
      // Fallback drag: position the window ourselves (physical px).
      const dpr = window.devicePixelRatio || 1;
      invoke("set_window_pos", {
        x: Math.round(manual.x + (e.screenX - manual.sx) * dpr),
        y: Math.round(manual.y + (e.screenY - manual.sy) * dpr),
      });
      return;
    }
    if (!down) return;
    if (Math.hypot(e.screenX - down.x, e.screenY - down.y) < DRAG_PX) return;
    const start = down;
    down = null;
    dragActive = true;
    startPanic();
    invoke("set_drag_lock", { locked: true });
    // Native OS drag preferred; if the call is rejected, fall back to manual.
    appWindow.startDragging().catch(async () => {
      const g = await invoke("get_geometry");
      if (g) manual = { x: g.winX, y: g.winY, sx: start.x, sy: start.y };
    });
  });

  stage.addEventListener("pointerup", (e) => {
    if (e.button !== 0) return;
    if (manual) {
      // Fallback drag ends: this spot is the new home.
      manual = null;
      dragActive = false;
      endPanic();
      invoke("persist_position");
      invoke("set_drag_lock", { locked: false });
      return;
    }
    if (!down) return;
    down = null;
    // Click vs double-click disambiguation.
    if (clickTimer) {
      clearTimeout(clickTimer);
      clickTimer = null;
      invoke("activate_host", { host: getHost() });
    } else {
      clickTimer = setTimeout(() => {
        clickTimer = null;
        sm.pet();
      }, CLICK_MS);
    }
  });

  stage.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    invoke("show_menu");
  });
}
