<script lang="ts">
  // The search box: the notebook's people and plot points, notes written on places, named
  // places on the map, and districts and named businesses (from the settlement layouts) as
  // they arrive, in groups, and underground sites (by name, renamed levels and rooms, kind and
  // theme words, what is attached to them). Chips over the results: "Only in view" or "Near
  // here" keep to the view or round its middle; while underground sites are listed, kind and
  // depth chips narrow them. ↑/↓ move through the results, Enter opens one; Tab reaches the chips.
  import type { Npc, Overlay, PlaceInfo, Plot } from '../gen/protocol';
  import { hitName, kindIcon, kindLabel, listedName, removedKind, searchFeatures, UNDER_DEPTHS, UNDER_KINDS, underIcon, underTarget, unnamed, type BuildingHit, type DistrictHit, type Rect, type Selection, type UnderAttached, type UnderHit, type UnderResult } from './gazetteer';
  import { roleOf, parseLoc, type Token } from '../play/state';
  import Icon from './Icon.svelte';
  import type { IconName } from './icons';

  type Found = BuildingHit | DistrictHit;
  /** A result row; `indent` (with `last`): attached to the site row above it, `wrap`: a sub line
   * on up to two lines (underground sites: what matched, then where). */
  type Row = { key: string; group: string; icon: IconName; name: string; sub: string; pick: () => void; indent?: boolean; last?: boolean; wrap?: boolean };
  type Place = '' | 'view' | 'near';

  /** "Near here": round the view's middle, this far (ft: 5 mi, a morning's walk). */
  const NEAR_FT = 5 * 5280;
  /** Underground sites listed at most (the chips narrow the rest), and asked for. The kind and
   * depth chips filter here, at once, so the chip row can show every kind and depth found (an
   * ask narrowed by them would hide the others); past the ask's cut the count says "+". */
  const UNDER_MAX = 8;
  const UNDER_ASK = 300;
  /** Things attached to a site listed under it at most. */
  const ATTACHED_MAX = 3;

  interface Props {
    overlay: Overlay;
    renames: Record<string, string>;
    searchBuildings: (q: string, rect?: Rect) => Promise<Found[]>;
    viewRect: () => Rect;
    /** Where something in a layout is, by name: its settlement, else its site. */
    whereName: (index: number) => string;
    /** The settlement a site (by id) stands in, by name, if any. */
    inTown: (id: string) => string | null;
    /** A building drawn by hand behind a building id: its site's id (else null). */
    drawnOf: (id: string) => string | null;
    onSelect: (s: Selection) => void;
    npcs: Record<string, Npc>;
    plots: Record<string, Plot>;
    notes: Record<string, { text: string; tags?: string[] }>;
    /** A place's name and position (buildings and the like, for notes on them). */
    resolve: (id: string) => Promise<PlaceInfo | null>;
    onNpc: (id: string) => void;
    onPlot: (id: string) => void;
    /** Show the place a note is on. */
    onNote: (id: string) => void;
    /** The play tokens now (read when a query is typed, not per frame). */
    tokens?: () => Record<string, Token>;
    /** Go to a token: its location and floor, the camera on it, selected. */
    onToken?: (id: string) => void;
    /** The level names of a building or site (the floor picker's), null if unknown. */
    levelNames?: (id: string) => Promise<string[] | null>;
    /** Underground sites matching a search (`GenClient.searchUnder`). */
    searchUnder?: (q: string, filters: object) => Promise<UnderResult>;
    /** Open an underground site at a place of it (`underTarget`: the site, a level or a room). */
    onUnder?: (id: string) => void;
    /** Something is typed in the box (or it is cleared). */
    onType?: () => void;
    /** A settlement's or site's layout index by its feature id (-1 if none). */
    /** A layout's settlement (its own, or the one its site stands in) and site, as `{id, name}`. */
  }
  let { overlay, renames, searchBuildings, viewRect, whereName, inTown, drawnOf, onSelect, npcs, plots, notes, resolve, onNpc, onPlot, onNote, tokens, onToken, levelNames, searchUnder, onUnder, onType }: Props = $props();

  let q = $state('');
  let open = $state(false);
  let active = $state(0);
  let buildings = $state<Found[]>([]);
  let searching = $state(false);
  let under = $state<UnderHit[]>([]);
  /** Underground sites the last answer found in all (`under` holds the first UNDER_ASK). */
  let underFound = $state(0);
  let place = $state<Place>(readPlace());
  let rect = $state<Rect | null>(null);
  /** Near here: the middle of the view (a circle of NEAR_FT, as the underground search takes it; `rect` is its square). */
  let nearMid = $state<[number, number] | null>(null);
  const inNear = (x: number, y: number) => !nearMid || Math.hypot(x - nearMid[0], y - nearMid[1]) <= NEAR_FT;
  /** Kind chips on (the index's kind names) and the depth chip (an `UNDER_DEPTHS` index). */
  let kinds = $state<string[]>([]);
  let depth = $state<number | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let token = 0;
  let box: HTMLDivElement | undefined = $state();
  let input_: HTMLInputElement | undefined = $state();

  function readPlace(): Place {
    try {
      const p = localStorage.getItem('search.place');
      if (p === 'view' || p === 'near') return p;
      return localStorage.getItem('search.inView') === '1' ? 'view' : '';
    } catch {
      return '';
    }
  }

  // Names of places notes are on (map features at once, the rest looked up once). Called while
  // the rows are derived, so a lookup only starts there: the name is set when it comes back.
  // (Again for another world or this one remade: a new `resolve`.)
  let names = $state<Record<string, string>>({});
  let asked = new Set<string>();
  let namesOf: unknown = null;
  $effect.pre(() => {
    if (namesOf === resolve) return;
    [namesOf, names, asked] = [resolve, {}, new Set()];
    // (Another world: what was found in the last one can't be picked.)
    [under, underFound, buildings] = [[], 0, []];
  });
  function placeName(id: string): string {
    if (removedKind(id)) return listedName(id, renames, () => unnamed(id));
    if (renames[id]) return renames[id];
    const f = overlay.features.find((g) => g.id === id);
    if (f) return f.name;
    if (!asked.has(id)) {
      asked.add(id);
      const ask = resolve;
      void ask(id).then((p) => {
        if (namesOf === ask) names[id] = p?.generated ?? p?.name ?? unnamed(id);
      });
    }
    return names[id] ?? '…';
  }

  /** A note's title: its place's name; never a raw id (or the "…" while it is looked up): the note's first words. */
  function noteTitle(id: string, text: string): string {
    const nm = placeName(id);
    return nm && nm !== '…' && nm !== id ? nm : clip(text.replace(/\s+/g, ' ').trim(), 40) || 'Note';
  }

  /** Where a token is: the surface, or its building or site and level. */
  function tokenWhere(loc: string): string {
    if (loc === 'surface') return 'On the map';
    if (/^w:\d+$/.test(loc)) return 'Sewers';
    const { interior, level } = parseLoc(loc);
    if (interior) askLevels(interior);
    if (!interior) return 'On the map';
    const lv = levels[interior];
    const name = lv ? lv[level] : level === 0 ? 'ground floor' : undefined;
    return `${placeName(interior)} · ${name ?? (level === 0 ? 'ground floor' : `level ${level + 1}`)}`;
  }

  // Level names per place, looked up once (while the rows are derived, set when they come back).
  let levels = $state<Record<string, string[] | undefined>>({});
  let levelAsked = new Set<string>();
  let levelsOf: unknown = null;
  $effect.pre(() => {
    if (levelsOf === levelNames) return;
    [levelsOf, levels, levelAsked] = [levelNames, {}, new Set()];
  });
  function askLevels(id: string) {
    if (!levelNames || levelAsked.has(id)) return;
    levelAsked.add(id);
    const ask = levelNames;
    void ask(id).then((n) => {
      if (levelsOf === ask && n) levels[id] = n;
    });
  }

  const needle = $derived(q.trim().toLowerCase());
  const has = (s: string | undefined, t: string) => !!s && s.toLowerCase().includes(t);
  const snippet = (text: string, t: string) => {
    const i = text.toLowerCase().indexOf(t);
    const from = Math.max(0, i - 20);
    return `${from ? '…' : ''}${text.slice(from, from + 70).replace(/\s+/g, ' ')}${text.length > from + 70 ? '…' : ''}`;
  };

  const found = $derived.by<Row[]>(() => {
    const t = needle;
    if (!t) return [];
    const out: Row[] = [];
    const people = Object.entries(npcs)
      .filter(([, n]) => has(n.name, t) || n.tags.some((g) => has(g, t)) || has(n.status, t))
      .sort(([, a], [, b]) => Number(!a.name.toLowerCase().startsWith(t)) - Number(!b.name.toLowerCase().startsWith(t)) || a.name.localeCompare(b.name))
      .slice(0, 6);
    for (const [k, n] of people) out.push({ key: k, group: 'People', icon: 'user', name: n.name, sub: ['NPC', n.status, n.location ? placeName(n.location.id) : ''].filter(Boolean).join(' · '), pick: () => onNpc(k) });
    const plotRows = Object.entries(plots)
      .filter(([, p]) => has(p.title, t) || has(p.text, t) || p.tags.some((g) => has(g, t)))
      .slice(0, 5);
    for (const [k, p] of plotRows) out.push({ key: k, group: 'Plots', icon: 'scroll', name: p.title, sub: has(p.title, t) ? p.status : snippet(p.text, t), pick: () => onPlot(k) });
    if (tokens && onToken) {
      const list = Object.values(tokens())
        .filter((k) => has(k.name, t))
        .sort((a, b) => Number(!a.name.toLowerCase().startsWith(t)) - Number(!b.name.toLowerCase().startsWith(t)) || a.name.localeCompare(b.name))
        .slice(0, 6);
      for (const k of list) out.push({ key: `token:${k.id}`, group: 'Tokens', icon: 'token', name: k.name, sub: [k.kind === 'character' ? roleOf(k) : k.kind, tokenWhere(k.loc), k.hidden ? 'hidden' : ''].filter(Boolean).join(' · '), pick: () => onToken(k.id) });
    }
    const noteRows = Object.entries(notes)
      .filter(([id, n]) => has(n.text, t) || (n.tags ?? []).some((g) => has(g, t)) || has(placeName(id), t))
      .slice(0, 5);
    for (const [id, n] of noteRows) out.push({ key: `note:${id}`, group: 'Notes', icon: 'note', name: noteTitle(id, n.text), sub: snippet(n.text, t) || (n.tags ?? []).join(', '), pick: () => onNote(id) });
    const places = new Set<string>();
    for (const f of searchFeatures(overlay, q, nearMid ? 60 : 12, rect ?? undefined).filter((g) => inNear(g.x, g.y)).slice(0, 12)) {
      const town = inTown(f.id);
      places.add(f.id);
      out.push({ key: f.id, group: 'Places', icon: /^(rd|v):/.test(f.id) ? kindIcon(f.kind) : 'pin', name: renames[f.id] ?? f.name, sub: `${kindLabel(f.kind)}${town ? ` · in ${town}` : ''}`, pick: () => onSelect({ kind: 'feature', feature: f }) });
    }
    const dupSub = subDuplicates(underShown.slice(0, UNDER_MAX));
    for (const h of underShown.slice(0, UNDER_MAX)) {
      out.push({ key: `under:${h.id}`, group: 'Underground', icon: underIcon(h.site_kind), name: underTitle(h), sub: underSub(h, dupSub), pick: () => onUnder?.(underTarget(h)), wrap: true });
      // (Once each: an NPC placed on the site and on one of its levels is one NPC.)
      const seen = new Set<string>();
      const list = h.attached.filter((a) => !seen.has(`${a.what}:${a.id}`) && !!seen.add(`${a.what}:${a.id}`) && attachedMatched(h, a)).slice(0, ATTACHED_MAX);
      list.forEach((a, i) => out.push({ ...attachedRow(h, a), last: i === list.length - 1 }));
    }
    for (const b of buildings.filter((g) => inNear(g.x, g.y))) {
      // A building drawn by hand is its site: listed once, under Places, and hidden with it.
      const site = b.kind === 'building' ? drawnOf(b.id) : null;
      if (site && (places.has(site) || !overlay.features.some((f) => f.id === site))) continue;
      out.push({
        key: b.id,
        group: 'Buildings & districts',
        icon: b.kind === 'district' ? 'grid' : 'building',
        name: hitName(b, renames),
        sub: `${b.kind === 'district' ? 'district' : b.function.toLowerCase()} · ${whereName(b.settlement)}`,
        pick: () => onSelect(b.kind === 'district' ? { kind: 'district', hit: b } : { kind: 'building', hit: b }),
      });
    }
    return out;
  });
  /** Where an underground site is: the site it lies under, else its settlement, else the named
   * place nearest. */
  function underPlace(h: UnderHit): string {
    return h.site ? `under ${renames[h.site.id] ?? h.site.name}` : h.settlement ? `in ${renames[h.settlement.id] ?? h.settlement.name}` : nearOf(h.x_ft, h.y_ft);
  }
  /** A site with no name of its own ("Crypt") is titled by its place: "Crypt under The Lost Halls". */
  const ownName = (h: UnderHit) => h.name !== h.generated || h.site_kind === 'sewer';
  const underTitle = (h: UnderHit) => (ownName(h) ? h.name : [h.name, underPlace(h)].filter(Boolean).join(' '));

  /** The parts of an underground site's sub line, what is different first: what matched (a level,
   * room, NPC, plot or note), the kind (not when the query, a chip or the title says it), the
   * theme, the levels, where it is (unless its title says so). */
  function subParts(h: UnderHit): { what: string; kind: string; theme: string; levels: string; where: string } {
    const m = h.match;
    const said = m && m.what !== 'name' && m.what !== 'kind' && m.what !== 'theme' ? m.what : '';
    const what = !m || !said ? '' : said === 'note' ? `note: ${snippet(m.text, needle)}` : `${said === 'npc' ? 'NPC' : said}: ${m.text}`;
    // (The theme alone when it says the kind: "ice cave", not "cave · ice cave".)
    const theme = h.theme_name && h.theme_name !== h.site_kind && h.site_kind !== 'sewer' ? h.theme_name : '';
    const word = h.site_kind;
    const kindSaid = needle.includes(word) || kinds.includes(word) || theme.split(' ').includes(word) || (!ownName(h) && underTitle(h).toLowerCase().includes(word));
    const kind = kindSaid ? '' : h.site_kind === 'sewer' ? 'sewers' : word;
    // (A city's sewers are named after it.)
    const where = !ownName(h) || (h.site_kind === 'sewer' && h.name === h.generated) ? '' : underPlace(h);
    return { what, kind, theme, levels: `${h.levels} level${h.levels === 1 ? '' : 's'}`, where };
  }
  const subKey = (h: UnderHit) => `${underTitle(h)}|${Object.values(subParts(h)).join('|')}`;
  /** Sub lines of the listed sites that would read the same as another's (by `subKey`). */
  const subDuplicates = (list: UnderHit[]) => {
    const n = new Map<string, number>();
    for (const h of list) n.set(subKey(h), (n.get(subKey(h)) ?? 0) + 1);
    return (h: UnderHit) => (n.get(subKey(h)) ?? 0) > 1;
  };

  /** An underground site's sub line (`dup`: it reads the same as a neighbour's, so the settlement
   * or region is added), what is attached to it and was not matched at the end ("1 NPC, 1 note"),
   * how far when near here. */
  function underSub(h: UnderHit, dup: (h: UnderHit) => boolean): string {
    const p = subParts(h);
    const town = h.settlement ? `in ${renames[h.settlement.id] ?? h.settlement.name}` : '';
    const region = dup(h) ? (town && !p.where.includes(town) ? town : p.where ? '' : nearOf(h.x_ft, h.y_ft)) : '';
    const far = h.distance_ft === undefined ? '' : h.distance_ft < 528 ? 'here' : `${(h.distance_ft / 5280).toFixed(1)} mi`;
    const rest = new Set<string>();
    const count = { npc: 0, plot: 0, note: 0 };
    for (const a of h.attached) {
      if (attachedMatched(h, a) || rest.has(`${a.what}:${a.id}`)) continue;
      rest.add(`${a.what}:${a.id}`);
      count[a.what]++;
    }
    const attached = (['npc', 'plot', 'note'] as const)
      .filter((k) => count[k])
      .map((k) => `${count[k]} ${k === 'npc' ? 'NPC' : k}${count[k] === 1 ? '' : 's'}`)
      .join(', ');
    return [p.what, p.kind, p.theme, p.levels, p.where, region, far, h.hidden ? 'hidden' : '', attached].filter(Boolean).join(' · ');
  }

  /** Whether the query matched something attached to a site (or the level or room it is on): only
   * those are listed under the site. */
  function attachedMatched(h: UnderHit, a: UnderAttached): boolean {
    const ms = h.matches?.length ? h.matches : h.match ? [h.match] : [];
    const direct = a.what === 'note' ? has(a.text, needle) : has(a.name, needle);
    return (
      direct ||
      ms.some((m) => {
        if (m.what === 'npc' || m.what === 'plot') return m.what === a.what && m.id === a.id;
        if (m.what === 'note') return a.what === 'note' && m.id === a.at;
        return (m.what === 'level' || m.what === 'room') && m.id === a.at;
      })
    );
  }

  /** The named place nearest a point (within 3 mi; not seas or land masses), as "near …". */
  function nearOf(x: number, y: number): string {
    let best = '';
    let bestD = 3 * 5280;
    for (const g of overlay.features) {
      if (['ocean', 'sea', 'continent', 'island', 'range'].includes(g.kind)) continue;
      const d = Math.hypot(g.x - x, g.y - y);
      if (d < bestD) [best, bestD] = [renames[g.id] ?? g.name, d];
    }
    return best && `near ${best}`;
  }

  /** A note, NPC or plot point on an underground site, listed under it: a note opens its place,
   * an NPC or plot point the notebook. */
  function attachedRow(h: UnderHit, a: UnderAttached): Row {
    // "on Hall of Bones": the level or room it is on, else the site.
    const on = `on ${a.at !== h.id && /^[lr]:/.test(a.at) ? placeName(a.at) : ownName(h) ? h.name : 'the site'}`;
    const base = { key: `under:${h.id}:${a.what}:${a.id}:${a.at}`, group: 'Underground', indent: true };
    if (a.what === 'npc') return { ...base, icon: 'user', name: a.name ?? '', sub: `NPC ${on}`, pick: () => onNpc(a.id) };
    if (a.what === 'plot') return { ...base, icon: 'scroll', name: a.name ?? '', sub: `plot ${on}`, pick: () => onPlot(a.id) };
    return { ...base, icon: 'note', name: clip((a.text ?? '').replace(/\s+/g, ' '), 60), sub: `note ${on}`, pick: () => onUnder?.(underTarget({ ...h, match: { what: 'note', id: a.at, text: '' } })) };
  }

  /** Text cut to about `n` characters, at a word. */
  function clip(text: string, n: number): string {
    if (text.length <= n) return text;
    const cut = text.slice(0, n);
    const sp = cut.lastIndexOf(' ');
    return `${(sp > n / 2 ? cut.slice(0, sp) : cut).replace(/[\s,;:.]+$/, '')}…`;
  }

  const band = (h: UnderHit, i: number) => h.levels >= UNDER_DEPTHS[i][1] && h.levels <= (UNDER_DEPTHS[i][2] ?? Infinity);
  /** The underground hits the kind and depth chips keep. */
  const underShown = $derived(under.filter((h) => (!kinds.length || kinds.includes(h.site_kind)) && (depth === null || band(h, depth))).map((h, i) => [h, i] as const).sort(([a, i], [b, j]) => underRank(a) - underRank(b) || i - j).map(([h]) => h));
  /** Best first: a name hit (a renamed site's), a level, room or what is attached, then a site
   * with a name of its own that only the kind or theme matched, then the rest. */
  function underRank(h: UnderHit): number {
    const w = h.match?.what;
    return w === 'name' ? 0 : w === 'kind' || w === 'theme' || !w ? (ownName(h) ? 2 : 3) : 1;
  }
  /** Kind and depth chips: every kind and depth, in a fixed order, whenever underground sites are
   * found; those with no site among them are dimmed and off, and so is a lone one (it can
   * narrow nothing) unless it is on. */
  const kindHas = $derived(UNDER_KINDS.map(([, k]) => under.some((h) => h.site_kind === k)));
  const depthHas = $derived(UNDER_DEPTHS.map((_, i) => under.some((h) => band(h, i))));
  const kindOk = (i: number) => kinds.includes(UNDER_KINDS[i][1]) || (kindHas[i] && kindHas.filter(Boolean).length > 1);
  const depthOk = (i: number) => depth === i || (depthHas[i] && depthHas.filter(Boolean).length > 1);
  const chipsOn = $derived(kinds.length > 0 || depth !== null);
  /** Underground sites past the ask's cut (how many of them the chips keep is not known). */
  const beyond = $derived(Math.max(0, underFound - under.length));
  /** The more the chips could narrow the sites listed: two kinds or depths among them. */
  const narrowable = $derived(new Set(underShown.map((h) => h.site_kind)).size > 1 || UNDER_DEPTHS.filter((_, i) => underShown.some((h) => band(h, i))).length > 1);

  // The groups (with a kind or depth chip on, Underground first: the chip is about it) and the
  // rows in that order (↑/↓ walk them as shown).
  const groups = $derived.by(() => {
    const list = [...new Set(found.map((r) => r.group))].map((g) => ({ g, rows: found.filter((r) => r.group === g) }));
    return chipsOn ? [...list.filter((x) => x.g === 'Underground'), ...list.filter((x) => x.g !== 'Underground')] : list;
  });
  const rows = $derived(groups.flatMap((x) => x.rows));
  /** A group's count for its header: its rows (not what is attached); all the sites found underground. */
  const count = (g: string, list: Row[]) => (g === 'Underground' ? `${underShown.length + (chipsOn ? 0 : beyond)}${chipsOn && beyond ? '+' : ''}` : `${list.filter((r) => !r.indent).length}`);

  function input() {
    open = true;
    active = 0;
    buildings = [];
    onType?.();
    // (The underground list stays until the new answer lands; the kind and depth chips last
    // while the query is edited, those the new answer has nothing for switch off.)
    if (!q.trim()) [kinds, depth] = [[], null];
    run(300);
  }

  /** Ask for buildings and underground sites (after `delay` ms; a stale answer is dropped). */
  function run(delay: number) {
    const v = viewRect();
    const mid: [number, number] = [(v[0] + v[2]) / 2, (v[1] + v[3]) / 2];
    // A plain copy: reactive state can't be posted to a worker.
    nearMid = place === 'near' ? mid : null;
    rect = place === 'view' ? [v[0], v[1], v[2], v[3]] : place === 'near' ? [mid[0] - NEAR_FT, mid[1] - NEAR_FT, mid[0] + NEAR_FT, mid[1] + NEAR_FT] : null;
    clearTimeout(timer);
    const mine = ++token;
    const query = q.trim();
    if (query.length < 3) {
      [buildings, under, underFound, searching, kinds, depth] = [[], [], 0, false, [], null];
      return;
    }
    const area = rect ? ([...rect] as Rect) : undefined;
    const filters = { ...(place === 'view' ? { rect: area } : place === 'near' ? { near: mid, radius_ft: NEAR_FT } : {}), limit: UNDER_ASK };
    timer = setTimeout(() => {
      searching = true;
      let left = searchUnder ? 2 : 1;
      const done = () => {
        if (mine === token && !--left) searching = false;
      };
      void searchBuildings(query, area).then((res) => {
        if (mine === token) buildings = res;
        done();
      });
      if (searchUnder)
        void searchUnder(query, filters).then((res) => {
          if (mine !== token) return done();
          [under, underFound] = [res.items, res.found];
          kinds = kinds.filter((k) => under.some((h) => h.site_kind === k));
          if (depth !== null && !under.some((h) => band(h, depth!))) depth = null;
          done();
        });
    }, delay);
  }

  function setPlace(p: Place) {
    place = place === p ? '' : p;
    try {
      localStorage.setItem('search.place', place);
    } catch {
      // Private mode: the choice just isn't remembered.
    }
    if (q.trim()) run(0);
  }

  function toggleKind(k: string) {
    kinds = kinds.includes(k) ? kinds.filter((x) => x !== k) : [...kinds, k];
    active = 0;
  }

  function setDepth(i: number) {
    depth = depth === i ? null : i;
    active = 0;
  }

  /** Focus left the search (box, chips and results) altogether: close the list. */
  function focusOut(e: FocusEvent) {
    if (box?.contains(e.relatedTarget as Node | null)) return;
    setTimeout(() => {
      if (!box?.contains(document.activeElement)) open = false;
    }, 150);
  }

  function pick(r: Row) {
    open = false;
    [kinds, depth] = [[], null];
    (document.activeElement as HTMLElement | null)?.blur();
    r.pick();
  }

  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      if (open && q) open = false;
      else (e.target as HTMLInputElement).blur();
      e.stopPropagation();
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      if (!rows.length) return;
      open = true;
      active = (active + (e.key === 'ArrowDown' ? 1 : rows.length - 1)) % rows.length;
      e.preventDefault();
    } else if (e.key === 'Enter') {
      const r = rows[active] ?? rows[0];
      if (r) pick(r);
    }
  }

  const placeholder = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform) ? 'Search places, people, notes… ⌘K' : 'Search places, people, notes…';
