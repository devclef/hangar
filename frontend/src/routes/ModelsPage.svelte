<script lang="ts">
  import { api, errorMessage, type Category, type Model } from '../lib/api';
  import { formatDate } from '../lib/format';
  import CategoryBadge from '../components/CategoryBadge.svelte';
  import StatusBadge from '../components/StatusBadge.svelte';
  import ErrorBanner from '../components/ErrorBanner.svelte';
  import Spinner from '../components/Spinner.svelte';
  import EmptyState from '../components/EmptyState.svelte';

  const CATEGORIES: Array<'all' | Category> = ['all', 'heli', 'plane', 'car', 'drone', 'boat', 'other'];

  let models = $state<Model[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let q = $state('');
  let category = $state<'all' | Category>('all');

  async function load() {
    error = null;
    try {
      models = await api.listModels({
        q: q.trim() || undefined,
        category: category === 'all' ? undefined : category,
      });
    } catch (e) {
      error = errorMessage(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void q;
    void category;
    const t = setTimeout(() => void load(), 200);
    return () => clearTimeout(t);
  });
</script>

<div class="mb-4 flex flex-wrap items-center gap-2">
  <input
    class="input w-56"
    type="search"
    placeholder="Search models…"
    value={q}
    oninput={(e) => (q = e.currentTarget.value)}
  />
  <div class="flex flex-wrap gap-1">
    {#each CATEGORIES as c}
      <button type="button" class="chip {category === c ? 'chip-active' : ''}" onclick={() => (category = c)}>
        {c}
      </button>
    {/each}
  </div>
  <a href="#/models/new" class="btn-primary ml-auto">+ Add model</a>
</div>

{#if error}
  <div class="mb-3"><ErrorBanner message={error} onRetry={load} /></div>
{/if}

<div class="card overflow-hidden">
  {#if loading && models.length === 0}
    <div class="flex items-center justify-center gap-2 py-16 text-sm text-stone-500">
      <Spinner /> Loading…
    </div>
  {:else if models.length === 0 && !error}
    <EmptyState
      title="No models yet"
      hint="Add your first RC model to start tracking parts for it."
      actionHref="#/models/new"
      actionLabel="Add model"
    />
  {:else}
    <div class="overflow-x-auto">
      <table class="w-full min-w-[48rem]">
        <thead class="border-b border-stone-200 bg-stone-50">
          <tr>
            <th class="th">Model</th>
            <th class="th">Category</th>
            <th class="th">Manufacturer</th>
            <th class="th">Status</th>
            <th class="th">Acquired</th>
            <th class="th text-right">Parts</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-stone-100">
          {#each models as m (m.id)}
            <tr class="transition-colors hover:bg-stone-50">
              <td class="td">
                <a class="font-medium text-zinc-900 hover:underline" href="#/models/{m.id}">{m.name}</a>
              </td>
              <td class="td"><CategoryBadge category={m.category} /></td>
              <td class="td text-stone-600">{m.manufacturer ?? '—'}</td>
              <td class="td"><StatusBadge status={m.status} /></td>
              <td class="td tabular-nums text-stone-600">{formatDate(m.date_acquired)}</td>
              <td class="td text-right">
                <a
                  class="inline-flex min-w-7 items-center justify-center rounded-full bg-stone-100 px-2 py-0.5 text-xs font-semibold tabular-nums text-stone-600 hover:bg-stone-200"
                  href="#/models/{m.id}"
                  title="View linked parts"
                >{m.part_count ?? 0}</a>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>
