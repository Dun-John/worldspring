// The sprites' drawing code (sprites.ts) is a chunk of its own, loaded once a world opens (`MapView.loadWorld`,
// `warmBattle`): the battlemap and interior layers, made only after that, read it through `sprites()`.
type Code = typeof import('./sprites');
let code: Code | null = null;
let loading: Promise<Code> | null = null;

/** Loads the drawing code (once; later calls return the same promise). */
export function loadSprites(): Promise<Code> {
  // (A failed fetch is tried again by the next call.)
  return (loading ??= import('./sprites').then(
    (m) => (code = m),
    (e) => {
      loading = null;
      throw e;
    },
  ));
}

/** The drawing code, loaded by `loadSprites` before anything draws a sprite. */
export function sprites(): Code {
  if (!code) throw new Error('sprites drawn before their code was loaded (loadSprites)');
  return code;
}
