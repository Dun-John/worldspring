// A component loaded on first use, so it is its own chunk and not in the app's first download:
// `.c` is null until the module has come (the first read asks for it), then the component, kept
// for every later mount (shown at once, as an eagerly imported one is).
export function lazy<C>(load: () => Promise<{ default: C }>): { readonly c: C | null } {
  let c = $state.raw<C | null>(null);
  let asked = false;
  return {
    get c() {
      if (!asked) {
        asked = true;
        load().then(
          (m) => (c = m.default),
          (e) => ((asked = false), console.error(e)),
        );
      }
      return c;
    },
  };
}
