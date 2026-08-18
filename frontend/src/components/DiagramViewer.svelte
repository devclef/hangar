<script lang="ts">
  import { diagramSvg, pinNumbers } from '../lib/diagram';
  import type { CatalogPartView } from '../lib/api';

  /**
   * Renders a diagram SVG (from lib/diagrams/) with numbered hotspot pins at
   * each part's diagram_x/diagram_y percentages. Hovering/focusing a pin shows
   * a tooltip with the part's name, part number, and owned quantity; clicking
   * it invokes `onAdd` (the parent decides what "add" means).
   *
   * Pin colors: gray = not tied to inventory yet, red = out of stock,
   * amber = at/below the low-stock threshold, green = in stock.
   */
  let {
    asset,
    category,
    parts,
    lowStockEnabled = true,
    lowStockThreshold = 2,
    onAdd,
  }: {
    /** Diagram file name (e.g. "heli-generic.svg"); null = category fallback. */
    asset: string | null;
    category: string;
    parts: CatalogPartView[];
    lowStockEnabled?: boolean;
    lowStockThreshold?: number;
    onAdd?: (part: CatalogPartView) => void;
  } = $props();

  const svg = $derived(diagramSvg(asset, category));
  const pins = $derived(pinNumbers(parts));
  const placeable = $derived(parts.filter((p) => p.diagram_x !== null && p.diagram_y !== null));

  let active = $state<CatalogPartView | null>(null);

  function pinClasses(p: CatalogPartView): string {
    const q = p.owned_quantity;
    if (q === null || q === undefined) return 'bg-stone-400 text-white dark:bg-zinc-600';
    if (q === 0) return 'bg-rose-500 text-white';
    if (lowStockEnabled && q <= lowStockThreshold) return 'bg-amber-400 text-zinc-950';
    return 'bg-emerald-500 text-white';
  }

  function ownedText(p: CatalogPartView): string {
    const q = p.owned_quantity;
    if (q === null || q === undefined) return 'not in inventory yet';
    if (q === 0) return 'out of stock';
    if (lowStockEnabled && q <= lowStockThreshold) return `${q} in stock (low)`;
    return `${q} in stock`;
  }

  /** Tooltip placement: below the pin near the top edge, edge-aware left/right. */
  function tipClasses(p: CatalogPartView): string {
    const below = (p.diagram_y ?? 50) < 32;
    const x = p.diagram_x ?? 50;
    const align = x > 68 ? 'right-0' : x < 32 ? 'left-0' : 'left-1/2 -translate-x-1/2';
    return below ? `top-full mt-2 ${align}` : `bottom-full mb-2 ${align}`;
  }
</script>

<div>
  <div
    class="relative w-full text-stone-500 dark:text-zinc-400"
    style="aspect-ratio: 100/60;"
    role="group"
    aria-label="Parts diagram — click a numbered pin to add that part to your inventory"
  >
    {#if svg}
      {@html svg}
    {:else}
      <div class="flex h-full w-full items-center justify-center rounded-md border border-dashed border-stone-300 text-xs text-stone-400 dark:border-zinc-700 dark:text-zinc-500">
        No diagram available for {asset ?? `${category}-generic.svg`}
      </div>
    {/if}

    {#each placeable as p (p.id)}
      <button
        type="button"
        class="absolute z-10 flex h-5 w-5 -translate-x-1/2 -translate-y-1/2 cursor-pointer items-center justify-center rounded-full text-[11px] font-bold shadow ring-2 ring-white transition-transform hover:scale-125 focus:outline-none focus-visible:ring-2 dark:ring-zinc-900 {pinClasses(p)}"
        style="left: {p.diagram_x}%; top: {p.diagram_y}%;"
        title="{p.name}"
        aria-label="Pin {pins.get(p.id)}: {p.name} — click to add to inventory"
        onmouseenter={() => (active = p)}
        onmouseleave={() => (active === p ? (active = null) : undefined)}
        onfocus={() => (active = p)}
        onblur={() => (active === p ? (active = null) : undefined)}
        onclick={() => onAdd?.(p)}
      >
        {pins.get(p.id)}
        {#if active === p}
          <span
            class="pointer-events-none absolute z-20 w-44 rounded-md border border-stone-200 bg-white p-2 text-left text-xs shadow-lg dark:border-zinc-700 dark:bg-zinc-900 {tipClasses(p)}"
          >
            <span class="block font-semibold text-zinc-900 dark:text-zinc-100">{p.name}</span>
            <span class="mt-0.5 block text-stone-500 dark:text-zinc-400">
              {#if p.part_number}
                <code class="rounded bg-stone-100 px-1 font-mono text-[11px] dark:bg-zinc-800"
                  >{p.part_number}</code
                >
              {:else}
                no part number yet
              {/if}
            </span>
            <span class="mt-0.5 block text-stone-500 dark:text-zinc-400">{ownedText(p)}</span>
          </span>
        {/if}
      </button>
    {/each}
  </div>

  <div class="mt-2 flex flex-wrap items-center gap-x-4 gap-y-1 text-[11px] text-stone-500 dark:text-zinc-400">
    <span class="inline-flex items-center gap-1.5">
      <span class="h-2.5 w-2.5 rounded-full bg-stone-400 dark:bg-zinc-600"></span> not in inventory
    </span>
    <span class="inline-flex items-center gap-1.5">
      <span class="h-2.5 w-2.5 rounded-full bg-emerald-500"></span> in stock
    </span>
    <span class="inline-flex items-center gap-1.5">
      <span class="h-2.5 w-2.5 rounded-full bg-amber-400"></span> low
    </span>
    <span class="inline-flex items-center gap-1.5">
      <span class="h-2.5 w-2.5 rounded-full bg-rose-500"></span> out
    </span>
  </div>
</div>
