// App entry. Renderer + state machine; the live Claude-state feed (Task 5) and
// interactions (Task 6) attach here.
import { SpriteRenderer } from "./sprites.js";
import { StateMachine } from "./state-machine.js";

const IN_TAURI = typeof window.__TAURI__ !== "undefined";

window.addEventListener("DOMContentLoaded", () => {
  const canvas = document.getElementById("crab");
  const renderer = new SpriteRenderer(canvas);
  const sm = new StateMachine(renderer);
  sm.apply({ state: "idle" });
  renderer.start();

  // Dev cycler (removed when the live feed lands, Task 5): keys 1-5 drive states.
  const DEV_STATES = [
    { state: "idle" },
    { state: "thinking" },
    { state: "tool", tool: "Bash" },
    { state: "tool", tool: "Edit" },
    { state: "permission" },
    { state: "done" },
  ];
  window.addEventListener("keydown", (e) => {
    const n = parseInt(e.key, 10);
    if (n >= 1 && n <= DEV_STATES.length) sm.apply(DEV_STATES[n - 1]);
  });

  window.__crab = { renderer, sm }; // dev handle
  if (IN_TAURI) console.log("Clawd Pet running in Tauri");
});
