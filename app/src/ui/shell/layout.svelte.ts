// What the map's window has open, and the layout the screen calls for. UI state only: the
// world's modes (playing, sketching, designing, tools armed) stay in App. Phones in portrait get
// a bottom sheet and a tab bar; everything else (short landscape phones too) a dock on the right.
export type Section = 'world' | 'edit' | 'notes' | 'play';
export type WorldTab = 'generate' | 'sketch' | 'library';
export type EditTab = 'names' | 'sites' | 'build' | 'town' | 'scatter' | 'design';
export type NotesTab = 'npcs' | 'plots' | 'places';
export type Tabs = { world: WorldTab; edit: EditTab; notes: NotesTab };
/** How far a phone's sheet is up: its tool strip only, half the screen, or all of it. */
export type Snap = 'peek' | 'half' | 'full';

export const SECTIONS: { key: Section; label: string; icon: 'globe' | 'pencil' | 'book' | 'pawn'; kbd: string }[] = [
  { key: 'world', label: 'World', icon: 'globe', kbd: '1' },
  { key: 'edit', label: 'Edit', icon: 'pencil', kbd: '2' },
  { key: 'notes', label: 'Notes', icon: 'book', kbd: '3' },
  { key: 'play', label: 'Play', icon: 'pawn', kbd: '4' },
];

/** A question before work would be dropped: new sketch strokes, or a site's unsaved design.
 * `proceed` carries on once the work is dealt with; `generate`: the sketch can be generated. */
export interface Ask {
  kind: 'sketch' | 'design';
  proceed: () => void;
  generate?: boolean;
}

const PHONE ='(max-width: 720px) and (min-height: 501px)';
const SHORT = '(max-height: 500px)';

function stored(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function store(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Private mode: the choice just isn't remembered.
  }
}

/** A benchmark run shows the developer stats; otherwise as this viewer left them (off at first). */
const bench = typeof location !== 'undefined' && new URLSearchParams(location.search).has('bench');

class Shell {
  /** The section whose panel is open. */
  section = $state<Section | null>(null);
  /** Each section's tab, kept while the panel is shut. */
  tabs = $state<Tabs>({ world: 'generate', edit: 'names', notes: 'npcs' });
  /** Desktop: the dock folded down to its header and tool strip. */
  collapsed = $state(false);
  snap = $state<Snap>('half');
  /** Phones: the sheet shows the selected place's card over the open panel. */
  card = $state(false);
  menu = $state<'main' | 'layers' | null>(null);
  /** The keyboard shortcuts overlay. */
  help = $state(false);
  stats = $state(bench || stored('ui.stats') === '1');
  ask = $state.raw<Ask | null>(null);
  phone = $state(false);
  short = $state(false);

  constructor() {
    if (typeof window === 'undefined') return;
    const phone = matchMedia(PHONE);
    const short = matchMedia(SHORT);
    const apply = () => {
      this.phone = phone.matches;
      this.short = short.matches;
      document.documentElement.dataset.layout = this.phone ? 'phone' : 'wide';
      if (this.short) document.documentElement.dataset.short = '';
      else delete document.documentElement.dataset.short;
    };
    apply();
    phone.addEventListener('change', apply);
    short.addEventListener('change', apply);
  }

  setStats(on: boolean) {
    this.stats = on;
    if (!bench) store('ui.stats', on ? '1' : '0');
  }
}

export const shell = new Shell();

/** Groups a panel keeps open or shut, as this viewer left them. */
export function remembered(key: string, fallback: boolean): boolean {
  const v = stored(`ui.open.${key}`);
  return v === null ? fallback : v === '1';
}

export function remember(key: string, open: boolean) {
  store(`ui.open.${key}`, open ? '1' : '0');
}
