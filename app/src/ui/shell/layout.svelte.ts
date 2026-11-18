// What the map's window has open, and the layout the screen calls for. UI state only: the
// world's modes (playing, sketching, designing, tools armed) stay in App. Phones in portrait get
// a bottom sheet and a tab bar; everything else (short landscape phones too) a dock on the right.
import { EDIT_TOOLS, type GroupKey, type ToolKey } from './editTools';

/** `?ui=new`: Edit grouped by thing (Roadmap 6 R8), until the user approves it. Every switch on
 * it is `if (NEW_UI)` or a prop that defaults to the old layout, so taking it out is a grep. */
export const NEW_UI = typeof location !== 'undefined' && new URLSearchParams(location.search).get('ui') === 'new';
export type Section = 'world' | 'edit' | 'notes' | 'play';
export type WorldTab = 'generate' | 'sketch' | 'library';
export type EditTab = 'sites' | 'build' | 'town' | 'ground' | 'scatter' | 'design';
export type NotesTab = 'npcs' | 'plots' | 'notes' | 'gazetteer';
export type Tabs = { world: WorldTab; edit: EditTab; notes: NotesTab };
/** How far a phone's sheet is up: its tool strip only, half the screen, or all of it. */
export type Snap = 'peek' | 'half' | 'full';

export const SECTIONS: { key: Section; label: string; icon: 'globe' | 'pencil' | 'book' | 'pawn'; kbd: string }[] = [
  { key: 'world', label: 'World', icon: 'globe', kbd: '1' },
  { key: 'edit', label: 'Edit', icon: 'pencil', kbd: '2' },
  { key: 'notes', label: 'Notes', icon: 'book', kbd: '3' },
  { key: 'play', label: 'Play', icon: 'pawn', kbd: '4' },
];

/** What generating would do to the changes made on the world shown: remake this world round
 * them (`regen`: its sketch redrawn), or make another world and leave them with this one
 * (`newworld`: a new seed or settings). */
export type Remake = 'regen' | 'newworld';

/** A question before work would be dropped or moved: new sketch strokes, a site's unsaved
 * design, a world about to be remade round its changes (or left for another), or one an agent
 * remade through mapd, to load (`remade`). `proceed`
 * carries on once it is answered; `generate`: the sketch can be generated, `remake`: what that
 * would do to the changes made. */
export interface Ask {
  kind: 'sketch' | 'design' | 'remade' | Remake;
  proceed: () => void;
  generate?: boolean;
  remake?: Remake | null;
  /** `remade` (an agent remade the world): what loading it would drop, said in a line. */
  drops?: string;
}

const PHONE ='(max-width: 720px) and (min-height: 501px)';
const SHORT = '(max-height: 500px)';

/** A setting this viewer's browser keeps (null when unset or unreadable). */
export function stored(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

/** Keep a setting in this viewer's browser. */
export function store(key: string, value: string) {
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
  tabs = $state<Tabs>({ world: 'generate', edit: 'sites', notes: 'npcs' });
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
  /** Where the dock begins (px): the phone's sheet's top, and the right dock's left edge (the
   * window's edges while there is none), for what floats over the map beside it. */
  sheetTop = $state(Infinity);
  dockLeft = $state(Infinity);
  /** `?ui=new`: the Edit group whose tools the tool row shows (the open tool's, or the last one picked). */
  group = $state<GroupKey>('settlements');
  /** `?ui=new`: the Edit tool open (App's, worked out from the panels), else null. */
  toolNow: () => ToolKey | null = () => null;
  /** Tools used since their hints were last shown (the rest are read from storage). */
  private used = $state<Partial<Record<ToolKey, boolean>>>({});

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

  /** The tool has made a change: its one-line hint is shown no more (only as its tooltip). */
  isUsed(t: ToolKey): boolean {
    return this.used[t] ?? stored(`ui.used.${t}`) === '1';
  }
  markUsed(t: ToolKey) {
    if (this.isUsed(t)) return;
    store(`ui.used.${t}`, '1');
    this.used = { ...this.used, [t]: true };
  }
  /** Every tool's hint shown again (the ☰ menu). */
  hintsAgain() {
    for (const t of EDIT_TOOLS) {
      try {
        localStorage.removeItem(`ui.used.${t.key}`);
      } catch {
        // (No storage: the hints come back for this visit.)
      }
    }
    this.used = Object.fromEntries(EDIT_TOOLS.map((t) => [t.key, false]));
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

/** `?ui=new`: whether a tool's "More options" fold is open (shut at first), as this viewer left it
 * (the same key `ToolPanel` keeps). */
export function foldOpen(tool: string): boolean {
  return stored(`ws-fold:tool:${tool}`) === 'open';
}

export function setFold(tool: string, open: boolean) {
  store(`ws-fold:tool:${tool}`, open ? 'open' : 'shut');
}
