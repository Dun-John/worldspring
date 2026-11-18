// The notes and the notebook's saves: NPCs and plot points kept, a place's label hidden or its
// note set, and the notes on rooms and at points in rooms (inside, the DM's): the card open and
// where it is, a point note put down or moved by a click, the Notebook's Notes tab's rows, a
// note gone to. `ui/notes.ts` keeps the pure helpers; App keeps the markup reading these.
import { untrack } from 'svelte';
import type { Edits, Interior, Note, Npc, Plot } from '../../gen/protocol';
import type { Move } from '../../render/InteriorLayer';
import type { MapView, PointerTool } from '../../render/MapView';
import type { Change } from '../../sync/mapd';
import type { Selection } from '../../ui/gazetteer';
import type { NoteRow } from '../../ui/NoteList.svelte';
import type { Here } from '../../ui/Notebook.svelte';
import { newPointId, splitPoint, splitRoom } from '../../ui/notes';
import { shell, type NotesTab } from '../../ui/shell/layout.svelte';
import { isAside } from '../../world/setaside';

/** What the notes need from the app. */
export interface NotesHost {
  readonly view: MapView;
  readonly edits: Edits;
  readonly notes: Record<string, Note>;
  /** The notes on places as shown (a building drawn by hand's under its created id). */
  readonly placeNotes: Record<string, Note>;
  readonly renames: Record<string, string>;
  /** The place selected, as the Notebook sees it. */
  readonly here: Here | null;
  /** Counts the worlds made (a new world: its notes' places looked up again). */
  readonly worldsMade: number;
  /** App's `resolvePlace` (a new one per world made: the Notes tab's paths worked out again). */
  readonly resolvePlace: unknown;
  readonly notebookFocus: { readonly tab: NotesTab };
  selection: Selection | null;
  moves: { list: Move[]; x: number; y: number } | null;
  /** The room's or point note's card open (`r:`/`m:` id) and where it is. */
  noteSel: string | null;
  notePath: { site: string; level: string; room: string | null } | null;
  /** The map's pointer while a spot is picked (App's `syncTool` puts it first). */
  pickTool: PointerTool | null;
  toast(text: string, undo?: () => void, more?: Pick<import('../../ui/Toasts.svelte').Toast, 'action' | 'sticky' | 'ms'>): number;
  applyEdits(next: Edits, change: Change, author: string, opts?: { record?: boolean; send?: boolean; quiet?: boolean }): void;
  syncTool(): void;
  drawnPair(id: string): { c: string; b: string } | null;
  stretchesOf(id: string): string[];
  settlementName(index: number): string;
  placeInfo(id: string): Promise<{ name: string } | null>;
  nameOf(id: string): string;
  goName(id: string): Promise<unknown>;
  flyToSize(x: number, y: number, size: number): void;
}

