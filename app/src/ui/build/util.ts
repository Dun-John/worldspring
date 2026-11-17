// What Build's panels share.
import { TOWN_WARDS, type BuildingFuncs } from '../../gen/protocol';

export const label = (s: string) => s.replace(/_/g, ' ').replace(/^./, (c) => c.toUpperCase());
export const WARDS = TOWN_WARDS.filter((w) => w !== 'empty' && w !== 'castle');
/** The kinds of business, by category, as the menus group them. */
export const categoriesOf = (funcs: BuildingFuncs | null) => (funcs ? [...new Set(funcs.businesses.map((b) => b.category))] : []);
