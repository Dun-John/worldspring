// Edit › Rooms › Ways across: paired exits (`Edits.links`) put down square by square between
// places below ground (`LinkTool`), changed or taken away in the panel. App keeps `syncTool()` /
// `go()` and calls `armLinks` / `disarmLinks`.
import type { LinkEdit, LinkEnd } from '../../gen/protocol';
import { endSite, LinkTool, newLinkId } from '../links';

/** What the ways across need from the app. */
export interface LinkHost {
  readonly view: import('../../render/MapView').MapView;
  readonly edits: import('../../gen/protocol').Edits;
  readonly renames: Record<string, string>;
  /** The place in view inside, if any. */
  readonly inside: import('../../render/MapView').InteriorState | null;
  readonly playOn: boolean;
  /** The site designer open (its rooms unsaved when `dirty`). */
  readonly designer: { dirty: boolean } | null;
  ask(kind: 'design', proceed: () => void): void;
  closeDesigner(): void;
  go(to: import('../../ui/shell/layout.svelte').Section | null, tab?: string): void;
  syncTool(): void;
  /** The Objects tool's chip over the map. */
  chipAt(x: number, y: number, text: string | null): void;
  placeInfo(id: string): Promise<{ name: string } | null | undefined>;
  settlementName(index: number): string;
  toast(text: string, undo?: () => void, more?: Pick<import('../../ui/Toasts.svelte').Toast, 'action' | 'sticky' | 'ms'>): number;
  applyEdits(next: import('../../gen/protocol').Edits, change: import('../../sync/mapd').Change, author: string, opts?: { record?: boolean; send?: boolean; quiet?: boolean }): void;
}

