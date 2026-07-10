// Maps Claude Code feed states to crab animations, and runs the idle micro-life
// scheduler: while idle the crab stands still and occasionally blinks, shuffles,
// stretches or peeks — no constant bobbing.
// Feed contract (state.json): { state: idle|thinking|tool|permission|done, tool?: string }

const DONE_MS = 1500; // celebrate length before settling back to rest

const MICRO = ["blink", "blink", "blink", "look", "look", "shuffle", "stretch", "peek", "wave"]; // blink/look-weighted, wave rare
const MICRO_MIN_MS = 3000;
const MICRO_MAX_MS = 9000;
const SLEEP_AFTER_MS = 90_000; // continuous idle before nodding off

export class StateMachine {
  constructor(renderer) {
    this.r = renderer;
    this.state = "idle";
    this._decay = null;
    this._micro = null;
    // Must start "now": the boot state is idle→idle so apply() never stamps it,
    // and an unset value made the sleep check pass instantly (insta-narcolepsy).
    this._idleSince = Date.now();
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
      // Long uninterrupted idle → fall asleep (loops until anything wakes him).
      if (Date.now() - (this._idleSince || 0) > SLEEP_AFTER_MS) {
        this._micro = -1;
        this._sleeping = true;
        this.r.play("sleep");
        return;
      }
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
    // Repeat idle events (hook chatter) must not reset the micro-life timer,
    // cancel an active hover, or wake a sleeping crab.
    if (next === "idle" && this.state === "idle" && (this._micro || this._hovering || this._sleeping)) {
      if (this._hovering) this.r.play("hover"); // e.g. came home hovered mid-walk-anim
      return;
    }
    clearTimeout(this._decay);
    this._stopMicro();
    if (next !== "idle" || this.state !== "idle") this._idleSince = Date.now();
    this._sleeping = false; // any real event wakes him
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
        // Any tool: he sits at his laptop and types.
        this.r.play("work");
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
      this._sleeping = false; // a looming cursor wakes him
      this._idleSince = Date.now();
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
    this._sleeping = false;
    this._idleSince = Date.now();
    this.r.play("celebrate");
    clearTimeout(this._decay);
    this._decay = setTimeout(() => this.apply(prev), 700);
  }
}
