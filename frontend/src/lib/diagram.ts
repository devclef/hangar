import type { CatalogPartView } from './api';

/**
 * Eagerly import every SVG in lib/diagrams/ as a raw string so the diagram
 * viewer can look one up by file name — including future per-model overrides
 * named in a catalog file — with no extra network request.
 */
const diagrams = import.meta.glob('./diagrams/*.svg', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>;

/**
 * Resolves the SVG text for a diagram asset. Falls back to the generic
 * per-category SVG when the asset is null/unknown (e.g. a typo in a catalog
 * file must not blank the viewer).
 */
export function diagramSvg(asset: string | null, category: string): string | null {
  const name =
    asset !== null && diagrams[`./diagrams/${asset}`] !== undefined
      ? asset
      : `${category}-generic.svg`;
  return diagrams[`./diagrams/${name}`] ?? null;
}

/**
 * 1-based pin numbers for the parts that are diagram-placeable, in list
 * order. Parts without coordinates get no pin (they stay in the list only).
 */
export function pinNumbers(parts: CatalogPartView[]): Map<number, number> {
  const map = new Map<number, number>();
  let n = 0;
  for (const p of parts) {
    if (p.diagram_x !== null && p.diagram_y !== null) map.set(p.id, ++n);
  }
  return map;
}
