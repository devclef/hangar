<script lang="ts">
  import type { UsageRecord } from '../lib/api';
  import { formatTimestamp } from '../lib/format';
  import CategoryBadge from './CategoryBadge.svelte';
  import EmptyState from './EmptyState.svelte';

  let {
    records,
    emptyTitle = 'No usage recorded yet.',
    emptyHint,
  }: {
    records: UsageRecord[];
    emptyTitle?: string;
    emptyHint?: string;
  } = $props();
</script>

{#if records.length === 0}
  <div class="px-4 py-10">
    <EmptyState title={emptyTitle} hint={emptyHint} />
  </div>
{:else}
  <div class="overflow-x-auto">
    <table class="w-full min-w-[42rem]">
      <thead class="border-b border-stone-100 dark:border-zinc-800 bg-stone-50 dark:bg-zinc-900/70">
        <tr>
          <th class="th">When</th>
          <th class="th">Part</th>
          <th class="th">Model</th>
          <th class="th">Qty</th>
          <th class="th">Notes</th>
        </tr>
      </thead>
      <tbody class="divide-y divide-stone-100 dark:divide-zinc-800">
        {#each records as r (r.id)}
          <tr class="transition-colors hover:bg-stone-50 dark:bg-zinc-900/70 dark:hover:bg-zinc-800/60">
            <td class="td whitespace-nowrap text-stone-600 dark:text-zinc-400">{formatTimestamp(r.used_at)}</td>
            <td class="td">
              <a
                class="font-medium text-zinc-900 dark:text-zinc-100 hover:underline"
                href="#/parts/{r.part_id}"
              >{r.part_name}</a>
            </td>
            <td class="td">
              <a
                class="font-medium text-zinc-900 dark:text-zinc-100 hover:underline"
                href="#/models/{r.model_id}"
              >{r.model_name}</a>
              <span class="ml-2 inline-block align-middle">
                <CategoryBadge category={r.model_category} />
              </span>
            </td>
            <td class="td tabular-nums text-stone-700 dark:text-zinc-300">{r.quantity}</td>
            <td class="td max-w-xs truncate text-stone-500 dark:text-zinc-400" title={r.notes ?? ''}>
              {r.notes ?? '—'}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{/if}
