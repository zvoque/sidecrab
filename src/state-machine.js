// Maps Claude Code feed states to crab animations.
// Feed contract (state.json): { state: idle|thinking|tool|permission|done, tool?: string }

const TYPE_TOOLS = new Set(["Edit", "Write", "MultiEdit", "NotebookEdit"]);
const DONE_MS = 1500; // celebrate length before settling back to rest

export class StateMachine {
  constructor(renderer) {
    this.r = renderer;
    this.state = "idle";
    this._decay = null;
  }

  apply({ state, tool } = {}) {
    clearTimeout(this._decay);
    this.state = state || "idle";
    switch (this.state) {
      case "thinking":
        this.r.play("think");
        break;
      case "tool":
        // Bash scurries; edit-ish tools (and anything unknown) type.
        this.r.play(tool === "Bash" ? "run" : "type");
        break;
      case "permission":
        this.r.play("alert");
        break;
      case "done":
        this.r.play("celebrate");
        this._decay = setTimeout(() => {
          this.state = "idle";
          this.r.play("rest");
        }, DONE_MS);
        break;
      default:
        this.r.play("rest");
    }
  }

  current() {
    return this.state;
  }
}
