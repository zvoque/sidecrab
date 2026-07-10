// App entry. Renderer + state machine, driven by the Rust `claude-state` feed when
// running inside Tauri. In a plain browser (art iteration) keys 1-6 cycle states.
import { SpriteRenderer } from "./sprites.js";
import { StateMachine } from "./state-machine.js";
import { attachInput } from "./input.js";
import { attachBehavior } from "./behavior.js";

const IN_TAURI = typeof window.__TAURI__ !== "undefined";

window.addEventListener("DOMContentLoaded", async () => {
  const canvas = document.getElementById("crab");
  const renderer = new SpriteRenderer(canvas);
  const sm = new StateMachine(renderer);
  sm.apply({ state: "idle" });
  renderer.start();

  if (IN_TAURI) {
    let host = "Claude"; // last seen host app, for double-click activation
    attachInput({ renderer, sm, getHost: () => host });
    const behavior = attachBehavior({ renderer, sm });
    const { listen } = window.__TAURI__.event;
    const { invoke } = window.__TAURI__.core;
    invoke("get_config").then((c) => c && renderer.setHat(c.hat));
    await listen("hat-changed", (e) => renderer.setHat(e.payload));
    await listen("claude-state", (e) => {
      if (e.payload?.host) host = e.payload.host;
      behavior.onClaudeState(e.payload); // wander preemption before the anim swap
      sm.apply(e.payload);
    });
    // crab-hover is consumed by behavior.js (forwarding + harassment detection)
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