export function linkTools(host: LinkHost) {
  // Rooms › Ways across (paired exits, `Edits.links`): the first end put down, the way and name for
  // the next, the one picked (to change or take away), and why an end was left off.
  let linkOn = $state(false);
  let linkFirst = $state<LinkEnd | null>(null);
  let linkWay = $state('stairs');
  let linkName = $state('');
  let linkPicked = $state<string | null>(null);
  let linkNote = $state<string | null>(null);
  /** Level ids (`l:`) named for the panel: the site's name and the level's. */
  let linkNames = $state<Record<string, { site: string; level: string }>>({});
  /** An end's name for the panel: its level's, and its site's unless that is the place in host.view. */
  function endName(lv: string): string {
    const n = linkNames[lv];
    if (!n) return '…';
    // (A sewer end always carries the sewers' name: its sections have no names of their own.)
    const site = endSite(lv).site;
    return site === host.inside?.id && !site.startsWith('w:') ? n.level : `${n.site} · ${n.level}`;
  }
  let linkTool: LinkTool | null = null;
  /** In a place a way across can go: below ground, or a building with cellars (not while playing). */
  const canLink = $derived(!!host.inside && !host.playOn && (/^[uwk]:/.test(host.inside.id) || (host.inside.id.startsWith('b:') && host.inside.levels.some((l) => l.z < 0))));

  /** Rooms on its ways across (the host.designer shut first; unsaved rooms asked about). */
  function openLinks() {
    if (host.designer?.dirty) return host.ask('design', openLinks);
    linkOn = true;
    if (host.designer) host.closeDesigner();
    host.go('edit', 'design');
  }

  /** Back from the ways across to designing the rooms. */
  function closeLinks() {
    disarmLinks();
    host.go('edit', 'design');
  }

  function armLinks() {
    linkTool ??= new LinkTool(host.view, {
      edits: () => host.edits,
      first: () => linkFirst,
      picked: () => linkPicked,
      square: linkSquare,
      chip: (x, y, text) => host.chipAt(x, y, text),
    });
    host.syncTool();
  }

  function disarmLinks() {
    if (!linkOn) return;
    linkOn = false;
    [linkFirst, linkPicked, linkNote] = [null, null, null];
    host.syncTool();
  }

  /** A square picked with the ways-across tool: the first end, the second (the link is put down),
   * or an end of one put down earlier (it is picked). */
  function linkSquare(end: LinkEnd, id: string | null) {
    linkNote = null;
    if (!linkFirst) {
      [linkPicked, linkFirst] = id ? [id, null] : [null, end];
      return;
    }
    const a = linkFirst;
    if (id || (end.level === a.level && end.x === a.x && end.y === a.y)) return host.toast('That square has a way across already: pick another for the other end');
    linkFirst = null;
    const jid = newLinkId();
    const link: LinkEdit = { a, b: end, way: linkWay, ...(linkName.trim() ? { name: linkName.trim() } : {}) };
    host.applyEdits({ ...host.edits, links: { ...(host.edits.links ?? {}), [jid]: link } }, { tool: 'create_link', id: jid, way: linkWay }, 'user');
    linkName = '';
    linkPicked = jid;
    void checkLink(jid, link);
  }

  /** Whether both ends of a link just put down took it (an end on stairs, a way, a hazard or a
   * square that is not floor is left off): if not, say so. */
  async function checkLink(id: string, l: LinkEdit) {
    const off: string[] = [];
    for (const e of [l.a, l.b]) {
      const { site, level } = endSite(e.level);
      const it = await host.view.gen.interior(site);
      if (!it?.levels[level]?.links?.some((k) => k.id === id)) off.push(endName(e.level));
    }
    if (host.edits.links?.[id] === l && off.length) linkNote = `No way across shows at ${off.join(' or ')}: that square has stairs, a way on, a hazard or another way across. Take it away and pick another square.`;
  }

  function changeLink(id: string, patch: { way?: string; name?: string }) {
    const l = host.edits.links?.[id];
    if (!l) return;
    const next: LinkEdit = { ...l, ...patch };
    if (!next.name) delete next.name;
    host.applyEdits({ ...host.edits, links: { ...(host.edits.links ?? {}), [id]: next } }, { tool: 'create_link', id, way: next.way, changed: true }, 'user');
  }

  function removeLink(id: string) {
    const { [id]: _, ...rest } = host.edits.links ?? {};
    if (!_) return;
    host.applyEdits({ ...host.edits, links: rest }, { tool: 'delete_link', id }, 'user');
    if (linkPicked === id) linkPicked = null;
    linkNote = null;
  }

  /** The ways across with an end in the place in view (in the sewers, anywhere in the network), and the one picked. */
  const linksHere = $derived.by(() => {
    const id = host.inside?.id ?? '';
    const net = /^w:(\d+):/.exec(id);
    const at = (level: string) => {
      const site = endSite(level).site;
      return site === id || (!!net && site.startsWith(`w:${net[1]}:`));
    };
    return Object.entries(host.edits.links ?? {})
      .filter(([k, l]) => k === linkPicked || at(l.a.level) || at(l.b.level))
      .map(([k, l]) => ({ id: k, link: l, a: endName(l.a.level), b: endName(l.b.level) }));
  });

  // Each level the panel names, asked for once ("Undermountain · Level 2"; renamed: as renamed).
  $effect(() => {
    if (!linkOn) return;
    const want = new Set<string>();
    for (const l of linksHere) [l.link.a.level, l.link.b.level].forEach((v) => want.add(v));
    if (linkFirst) want.add(linkFirst.level);
    for (const lv of want) {
      if (lv in linkNames) continue;
      linkNames[lv] = { site: '…', level: '…' };
      void (async () => {
        const { site, level } = endSite(lv);
        const [p, q] = await Promise.all([host.placeInfo(site), host.placeInfo(lv)]);
        // (As the title host.inside has them: a sewer section by its network, a site by its town.)
        const li = Number(site.split(':')[1]);
        const own = host.renames[site] ?? (site.startsWith('w:') ? (host.renames[`w:${li}`] ?? `Sewers of ${host.settlementName(li)}`) : null);
        const siteName = own ?? (/^[uk]:/.test(site) && p?.name ? `${host.settlementName(li)} · ${p.name}` : (p?.name ?? 'a place below ground'));
        linkNames[lv] = { site: siteName, level: q?.name ?? `level ${level + 1}` };
      })();
    }
  });

  return {
    get linkOn() {
      return linkOn;
    },
    set linkOn(v) {
      linkOn = v;
    },
    get linkFirst() {
      return linkFirst;
    },
    set linkFirst(v) {
      linkFirst = v;
    },
    get linkWay() {
      return linkWay;
    },
    set linkWay(v) {
      linkWay = v;
    },
    get linkName() {
      return linkName;
    },
    set linkName(v) {
      linkName = v;
    },
    get linkPicked() {
      return linkPicked;
    },
    set linkPicked(v) {
      linkPicked = v;
    },
    get linkNote() {
      return linkNote;
    },
    set linkNote(v) {
      linkNote = v;
    },
    get linkTool() {
      return linkTool;
    },
    get canLink() {
      return canLink;
    },
    get linksHere() {
      return linksHere;
    },
    endName,
    openLinks,
    closeLinks,
    armLinks,
    disarmLinks,
    changeLink,
    removeLink,
  };
}

export type LinkTools = ReturnType<typeof linkTools>;