</script>

{#snippet chip(label: string, on: boolean, toggle: () => void, title?: string, off = false)}
  <button class="ws-chip" class:on class:dim={off} disabled={off} aria-pressed={on} onclick={toggle} {title}>{#if on}<Icon name="check" size={12} />{/if}{label}</button>
{/snippet}

<div class="search" bind:this={box} onfocusout={focusOut}>
  <input
    bind:this={input_}
    id="map-search"
    type="search"
    {placeholder}
    bind:value={q}
    oninput={input}
    onfocus={() => (open = true)}
    onkeydown={key}
    aria-label="Search the map, people and notes"
    aria-expanded={open && !!needle}
    aria-controls="search-results"
    role="combobox"
    autocomplete="off"
  />
  {#if open && needle}
    <!-- (Pressing in the list keeps the box focused, so it stays open for the click.) -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      class="results"
      id="search-results"
      role="listbox"
      tabindex="-1"
      onmousedown={(e) => e.preventDefault()}
      onkeydown={(e) => {
        if (e.key === 'Escape') (input_?.focus(), e.stopPropagation());
      }}
    >
      <!-- Chips in groups, a line each (a phone scrolls it sideways): where (always), then every kind and depth while underground sites are found. -->
      <div class="chips" role="group" aria-label="Narrow the results">
        <div class="cg" role="group" aria-label="Where">
          <span class="cl" aria-hidden="true">Where</span>
          {@render chip('Only in view', place === 'view', () => setPlace('view'))}
          {@render chip('Near here', place === 'near', () => setPlace('near'), 'Within 5 miles of the middle of the view')}
        </div>
        {#if under.length}
          <div class="cg" role="group" aria-label="Kind">
            <span class="cl" aria-hidden="true">Kind</span>
            {#each UNDER_KINDS as [label, k], i (k)}{@render chip(label, kinds.includes(k), () => toggleKind(k), undefined, !kindOk(i))}{/each}
          </div>
          <div class="cg" role="group" aria-label="Depth">
            <span class="cl" aria-hidden="true">Depth</span>
            {#each UNDER_DEPTHS as [label], i (label)}{@render chip(label, depth === i, () => setDepth(i), undefined, !depthOk(i))}{/each}
          </div>
        {/if}
      </div>
      {#each groups as { g, rows: list } (g)}
        <div class="group">{g}<span class="count">{count(g, list)}</span></div>
        {#each list as r (r.key)}
          {@const i = rows.indexOf(r)}
          <button class="row" class:indent={r.indent} class:last={r.last} class:wrap={r.wrap} class:active={i === active} role="option" aria-selected={i === active} onclick={() => pick(r)} onmouseenter={() => (active = i)}>
            <Icon name={r.icon} size={16} />
            <span class="text"><span class="name">{r.name}</span><span class="sub">{r.sub}</span></span>
          </button>
        {/each}
        {#if g === 'Underground' && underShown.length + (chipsOn ? 0 : beyond) > UNDER_MAX}
          <div class="note">{underShown.length - UNDER_MAX + (chipsOn ? 0 : beyond)}{chipsOn && beyond ? '+' : ''} more underground{narrowable ? ': narrow them with the chips' : ': type more to narrow them'}</div>
        {/if}
      {/each}
      {#if under.length && !underShown.length}<div class="note">No underground sites with these chips</div>{/if}
      {#if searching}<div class="note">Searching…</div>{/if}
      {#if !searching && !rows.length && needle.length >= 3}<div class="note">Nothing found{place === 'view' ? ' in view' : place === 'near' ? ' near here' : ''}</div>{/if}
      {#if !rows.length && needle.length < 3}<div class="note">Keep typing…</div>{/if}
    </div>
  {/if}
</div>

<style>
  .search {
    position: relative;
    flex: 1;
    min-width: 0;
    font: 13px/1.4 var(--font);
    color: var(--ink);
  }
  input[type='search'] {
    width: 100%;
    box-sizing: border-box;
    min-height: var(--tap);
    font: inherit;
    font-size: 14px;
    padding: 4px 6px;
    background: transparent;
    border: none;
    outline: none;
    color: var(--ink);
  }
  input[type='search']::placeholder {
    color: var(--ink-3);
  }
  .results {
    position: absolute;
    z-index: var(--z-search);
    left: -42px;
    right: -30px;
    top: calc(100% + 8px);
    padding: 4px 0;
    max-height: min(65vh, 560px);
    overflow-y: auto;
    background: var(--paper-solid);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    box-shadow: var(--shadow-lg);
  }
  .chips {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 2px 10px 6px;
    border-bottom: 1px solid var(--line-faint);
  }
  .cg {
    display: flex;
    flex-wrap: nowrap;
    align-items: center;
    gap: 4px;
    overflow-x: auto;
    scrollbar-width: none;
  }
  .cg :global(.ws-chip) {
    flex: none;
  }
  .cg :global(.ws-chip.dim) {
    opacity: 0.4;
  }
  .cl {
    flex: none;
    min-width: 3.4em;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--ink-2);
  }
  /* Off chips stand out from the panel, on ones are filled (and ticked). */
  .chips :global(.ws-chip:not(.on)) {
    background: var(--paper-solid);
    border-color: var(--line-soft);
  }
  .row.indent {
    position: relative;
    padding-left: 36px;
    min-height: 32px;
  }
  /* Attached rows hang off their site row's icon. */
  .row.indent::before {
    content: '';
    position: absolute;
    left: 20px;
    top: 0;
    bottom: 0;
    width: 10px;
    border-left: 1px solid var(--line-soft);
  }
  .row.indent.last::before {
    bottom: 50%;
    border-bottom: 1px solid var(--line-soft);
    border-bottom-left-radius: 4px;
  }
  .row.indent:not(.last)::after {
    content: '';
    position: absolute;
    left: 20px;
    top: 50%;
    width: 10px;
    border-top: 1px solid var(--line-soft);
  }
  .group {
    display: flex;
    justify-content: space-between;
    padding: 6px 12px 2px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--ink-3);
  }
  .row {
    all: unset;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: var(--tap);
    padding: 3px 12px;
    cursor: pointer;
    color: var(--ink-2);
  }
  .row.active,
  .row:focus-visible {
    background: var(--btn-hover);
  }
  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    line-height: 1.25;
  }
  .name,
  .sub {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name {
    font-weight: bold;
    color: var(--ink);
  }
  .sub {
    font-size: 12px;
    font-style: italic;
    color: var(--ink-2);
  }
  .row.wrap .sub {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    white-space: normal;
  }
  .count {
    font-variant-numeric: tabular-nums;
  }
  .note {
    padding: 4px 12px;
    color: var(--ink-3);
    font-style: italic;
  }
</style>
