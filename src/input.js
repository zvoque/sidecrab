// Pointer interactions (Tauri only): manual drag (keeps click/dblclick semantics),
// click-to-pet, double-click → activate host app, right-click → native menu.
// Click-through for empty pixels is handled by a Rust-side cursor poller fed with
// the sprite's opaque rect via set_opaque_rect.

const CLICK_MS = 250; // single-click fires only if no second click lands in time
const DRAG_PX = 4;    // movement below this is a click, not a drag

export function attachInput({ renderer, sm, getHost }) {
  const { invoke } = window.__TAURI__.core;
  const stage = document.getElementById("stage");

  const pushBounds = () => {
    const b = renderer.bounds();
    invoke("set_opaque_rect", { x0: b.x0, y0: b.y0, x1: b.x1, y1: b.y1 });
  };
  pushBounds();
  // Re-push when the animation changes (bounds differ per anim, e.g. raised claws).
  const origPlay = renderer.play.bind(renderer);
  renderer.play = (anim) => {
    origPlay(anim);
    pushBounds();
  };

  let down = null; // {x, y, dragging}
  let clickTimer = null;

  stage.addEventListener("pointerdown", (e) => {
    if (e.button !== 0) return;
    down = { x: e.screenX, y: e.screenY, dragging: false };
    stage.setPointerCapture(e.pointerId);
  });

  stage.addEventListener("pointermove", (e) => {
    if (!down) return;
    const dx = e.screenX - down.x;
    const dy = e.screenY - down.y;
    if (!down.dragging && Math.hypot(dx, dy) < DRAG_PX) return;
    if (!down.dragging) {
      down.dragging = true;
      invoke("set_drag_lock", { locked: true });
    }
    down.x = e.screenX;
    down.y = e.screenY;
    invoke("move_window_by", { dx: Math.round(dx), dy: Math.round(dy) });
  });

  stage.addEventListener("pointerup", (e) => {
    if (e.button !== 0 || !down) return;
    const wasDrag = down.dragging;
    down = null;
    if (wasDrag) {
      invoke("set_drag_lock", { locked: false });
      invoke("persist_position");
      return;
    }
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
