// The world's undo history (the steps `App.applyEdits` records), with counts and labels the
// undo and redo buttons can follow.
import type { Step } from '../../sync/history';

const LIMIT = 100;

export class UndoHistory {
  private done: Step[] = [];
  private undone: Step[] = [];
  /** Bumped on every change, so what reads the history follows it. */
  version = $state(0);

  get canUndo(): boolean {
    void this.version;
    return this.done.length > 0;
  }

  get canRedo(): boolean {
    void this.version;
    return this.undone.length > 0;
  }

  get undoLabel(): string | null {
    void this.version;
    return this.done.at(-1)?.label ?? null;
  }

  get redoLabel(): string | null {
    void this.version;
    return this.undone.at(-1)?.label ?? null;
  }

  /** A new step: what was undone can't be redone any more. */
  push(step: Step) {
    this.done.push(step);
    if (this.done.length > LIMIT) this.done.shift();
    this.undone.length = 0;
    this.version++;
  }

  /** Take `step` (else the last) off to undo it; null if it isn't in the history. */
  takeUndo(step = this.done.at(-1)): Step | null {
    const i = step ? this.done.indexOf(step) : -1;
    if (i < 0) return null;
    this.done.splice(i, 1);
    this.undone.push(step!);
    this.version++;
    return step!;
  }

  /** Forget every step (another world opened: its edits are not these). */
  clear() {
    this.done.length = 0;
    this.undone.length = 0;
    this.version++;
  }

  takeRedo(): Step | null {
    const step = this.undone.pop();
    if (!step) return null;
    this.done.push(step);
    this.version++;
    return step;
  }
}
