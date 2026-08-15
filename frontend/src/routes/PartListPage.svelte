<script lang="ts">
  import {
    api,
    errorMessage,
    type Part,
    type PartSortParam,
  } from '../lib/api';
  import { isUrl } from '../lib/format';
  import QuantityStepper from '../components/QuantityStepper.svelte';
  import ErrorBanner from '../components/ErrorBanner.svelte';
  import Spinner from '../components/Spinner.svelte';
  import EmptyState from '../components/EmptyState.svelte';

  const SORTS: Array<{ value: PartSortParam; label: string }> = [
    { value: 'quantity_asc', label: 'Quantity: low → high' },
    { value: 'quantity_desc', label: 'Quantity: high → low' },
    { value: 'name_asc', label: 'Name: A → Z' },
    { value: 'name_desc', label: 'Name: Z → A' },
    { value: 'recent', label: 'Recently added' },
  ];

  let parts = $state<Part[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let q = $state('');
  let partType = $state('');
  let sort = $state<PartSortParam>('quantity_asc');

  async function load() {
    error = null;
    try {
      parts = await api.listParts({
        q: q.trim() || undefined,
        part_type: partType || undefined,
        sort,
      });
    } catch (e) {
      error = errorMessage(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void q;
    void partType;
    void sort;
    const t = setTimeout(() => void load(), 200);
    return () => clearTimeout(t);
  });

  async function adjustQty(part: Part, delta: number) {
    try {
      const updated = await api.adjustQuantity(part.id, delta);
      part.quantity = updated.quantity;
    } catch (e) {
      error = errorMessage(e);
    }
  }

  async function remove(part: Part) {
    const ok = window.confirm(`Delete part "${part.name}"? Linked models will lose this part.`);
    if (!ok) return;
    try {
      await api.deletePart(part.id);
      parts = parts.filter((p) => p.id !== part.id);
    } catch (e) {
      error = errorMessage(e);
    }
  }

  const types = $derived([...new Set(parts.map((p) => p.part_type).filter(Boolean))] as string[]);

  const modelNames = (p: Part): string =>
    p.model_names ? p.model_names.split('|').join(', ') : '';
</script>

<div class="mb-4 flex flex-wrap items-center gap-2">
  <input
    class="input w-56"
    type="search"
    placeholder="Search parts…"
    value={q}
    oninput={(e) => (q = e.currentTarget.value)}
  />
  <select
    class="input w-44"
    value={partType}
    onchange={(e) => (partType = e.currentTarget.value)}
    aria-label="Filter by type"
  >
    <option value="">All types</option>
    {#each types as t (t)}
      <option value={t}>{t}</option>
    {/each}
  </select>
  <select
    class="input w-48"
    value={sort}
    onchange={(e) => (sort = e.currentTarget.value as PartSortParam)}
    aria-label="Sort parts"
  >
    {#each SORTS as s (s.value)}
      <option value={s.value}>{s.label}</option>
    {/each}
  </select>
  <a href="#/parts/new" class="btn-primary ml-auto">+ Add part</a>
</div>

{#if error}
  <div class="mb-3"><ErrorBanner message={error} onRetry={load} /></div>
{/if}

<div class="card overflow-hidden">
  {#if loading && parts.length === 0}
    <div class="flex items-center justify-center gap-2 py-16 text-sm text-stone-500">
      <Spinner /> Loading…
    </div>
  {:else if parts.length === 0 && !error}
    <EmptyState
      title="No parts found"
      hint="Add spares and components, then link them to the models they fit."
      actionHref="#/parts/new"
      actionLabel="Add part"
    />
  {:else}
    <div class="overflow-x-auto">
      <table class="w-full min-w-[56rem]">
        <thead class="border-b border-stone-200 bg-stone-50">
          <tr>
            <th class="th">Part</th>
            <th class="th">Type</th>
            <th class="th">Qty</th>
            <th class="th">Models</th>
            <th class="th">Link / SKU</th>
            <th class="th w-24"></th>
          </tr>
        </thead>
        <tbody class="divide-y divide-stone-100">
          {#each parts as p (p.id)}
            <tr class="transition-colors hover:bg-stone-50">
              <td class="td">
                <a class="font-medium text-zinc-900 hover:underline" href="#/parts/{p.id}">{p.name}</a>
                {#if p.notes}
                  <div class="max-w-56 truncate text-xs text-stone-400" title={p.notes}>{p.notes}</div>
                {/if}
              </td>
              <td class="td text-stone-600">{p.part_type ?? '—'}</td>
              <td class="td">
                <div class="flex items-center gap-2">
                  <QuantityStepper qty={p.quantity} onAdjust={(d) => adjustQty(p, d)} />
                  {#if p.quantity === 0}
                    <span class="rounded bg-rose-100 px-1.5 py-0.5 text-xs font-semibold text-rose-700">out</span>
                  {:else if p.quantity <= 2}
                    <span class="rounded bg-amber-100 px-1.5 py-0.5 text-xs font-semibold text-amber-700">low</span>
                  {/if}
                </div>
              </td>
              <td class="td text-stone-600">
                {#if p.model_count}
                  <span title={modelNames(p)}>{p.model_count} linked{p.model_count === 1 ? ' model' : ''}</span>
                {:else}
                  <span class="text-stone-400">unlinked</span>
                {/if}
              </td>
              <td class="td max-w-48">
                {#if p.link}
                  {#if isUrl(p.link)}
                    <a
                      class="block truncate text-sky-700 hover:underline"
                      href={p.link}
                      target="_blank"
                      rel="noreferrer"
                    >{p.link}</a>
                  {:else}
                    <span class="block truncate font-mono text-xs text-stone-600" title={p.link}>{p.link}</span>
                  {/if}
                {:else}
                  <span class="text-stone-400">—</span>
                {/if}
              </td>
              <td class="td text-right">
                <div class="flex justify-end gap-1">
                  <a class="btn-ghost px-2 py-1 text-xs" href="#/parts/{p.id}/edit">Edit</a>
                  <button
                    type="button"
                    class="btn-ghost px-2 py-1 text-xs text-rose-600 hover:bg-rose-50"
                    onclick={() => remove(p)}
                  >Del</button>
                </div>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>
