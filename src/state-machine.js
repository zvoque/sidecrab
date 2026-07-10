// Maps Claude Code feed states to crab animations, and runs the idle micro-life
// scheduler: while idle the crab stands still and occasionally blinks, shuffles,
// stretches or peeks — no constant bobbing.
// Feed contract (state.json): { state: idle|thinking|tool|permission|done, tool?: string }

const RUN_TOOLS = new Set(["Bash"]);
const DONE_MS = 1500; // celebrate length before settling back to rest

const MICRO = ["blink", "blink", "blink", "shuffle", "stretch", "peek"]; // blink-weighted
const MICRO_MIN_MS = 3000;
const MICRO_MAX_MS = 9000;

export class StateMachine {
  constructor(renderer) {
    this.r = renderer;
    this.state = "idle";
    this._decay = null;
    this._micro = null;
  }

  _stopMicro() {
    clearTimeout(this._micro);
    this._micro = null;
  }

  _scheduleMicro() {
    this._stopMicro();
    const delay = MICRO_MIN_MS + Math.random() * (MICRO_MAX_MS - MICRO_MIN_MS);
    this._micro = setTimeout(() => {
      if (this.state !== "idle" || this._microPaused) return;
      this._micro = -1; // sentinel: one-shot in flight, keep the repeat-idle guard on
      const anim = MICRO[Math.floor(Math.random() * MICRO.length)];
      this.r.play(anim, () => this._scheduleMicro()); // one-shot, then re-arm
    }, delay);
  }

  /// Wander drives the window; a micro-anim mid-walk would freeze the legs while
  /// the window still moves. Behavior pauses idle life for the whole excursion.
  pauseIdleLife() {
    this._microPaused = true;
    this._stopMicro();
  }

  /// Unpause only — the caller re-applies state right after, which restores the
  /// proper animation AND re-arms the micro timer (arming here first would trip
  /// apply()'s repeat-idle guard and leave the walk animation frozen).
  resumeIdleLife() {
    this._microPaused = false;
  }

  apply({ state, tool } = {}) {
    const next = state || "idle";
    // Repeat idle events (hook chatter) must not reset the micro-life timer
    // (or cancel an active hover), or the crab never gets around to blinking.
    if (next === "idle" && this.state === "idle" && (this._micro || this._hovering)) {
      if (this._hovering) this.r.play("hover"); // e.g. came home hovered mid-walk-anim
      return;
    }
    clearTimeout(this._decay);
    this._stopMicro();
    this.state = next;
    this._lastTool = tool;
    // done still decays to idle even while hover has hijacked the visuals.
    if (next === "done") {
      this._decay = setTimeout(() => this.apply({ state: "idle" }), DONE_MS);
    }
    if (this._hovering) {
      this.r.play("hover"); // hover hijacks whatever would play; state tracks underneath
      return;
    }
    if (this._traveling) return; // behavior owns the visuals while walking somewhere
    switch (next) {
      case "thinking":
        this.r.play("think");
        break;
      case "tool":
        // Bash gets the in-place busy scurry (the walk cycle is reserved for
        // actual travel); edit-ish tools fidget-type.
        this.r.play(RUN_TOOLS.has(tool) ? "busy" : "type");
        break;
      case "permission":
        this.r.play("alert");
        break;
      case "done":
        this.r.play("celebrate");
        break;
      default:
        this.r.play("rest");
        this._scheduleMicro();
    }
  }

  current() {
    return this.state;
  }

  /// Behavior sets this while it walks the window somewhere: state keeps tracking
  /// (and hover still hijacks), but hook events must not swap the walk animation
  /// out from under a moving window.
  setTraveling(on) {
    this._traveling = !!on;
  }

  /// Cursor over the crab: hover hijacks whatever is playing; leaving restores
  /// the animation the current state calls for.
  setHover(on) {
    on = !!on;
    if (on === this._hovering) return;
    this._hovering = on;
    if (on) {
      this._stopMicro();
      this.r.play("hover");
    } else if (this._traveling) {
      // walkTo reclaims the walk animation on its next tick
    } else {
      this.apply({ state: this.state, tool: this._lastTool });
    }
  }

  /// Petting reaction: quick happy hop, then back to whatever was happening.
  pet() {
    const prev = { state: this.state, tool: this._lastTool };
    this._stopMicro();
    this.r.play("celebrate");
    clearTimeout(this._decay);
    this._decay = setTimeout(() => this.apply(prev), 700);
  }
}
