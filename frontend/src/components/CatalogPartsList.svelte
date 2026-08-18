<script lang="ts">
  import { pinNumbers } from '../lib/diagram';
  import type { CatalogPartView } from '../lib/api';

  /**
   * The parts table beside a DiagramViewer: name (with matching pin number),
   * part number, legend group, live owned quantity, and an add-to-inventory
   * action. Rows whose notes field is set show a hint icon.
   */
  let {
    parts,
    lowStockEnabled = true,
    lowStockThreshold = 2,
    onAdd,
    onDelete,
    busy = false,
    addLabel = 'Add to inventory',
  }: {
    parts: CatalogPartView[];
    lowStockEnabled?: boolean;
    lowStockThreshold?: number;
    onAdd?: (part: CatalogPartView) => void;
    onDelete?: (part: CatalogPartView) => void;
    busy?: boolean;
    addLabel?: string;
  } = $props();

  const pins = $derived(pinNumbers(parts));

  function ownedBadge(p: CatalogPartView): { text: string; cls: string } {
    const q = p.owned_quantity;
    if (q === null || q === undefined) {
      return { text: '—', cls: 'bg-stone-100 text-stone-500 dark:bg-zinc-800 dark:text-zinc-400' };
    }
    if (q === 0) {
      return { text: '0 · out', cls: 'bg-rose-100 text-rose-700 dark:bg-rose-500/15 dark:text-rose-400' };
    }
    if (lowStockEnabled && q <= lowStockThreshold) {
      return {
        text: `${q} · low`,
        cls: 'bg-amber-100 text-amber-700 dark:bg-amber-400/15 dark:text-amber-400',
      };
    }
    return { text: `${q}`, cls: 'bg-emerald-100 text-emerald-700 dark:bg-emerald-500/15 dark:text-emerald-400' };
  }
</script>

{#if parts.length === 0}
  <div class="px-4 py-10 text-center text-sm text-stone-500 dark:text-zinc-400">
    No parts in this catalog model yet.
  </div>
{:else}
  <div class="overflow-x-auto">
    <table class="w-full min-w-[42rem]">
      <thead class="border-b border-stone-100 dark:border-zinc-800 bg-stone-50 dark:bg-zinc-900/70">
        <tr>
          <th class="th">Part</th>
          <th class="th">Part #</th>
          <th class="th">Group</th>
          <th class="th">Owned</th>
          <th class="th w-44"></th>
        </tr>
      </thead>
      <tbody class="divide-y divide-stone-100 dark:divide-zinc-800">
        {#each parts as p (p.id)}
          <tr class="transition-colors hover:bg-stone-50 dark:bg-zinc-900/70 dark:hover:bg-zinc-800/60">
            <td class="td">
              <span class="flex items-center gap-2">
                {#if pins.has(p.id)}
                  <span
                    class="inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-stone-200 text-[11px] font-bold text-stone-600 dark:bg-zinc-700 dark:text-zinc-300"
                    title="Pin {pins.get(p.id)} on the diagram"
                  >{pins.get(p.id)}</span
                  >
                {/if}
                <span class="font-medium text-zinc-900 dark:text-zinc-100">{p.name}</span>
                {#if p.notes}
                  <span
                    class="cursor-help text-stone-400 dark:text-zinc-500"
                    title={p.notes}
                    aria-label="Notes: {p.notes}"
                    >ⓘ</span
                  >
                {/if}
              </span>
            </td>
            <td class="td">
              {#if p.part_number}
                <code class="rounded bg-stone-100 px-1.5 py-0.5 font-mono text-xs dark:bg-zinc-800"
                  >{p.part_number}</code
                >
              {:else}
                <span class="text-stone-400 dark:text-zinc-500">—</span>
              {/if}
            </td>
            <td class="td text-stone-600 dark:text-zinc-400">{p.category ?? '—'}</td>
            <td class="td">
              <span
                class="inline-flex items-center rounded-full px-2 py-0.5 text-xs font-semibold tabular-nums {ownedBadge(p).cls}"
              >{ownedBadge(p).text}</span
              >
            </td>
            <td class="td text-right">
              <span class="inline-flex items-center gap-1.5">
                <button
                  type="button"
                  class="btn-ghost !px-2.5 !py-1 text-xs"
                  disabled={busy || onAdd === undefined}
                  onclick={() => onAdd?.(p)}
                >{addLabel}</button
                >
                {#if onDelete}
                  <button
                    type="button"
                    class="text-stone-400 dark:text-zinc-500 transition-colors hover:text-rose-600 dark:hover:text-rose-400 disabled:opacity-40"
                    disabled={busy}
                    title="Delete this catalog part (inventory parts are kept)"
                    aria-label="Delete catalog part {p.name}"
                    onclick={() => onDelete(p)}
                  >✕</button
                >
                {/if}
              </span>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{/if}
