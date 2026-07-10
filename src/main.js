// App entry. Renderer + state machine, driven by the Rust `claude-state` feed when
// running inside Tauri. In a plain browser (art iteration) keys 1-6 cycle states.
import { SpriteRenderer } from "./sprites.js";
import { StateMachine } from "./state-machine.js";

const IN_TAURI = typeof window.__TAURI__ !== "undefined";

window.addEventListener("DOMContentLoaded", async () => {
  const canvas = document.getElementById("crab");
  const renderer = new SpriteRenderer(canvas);
  const sm = new StateMachine(renderer);
  sm.apply({ state: "idle" });
  renderer.start();

  if (IN_TAURI) {
    const { listen } = window.__TAURI__.event;
    await listen("claude-state", (e) => sm.apply(e.payload));
  } else {
    // Browser-only dev cycler for eyeballing animations.
    const DEV = [
      { state: "idle" },
      { state: "thinking" },
      { state: "tool", tool: "Bash" },
      { state: "tool", tool: "Edit" },
      { state: "permission" },
      { state: "done" },
    ];
    window.addEventListener("keydown", (e) => {
      const n = parseInt(e.key, 10);
      if (n >= 1 && n <= DEV.length) sm.apply(DEV[n - 1]);
    });
  }

  window.__crab = { renderer, sm }; // dev handle
});
