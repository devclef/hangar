<script lang="ts">
  import {
    api,
    errorMessage,
    type CatalogManufacturer,
    type CatalogModel,
  } from '../lib/api';
  import CategoryBadge from '../components/CategoryBadge.svelte';
  import ErrorBanner from '../components/ErrorBanner.svelte';
  import Spinner from '../components/Spinner.svelte';
  import EmptyState from '../components/EmptyState.svelte';

  let manufacturers = $state<CatalogManufacturer[]>([]);
  let selected = $state<CatalogManufacturer | null>(null);
  let models = $state<CatalogModel[]>([]);
  let loading = $state(true);
  let modelsLoading = $state(false);
  let error = $state<string | null>(null);
  let modelsError = $state<string | null>(null);

  async function load() {
    error = null;
    try {
      manufacturers = await api.listCatalogManufacturers();
    } catch (e) {
      error = errorMessage(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => void load());

  async function open(mfr: CatalogManufacturer) {
    selected = mfr;
    models = [];
    modelsError = null;
    modelsLoading = true;
    try {
      models = await api.listCatalogModels(mfr.id);
    } catch (e) {
      modelsError = errorMessage(e);
    } finally {
      modelsLoading = false;
    }
  }
</script>

<div class="mb-4 flex flex-wrap items-center gap-2">
  {#if selected}
    <button type="button" class="btn-ghost" onclick={() => (selected = null)}>
      ← All manufacturers
    </button>
    <h1 class="text-lg font-semibold text-zinc-900 dark:text-zinc-100">{selected.name}</h1>
  {:else}
    <h1 class="text-lg font-semibold text-zinc-900 dark:text-zinc-100">Parts catalog</h1>
    <span class="text-sm text-stone-500 dark:text-zinc-400">
      Known models and their official parts — pick one to see the diagram.
    </span>
  {/if}
</div>

{#if error}
  <div class="mb-3"><ErrorBanner message={error} onRetry={load} /></div>
{/if}

{#if !selected}
  <div class="card overflow-hidden">
    {#if loading && manufacturers.length === 0}
      <div class="flex items-center justify-center gap-2 py-16 text-sm text-stone-500 dark:text-zinc-400">
        <Spinner /> Loading…
      </div>
    {:else if manufacturers.length === 0 && !error}
      <EmptyState
        title="No catalog manufacturers yet"
        hint="Catalog files live in catalog-data/ in the repo — drop one in and restart, or run: cargo run -- import-catalog catalog-data/<file>.json"
      />
    {:else}
      <div class="overflow-x-auto">
        <table class="w-full min-w-[36rem]">
          <thead class="border-b border-stone-200 dark:border-zinc-800 bg-stone-50 dark:bg-zinc-900/70">
            <tr>
              <th class="th">Manufacturer</th>
              <th class="th text-right">Models</th>
              <th class="th w-24"></th>
            </tr>
          </thead>
          <tbody class="divide-y divide-stone-100 dark:divide-zinc-800">
            {#each manufacturers as m (m.id)}
              <tr class="transition-colors hover:bg-stone-50 dark:bg-zinc-900/70 dark:hover:bg-zinc-800/60">
                <td class="td">
                  <button
                    type="button"
                    class="text-left font-medium text-zinc-900 hover:underline dark:text-zinc-100"
                    onclick={() => open(m)}
                  >{m.name}</button
                  >
                  {#if m.notes}
                    <span class="block text-xs text-stone-500 dark:text-zinc-400">{m.notes}</span>
                  {/if}
                </td>
                <td class="td text-right tabular-nums text-stone-600 dark:text-zinc-400">
                  {m.model_count}
                </td>
                <td class="td text-right">
                  <button type="button" class="btn-ghost" onclick={() => open(m)}>Browse</button>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>
{:else}
  <div class="card overflow-hidden">
    {#if modelsLoading}
      <div class="flex items-center justify-center gap-2 py-16 text-sm text-stone-500 dark:text-zinc-400">
        <Spinner /> Loading…
      </div>
    {:else if modelsError}
      <div class="p-4"><ErrorBanner message={modelsError} onRetry={() => selected && open(selected)} /></div>
    {:else if models.length === 0}
      <EmptyState
        title="No catalog models for {selected.name}"
        hint="Add a file in catalog-data/ for this manufacturer and restart (or run the import-catalog command)."
      />
    {:else}
      <div class="overflow-x-auto">
        <table class="w-full min-w-[40rem]">
          <thead class="border-b border-stone-200 dark:border-zinc-800 bg-stone-50 dark:bg-zinc-900/70">
            <tr>
              <th class="th">Model</th>
              <th class="th">Category</th>
              <th class="th">Source file</th>
              <th class="th w-28"></th>
            </tr>
          </thead>
          <tbody class="divide-y divide-stone-100 dark:divide-zinc-800">
            {#each models as m (m.id)}
              <tr class="transition-colors hover:bg-stone-50 dark:bg-zinc-900/70 dark:hover:bg-zinc-800/60">
                <td class="td">
                  <a
                    class="font-medium text-zinc-900 hover:underline dark:text-zinc-100"
                    href="#/catalog/models/{m.id}"
                  >{m.name}</a
                  >
                </td>
                <td class="td"><CategoryBadge category={m.category} /></td>
                <td class="td">
                  <code class="rounded bg-stone-100 px-1.5 py-0.5 font-mono text-xs text-stone-600 dark:bg-zinc-800 dark:text-zinc-400"
                    >{m.source_file}</code
                  >
                </td>
                <td class="td text-right">
                  <a class="btn-ghost" href="#/catalog/models/{m.id}">View</a>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>
{/if}