export function notesTools(host: NotesHost) {
  function saveNpc(id: string, npc: Npc | null, tool: string) {
    const all = { ...(host.edits.npcs ?? {}) };
    const name = npc?.name ?? all[id]?.name ?? id;
    if (npc) all[id] = npc;
    else delete all[id];
    // A deleted NPC leaves the plots they were in.
    const inPlots = npc ? host.edits.plots : Object.fromEntries(Object.entries(host.edits.plots ?? {}).map(([k, p]) => [k, p.npcs.includes(id) ? { ...p, npcs: p.npcs.filter((x) => x !== id) } : p]));
    host.applyEdits({ ...host.edits, npcs: all, plots: inPlots }, { tool, id, name }, 'user');
  }

  function savePlot(id: string, plot: Plot | null, tool: string) {
    const all = { ...(host.edits.plots ?? {}) };
    const name = plot?.title ?? all[id]?.title ?? id;
    if (plot) all[id] = plot;
    else delete all[id];
    host.applyEdits({ ...host.edits, plots: all }, { tool, id, name }, 'user');
  }

  /** Hide a feature's label from both maps and search, or show it again. */
  function setHidden(id: string, hide: boolean) {
    // (A building drawn by hand: kept under its created id, a mark under its building id moved.)
    const pair = host.drawnPair(id);
    const key = pair?.c ?? id;
    const set = new Set(host.edits.hidden ?? []);
    if (pair) set.delete(pair.b);
    for (const k of pair ? [key] : host.stretchesOf(key)) {
      if (hide) set.add(k);
      else set.delete(k);
    }
    const next = [...set].sort();
    const now = host.edits.hidden ?? [];
    if (next.length === now.length && next.every((h, i) => h === now[i])) return;
    host.applyEdits({ ...host.edits, hidden: next }, { tool: 'hide_feature', id: key, hidden: hide }, 'user');
  }

  /** Set (or with null, clear) a place's note; `name` says which place in the undo line. */
  function setNote(id: string, note: Note | null, name = host.here?.id === id ? host.here.name : undefined) {
    // (A building drawn by hand: kept under its created id, a note under its building id, shown
    // with it, dropped in the same step.)
    const pair = host.drawnPair(id);
    const key = pair?.c ?? id;
    const all = { ...host.notes };
    if (pair) delete all[pair.b];
    if (note) all[key] = note;
    else delete all[key];
    host.applyEdits({ ...host.edits, notes: all }, { tool: 'annotate_feature', id: key, ...(name ? { name } : {}) }, 'user');
  }

  // Notes on rooms and at points in rooms (inside, the DM's): the card open (`r:`/`m:` id) and
  // where it is.
  type NotePlace = { site: string; siteName: string; level: number; levelName: string; room: number | null; roomName: string | null };
  /** Interiors looked up for notes' places (by site id; cleared when the world changes). */
  let noteSites = new Map<string, Promise<Interior | null>>();
  $effect(() => {
    // (Or a site, building or town laid out anew: its rooms may have moved.)
    void host.worldsMade;
    void host.edits?.designs;
    void host.edits?.buildings;
    void host.edits?.towns;
    noteSites = new Map();
  });
  $effect(() => {
    // (Another place selected: the note card goes.)
    if (host.selection) untrack(() => host.noteSel && closeNote());
  });

  /** Where a room's or point note's id is: its site, level and room, by name. */
  async function notePlace(id: string): Promise<NotePlace | null> {
    const r = splitRoom(id) ?? splitPoint(id);
    if (!r) return null;
    let it: Interior | null = host.view.interior?.interior.id === r.site ? host.view.interior.interior : null;
    if (!it) {
      if (!noteSites.has(r.site)) noteSites.set(r.site, host.view.gen.interior(r.site).catch(() => null));
      it = await noteSites.get(r.site)!;
    }
    const lv = it?.levels[r.level];
    if (!it || !lv) return null;
    let room: number | null = 'room' in r ? r.room : null;
    const at = host.notes[id]?.at;
    if (at) {
      const [i, j] = [Math.floor(at[0]), Math.floor(at[1])];
      const c = i >= 0 && j >= 0 && i < it.nx && j < it.ny ? lv.cells[j * it.nx + i] : -1;
      room = c >= 0 ? c : null;
    }
    const roomName = room !== null && lv.rooms[room] ? (host.renames[`r:${r.site}:${r.level}:${room}`] ?? lv.rooms[room].kind) : null;
    const sewer = /^w:(\d+):/.exec(r.site);
    const siteName = host.renames[r.site] ?? (sewer ? (host.renames[`w:${sewer[1]}`] ?? `the sewers of ${host.settlementName(Number(sewer[1]))}`) : ((await host.placeInfo(r.site))?.name ?? it.name ?? it.function));
    return { site: r.site, siteName, level: r.level, levelName: host.renames[`l:${r.site}:${r.level}`] ?? lv.name, room, roomName };
  }

  /** Open a room's or point note's card (inside: the room outlined). */
  function openNote(id: string) {
    host.selection = null;
    host.moves = null;
    host.noteSel = id;
    host.notePath = null;
    host.view.markRoom(id.startsWith('r:') ? id : null);
    if (shell.phone) shell.card = true;
    void notePlace(id).then((p) => {
      if (host.noteSel === id) host.notePath = p && { site: p.siteName, level: p.levelName, room: p.roomName };
    });
  }

  function closeNote() {
    if (host.noteSel?.startsWith('r:')) host.view.markRoom(null);
    host.noteSel = null;
    host.notePath = null;
  }
  // (Names and the spot change under an open card: its place again.)
  $effect(() => {
    void host.renames;
    void host.notes;
    const id = untrack(() => host.noteSel);
    // (A statement, not `&&`: an assignment to the host's `$state` used as a value warns in dev.)
    if (id)
      void notePlace(id).then((p) => {
        if (untrack(() => host.noteSel === id)) host.notePath = p && { site: p.siteName, level: p.levelName, room: p.roomName };
      });
  });

  /** A room's note or a point's set (null: a room's cleared, a point taken away). */
  function saveNote(id: string, note: Note | null, tool = 'annotate_feature') {
    const all = { ...host.notes };
    if (note) all[id] = note;
    else delete all[id];
    host.applyEdits({ ...host.edits, notes: all }, { tool, id, ...(host.notePath?.room ? { name: host.notePath.room } : {}) }, 'user');
  }

  /** The point notes in a room (by where they are now). */
  const pointsInRoom = $derived.by(() => {
    const r = host.noteSel ? splitRoom(host.noteSel) : null;
    if (!r) return [];
    const it = host.view.interior?.interior.id === r.site ? host.view.interior.interior : null;
    const lv = it?.levels[r.level];
    return Object.entries(host.notes).filter(([k, n]) => {
      const p = splitPoint(k);
      if (!p || p.site !== r.site || p.level !== r.level || !n.at || !it || !lv) return false;
      const [i, j] = [Math.floor(n.at[0]), Math.floor(n.at[1])];
      return i >= 0 && j >= 0 && i < it.nx && j < it.ny && lv.cells[j * it.nx + i] === r.room;
    });
  });

  /** Put a note down at a point inside (`move`: that point note moved there), by a click. */
  function pickNotePoint(move?: string) {
    const done = () => {
      if (host.pickTool !== tool) return;
      host.pickTool = null;
      host.syncTool();
    };
    const tool: PointerTool = {
      down: () => true,
      move: () => {},
      hover: () => {},
      cancel: done,
      up: (x, y) => {
        done();
        const layer = host.view.interiorAt(x, y);
        if (!layer?.roomAt(x, y)) return host.toast('Pick a spot in a room');
        const g = layer.toGrid(x, y);
        putNote(layer.interior.id, layer.currentLevel, [Math.round(g[0] * 100) / 100, Math.round(g[1] * 100) / 100], move, layer.roomNameAt(x, y));
      },
    };
    host.pickTool = tool;
    if (shell.phone) shell.snap = 'peek';
    host.syncTool();
    host.toast(`${shell.phone ? 'Tap' : 'Click'} where in the room the note goes`);
  }

  /** A point note put down at `at` (grid units) on a site's level, or one moved there (to a new id
   * when it changes site or level). */
  function putNote(site: string, level: number, at: [number, number], move?: string, roomName?: string) {
    const all = { ...host.notes };
    let id: string;
    if (move && all[move]) {
      const p = splitPoint(move);
      id = p && p.site === site && p.level === level ? move : `m:${site}:${level}:${p?.own ?? newPointId(site, level).split(':').pop()}`;
      all[id] = { ...all[move], at };
      if (id !== move) delete all[move];
    } else {
      id = newPointId(site, level);
      all[id] = { text: '', at };
    }
    host.applyEdits({ ...host.edits, notes: all }, { tool: move ? 'move_note' : 'add_note_at', id, ...(roomName ? { name: roomName } : {}) }, 'user');
    openNote(id);
  }

  /** Every note (places, rooms, points) for the Notebook's Notes tab, with where it is. */
  let noteRows = $state<NoteRow[]>([]);
  let noteRowsSeq = 0;
  /** Rows already worked out, by id: a note's path changes with its spot or with names, not with its text. */
  let noteRowCache = new Map<string, { at: string; path: string[] }>();
  let noteRowKey: unknown = null;
  async function listNotes() {
    const seq = ++noteRowsSeq;
    // (Names or the world changed: the paths are worked out again.)
    const key = [host.renames, host.resolvePlace, noteSites];
    if (!noteRowKey || (noteRowKey as unknown[]).some((k, i) => k !== key[i])) noteRowCache = new Map();
    noteRowKey = key;
    const rows = await Promise.all(
      Object.entries(host.placeNotes)
        .filter(([id]) => !id.startsWith('gone:') && !isAside(id))
        .map(async ([id, n]): Promise<NoteRow> => {
          const kind = id.startsWith('m:') ? 'point' : id.startsWith('r:') ? 'room' : 'place';
          if (kind === 'place') return { id, kind, note: n, path: [host.nameOf(id)] };
          const at = String(n.at ?? '');
          const hit = noteRowCache.get(id);
          if (hit && hit.at === at) return { id, kind, note: n, path: hit.path };
          const p = await notePlace(id);
          const path = p ? [p.siteName, p.levelName, ...(p.roomName ? [p.roomName] : [])] : [host.nameOf(id)];
          // (Not found yet, say while the world is made: asked again next time.)
          if (p) noteRowCache.set(id, { at, path });
          return { id, kind, note: n, path };
        }),
    );
    if (seq === noteRowsSeq) noteRows = rows;
  }
  $effect(() => {
    if (shell.section === 'notes' && host.notebookFocus.tab === 'notes') {
      void host.placeNotes;
      void host.renames;
      void noteSites;
      untrack(() => void listNotes());
    }
  });

  /** Open a note from the Notebook: a place's as any place; a room's or point's inside its site,
   * on its level, the room outlined or the point framed, with its card. */
  async function goNote(id: string) {
    const r = splitRoom(id) ?? splitPoint(id);
    if (!r) return void host.goName(id);
    // (A sewer section opens as the city's sewers, on its own section.)
    if (host.view.interior?.interior.id !== r.site && !(await host.view.enterBuilding(r.site, { level: r.level }))) return host.toast('That place cannot be opened now');
    if (host.view.interior?.currentLevel !== r.level) host.view.setInteriorLevel(r.level);
    const p = await host.view.gen.place(id);
    if (p) host.flyToSize(p.x, p.y, 120);
    openNote(id);
  }

  return {
    saveNpc,
    savePlot,
    setHidden,
    setNote,
    openNote,
    closeNote,
    saveNote,
    pickNotePoint,
    putNote,
    goNote,
    /** Every note for the Notebook's Notes tab, with where it is. */
    get noteRows() {
      return noteRows;
    },
    /** The point notes in the room whose card is open. */
    get pointsInRoom() {
      return pointsInRoom;
    },
  };
}

export type NotesTools = ReturnType<typeof notesTools>;
