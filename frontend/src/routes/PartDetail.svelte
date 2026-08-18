<script lang="ts">
  import {
    api,
    errorMessage,
    type CatalogPartSearchHit,
    type Model,
    type PartDetail as PartDetailT,
    type UsageRecord,
  } from '../lib/api';
  import { formatCurrency, isUrl } from '../lib/format';
  import CategoryBadge from '../components/CategoryBadge.svelte';
  import QuantityStepper from '../components/QuantityStepper.svelte';
  import LogUsageForm from '../components/LogUsageForm.svelte';
  import UsageLog from '../components/UsageLog.svelte';
  import Flash from '../components/Flash.svelte';
  import ErrorBanner from '../components/ErrorBanner.svelte';
  import Spinner from '../components/Spinner.svelte';

  let { id }: { id: number } = $props();

  let detail = $state<PartDetailT | null>(null);
  let allModels = $state<Model[]>([]);
  let usage = $state<UsageRecord[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let flash = $state<string | null>(null);
  let linkSelection = $state<number | ''>('');
  let busy = $state(false);
  let catalogQuery = $state('');
  let catalogHits = $state<CatalogPartSearchHit[]>([]);
  let catalogSearched = $state(false);
  let catalogLinking = $state<number | null>(null);
  let currency = $state('USD');

  async function load() {
    error = null;
    try {
      detail = await api.getPart(id);
      [allModels, usage] = await Promise.all([
        api.listModels(),
        api.listUsage({ part_id: id }),
      ]);
      // Best-effort: settings only affect how the cost is displayed.
      api
        .getSettings()
        .then((s) => (currency = s.currency))
        .catch(() => {});
    } catch (e) {
      error = errorMessage(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void id;
    void load();
  });

  function flashOk(msg: string) {
    flash = msg;
    setTimeout(() => (flash = null), 2500);
  }

  async function addModel() {
    if (linkSelection === '' || !detail) return;
    const modelId = linkSelection;
    busy = true;
    try {
      await api.linkModel(id, modelId);
      linkSelection = '';
      detail = await api.getPart(id);
      flashOk('Model linked.');
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  async function removeModel(modelId: number) {
    if (!detail) return;
    busy = true;
    try {
      await api.unlinkModel(id, modelId);
      detail.models = detail.models.filter((m) => m.id !== modelId);
      flashOk('Model unlinked.');
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  async function adjustQty(delta: number) {
    if (!detail) return;
    try {
      const updated = await api.adjustQuantity(id, delta);
      detail.part.quantity = updated.quantity;
    } catch (e) {
      error = errorMessage(e);
    }
  }

  async function afterLogged() {
    try {
      [detail, usage] = await Promise.all([
        api.getPart(id),
        api.listUsage({ part_id: id }),
      ]);
    } catch (e) {
      error = errorMessage(e);
    }
  }

  async function remove() {
    if (!detail) return;
    const ok = window.confirm(`Delete part "${detail.part.name}"? It will be unlinked from all models.`);
    if (!ok) return;
    busy = true;
    try {
      await api.deletePart(id);
      window.location.hash = '#/parts';
    } catch (e) {
      error = errorMessage(e);
      busy = false;
    }
  }

  // Catalog part search: debounced while the user types; only runs while
  // the part is unlinked and the query is non-empty (empty = browse mode
  // is not auto-loaded on the detail page).
  $effect(() => {
    const q = catalogQuery.trim();
    if (q === '' || detail?.catalog) return;
    const t = setTimeout(async () => {
      try {
        catalogHits = await api.searchCatalogParts(q);
        catalogSearched = true;
      } catch (e) {
        error = errorMessage(e);
      }
    }, 200);
    return () => clearTimeout(t);
  });

  async function linkCatalogPart(hit: CatalogPartSearchHit) {
    if (!detail) return;
    catalogLinking = hit.id;
    try {
      detail = await api.linkPartCatalog(id, hit.id);
      catalogQuery = '';
      catalogHits = [];
      catalogSearched = false;
      flashOk('Linked to catalog part.');
    } catch (e) {
      error = errorMessage(e);
    } finally {
      catalogLinking = null;
    }
  }

  async function unlinkCatalogPart() {
    if (!detail) return;
    busy = true;
    try {
      await api.unlinkPartCatalog(id);
      detail.catalog = null;
      detail.part.catalog_part_id = null;
      flashOk('Unlinked from catalog part.');
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  const unlinkedModels = $derived.by(() => {
    const d = detail;
    if (!d) return [];
    return allModels.filter((m) => !d.models.some((x) => x.id === m.id));
  });
</script>

{#if loading && !detail}
  <div class="flex items-center justify-center gap-2 py-16 text-sm text-stone-500 dark:text-zinc-400">
    <Spinner /> Loading…
  </div>
{:else if error && !detail}
  <ErrorBanner message={error} onRetry={load} />
{:else if detail}
  <div class="mb-4 flex flex-wrap items-center justify-between gap-2">
    <a href="#/parts" class="text-sm text-stone-500 dark:text-zinc-400 hover:text-stone-800 dark:text-zinc-200 dark:hover:text-zinc-200">← All parts</a>
    <div class="flex gap-2">
      <a class="btn-ghost" href="#/parts/{id}/edit">Edit</a>
      <button type="button" class="btn-danger" disabled={busy} onclick={remove}>Delete</button>
    </div>
  </div>

  <Flash message={flash} />
  {#if error}
    <div class="mb-3"><ErrorBanner message={error} /></div>
  {/if}

  <div class="card p-5">
    <div class="flex flex-wrap items-center gap-3">
      <h1 class="text-2xl font-bold text-zinc-900 dark:text-zinc-100">{detail.part.name}</h1>
      {#if detail.part.catalog_part_id}
        <span
          class="rounded bg-indigo-100 dark:bg-indigo-500/15 px-1.5 py-0.5 text-xs font-semibold text-indigo-700 dark:text-indigo-400"
          title="Linked to a reference catalog part"
        >catalog</span>
      {/if}
    </div>
    <div class="mt-4 flex flex-wrap items-center gap-x-8 gap-y-3 text-sm">
      <div>
        <span class="label">Quantity on hand</span>
        <QuantityStepper qty={detail.part.quantity} onAdjust={adjustQty} />
      </div>
      {#if detail.part.cost !== null}
        <div>
          <span class="label">Cost</span>
          <span class="text-stone-700 dark:text-zinc-300">{formatCurrency(detail.part.cost, currency)}</span>
        </div>
      {/if}
      {#if detail.part.vendor}
        <div>
          <span class="label">Vendor</span>
          <span class="text-stone-700 dark:text-zinc-300">{detail.part.vendor}</span>
        </div>
      {/if}
      <div>
        <span class="label">Compatible with</span>
        <span class="text-stone-700 dark:text-zinc-300">
          {detail.models.length === 0
            ? 'no models yet'
            : `${detail.models.length} model${detail.models.length === 1 ? '' : 's'}`}
        </span>
      </div>
      {#if detail.part.link}
        <div>
          <span class="label">Link / SKU</span>
          {#if isUrl(detail.part.link)}
            <a class="text-sky-700 dark:text-sky-400 hover:underline" href={detail.part.link} target="_blank" rel="noreferrer">
              {detail.part.link}
            </a>
          {:else}
            <span class="font-mono text-xs text-stone-700 dark:text-zinc-300">{detail.part.link}</span>
          {/if}
        </div>
      {/if}
    </div>
    {#if detail.part.notes}
      <p class="mt-3 whitespace-pre-wrap text-sm text-stone-600 dark:text-zinc-400">{detail.part.notes}</p>
    {/if}
    {#if detail.part.photo_url}
      <img
        class="mt-4 max-h-72 rounded-md border border-stone-200 dark:border-zinc-800"
        src={detail.part.photo_url}
        alt={detail.part.name}
        loading="lazy"
      />
    {/if}
  </div>

  <div class="card mt-6">
    <div class="border-b border-stone-200 dark:border-zinc-800 px-4 py-3">
      <h2 class="text-sm font-semibold uppercase tracking-wide text-stone-600 dark:text-zinc-400">
        Compatible models <span class="font-normal normal-case text-stone-400 dark:text-zinc-500">({detail.models.length})</span>
      </h2>
    </div>
    <div class="flex flex-wrap gap-2 border-b border-stone-100 dark:border-zinc-800 px-4 py-3">
      <select
        class="input max-w-xs"
        bind:value={linkSelection}
        disabled={busy}
        aria-label="Select a model to link"
      >
        <option value="" disabled>Link a model…</option>
        {#each unlinkedModels as m (m.id)}
          <option value={m.id}>{m.name} ({m.category})</option>
        {/each}
      </select>
      <button type="button" class="btn-primary" disabled={busy || linkSelection === ''} onclick={addModel}>
        Link
      </button>
      <a class="btn-ghost" href="#/models/new">New model</a>
    </div>

    {#if detail.models.length === 0}
      <div class="px-4 py-12 text-center text-sm text-stone-500 dark:text-zinc-400">
        This part is not linked to any model yet.
      </div>
    {:else}
      <div class="overflow-x-auto">
        <table class="w-full min-w-[36rem]">
          <thead class="border-b border-stone-100 dark:border-zinc-800 bg-stone-50 dark:bg-zinc-900/70">
            <tr>
              <th class="th">Model</th>
              <th class="th">Category</th>
              <th class="th">Manufacturer</th>
              <th class="th w-12"></th>
            </tr>
          </thead>
          <tbody class="divide-y divide-stone-100 dark:divide-zinc-800">
            {#each detail.models as m (m.id)}
              <tr class="transition-colors hover:bg-stone-50 dark:bg-zinc-900/70 dark:hover:bg-zinc-800/60">
                <td class="td">
                  <a class="font-medium text-zinc-900 dark:text-zinc-100 hover:underline" href="#/models/{m.id}">{m.name}</a>
                </td>
                <td class="td"><CategoryBadge category={m.category} /></td>
                <td class="td text-stone-600 dark:text-zinc-400">{m.manufacturer ?? '—'}</td>
                <td class="td text-right">
                  <button
                    type="button"
                    class="text-stone-400 dark:text-zinc-500 transition-colors hover:text-rose-600 dark:text-rose-400 dark:hover:text-rose-400 disabled:opacity-40"
                    disabled={busy}
                    title="Unlink this model"
                    aria-label="Unlink model"
                    onclick={() => removeModel(m.id)}
                  >✕</button>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>

  <div class="card mt-6">
    <div class="border-b border-stone-200 dark:border-zinc-800 px-4 py-3">
      <h2 class="text-sm font-semibold uppercase tracking-wide text-stone-600 dark:text-zinc-400">
        Catalog
      </h2>
    </div>
    {#if detail.catalog}
      <div class="flex flex-wrap items-center justify-between gap-3 px-4 py-4">
        <div>
          <div class="flex flex-wrap items-center gap-2 text-sm font-medium text-zinc-900 dark:text-zinc-100">
            {detail.catalog.catalog_part_name}
            {#if detail.catalog.part_number}
              <span class="font-mono text-xs text-stone-500 dark:text-zinc-400">{detail.catalog.part_number}</span>
            {/if}
          </div>
          <div class="mt-1 flex flex-wrap items-center gap-2 text-sm text-stone-500 dark:text-zinc-400">
            <span>{detail.catalog.catalog_model_name} — {detail.catalog.manufacturer}</span>
            <CategoryBadge category={detail.catalog.model_category} />
          </div>
          <p class="mt-2 text-xs text-stone-400 dark:text-zinc-500">
            This part counts toward the owned quantities on the catalog page.
          </p>
        </div>
        <button type="button" class="btn-ghost" disabled={busy} onclick={unlinkCatalogPart}>
          Unlink
        </button>
      </div>
    {:else}
      <div class="border-b border-stone-100 dark:border-zinc-800 px-4 py-3">
        <p class="text-sm text-stone-500 dark:text-zinc-400">
          Link this part to a reference catalog part so it counts toward the catalog's
          owned quantities.
        </p>
        <div class="mt-2 flex max-w-md gap-2">
          <input
            class="input"
            type="search"
            placeholder="Search by part name or part number…"
            bind:value={catalogQuery}
          />
        </div>
      </div>
      {#if catalogSearched}
        {#if catalogHits.length === 0}
          <div class="px-4 py-8 text-center text-sm text-stone-500 dark:text-zinc-400">
            No catalog parts match "{catalogQuery.trim()}".
          </div>
        {:else}
          <div class="overflow-x-auto">
            <table class="w-full min-w-[36rem]">
              <thead class="border-b border-stone-100 dark:border-zinc-800 bg-stone-50 dark:bg-zinc-900/70">
                <tr>
                  <th class="th">Part</th>
                  <th class="th">Part number</th>
                  <th class="th">Model</th>
                  <th class="th">Manufacturer</th>
                  <th class="th w-16"></th>
                </tr>
              </thead>
              <tbody class="divide-y divide-stone-100 dark:divide-zinc-800">
                {#each catalogHits as hit (hit.id)}
                  <tr class="transition-colors hover:bg-stone-50 dark:bg-zinc-900/70 dark:hover:bg-zinc-800/60">
                    <td class="td font-medium text-zinc-900 dark:text-zinc-100">{hit.name}</td>
                    <td class="td font-mono text-xs text-stone-600 dark:text-zinc-400">
                      {hit.part_number ?? '—'}
                    </td>
                    <td class="td text-stone-600 dark:text-zinc-400">{hit.catalog_model_name}</td>
                    <td class="td text-stone-600 dark:text-zinc-400">{hit.manufacturer}</td>
                    <td class="td text-right">
                      <button
                        type="button"
                        class="btn-primary px-2 py-1 text-xs"
                        disabled={catalogLinking !== null}
                        onclick={() => linkCatalogPart(hit)}
                      >Link</button>
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      {/if}
    {/if}
  </div>

  <div class="card mt-6">
    <div class="border-b border-stone-200 dark:border-zinc-800 px-4 py-3">
      <h2 class="text-sm font-semibold uppercase tracking-wide text-stone-600 dark:text-zinc-400">
        Recent usage <span class="font-normal normal-case text-stone-400 dark:text-zinc-500">({usage.length})</span>
      </h2>
    </div>
    <div class="border-b border-stone-100 dark:border-zinc-800 px-4 py-3">
      <LogUsageForm part={detail.part} models={allModels} onLogged={afterLogged} />
    </div>
    <UsageLog
      records={usage}
      emptyTitle="No usage recorded for this part yet."
      emptyHint="Log a usage above when this part goes into a model."
    />
  </div>
{/if}
