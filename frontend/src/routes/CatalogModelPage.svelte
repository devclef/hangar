<script lang="ts">
  import {
    api,
    errorMessage,
    type CatalogModelDetail,
    type CatalogPartView,
    type Settings,
  } from '../lib/api';
  import DiagramViewer from '../components/DiagramViewer.svelte';
  import CatalogPartsList from '../components/CatalogPartsList.svelte';
  import CategoryBadge from '../components/CategoryBadge.svelte';
  import ErrorBanner from '../components/ErrorBanner.svelte';
  import Flash from '../components/Flash.svelte';
  import Spinner from '../components/Spinner.svelte';

  let { id }: { id: number } = $props();

  let detail = $state<CatalogModelDetail | null>(null);
  let settings = $state<Settings | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let flash = $state<string | null>(null);
  let busy = $state(false);

  /** Target model for "add to inventory": auto-picked when exactly one user
   *  model is linked; a picker otherwise. */
  let targetModel = $state<number | ''>('');

  const linked = $derived(detail?.linked_models ?? []);

  $effect(() => {
    const valid = linked.some((m) => m.id === targetModel);
    if (!valid && linked.length > 0) targetModel = linked[0].id;
    if (linked.length === 0) targetModel = '';
  });

  async function load() {
    error = null;
    try {
      [detail, settings] = await Promise.all([api.getCatalogModel(id), api.getSettings()]);
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

  async function addToInventory(part: CatalogPartView) {
    const modelId = targetModel === '' ? null : targetModel;
    if (modelId === null) {
      error = 'No linked model to add to — link one of your models to this catalog model first.';
      return;
    }
    busy = true;
    try {
      await api.addToInventory(part.id, modelId);
      detail = await api.getCatalogModel(id);
      flashOk(`${part.name} added to inventory.`);
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  async function deletePart(part: CatalogPartView) {
    if (!confirm(`Delete catalog part "${part.name}"? Inventory parts you already created are kept.`))
      return;
    busy = true;
    try {
      await api.deleteCatalogPart(part.id);
      detail = await api.getCatalogModel(id);
      flashOk(`Catalog part "${part.name}" deleted.`);
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="mb-4 flex flex-wrap items-center gap-3">
  <a href="#/catalog" class="btn-ghost">← Catalog</a>
  {#if detail}
    <h1 class="text-lg font-semibold text-zinc-900 dark:text-zinc-100">
      {detail.model.manufacturer} {detail.model.name}
    </h1>
    <CategoryBadge category={detail.model.category} />
    <code
      class="rounded bg-stone-100 px-1.5 py-0.5 font-mono text-xs text-stone-500 dark:bg-zinc-800 dark:text-zinc-400"
    >{detail.model.source_file}</code
    >
  {/if}
</div>

<Flash message={flash} />
{#if error}
  <div class="mb-3"><ErrorBanner message={error} onRetry={load} /></div>
{/if}

{#if loading && !detail}
  <div class="card flex items-center justify-center gap-2 py-16 text-sm text-stone-500 dark:text-zinc-400">
    <Spinner /> Loading…
  </div>
{:else if detail}
  <div class="grid gap-6 lg:grid-cols-2">
    <div class="card p-4">
      <div class="mb-3 flex items-center justify-between">
        <h2 class="text-sm font-semibold uppercase tracking-wide text-stone-600 dark:text-zinc-400">
          Diagram
        </h2>
      </div>
      <DiagramViewer
        asset={detail.diagram_asset ?? detail.model.diagram_asset}
        category={detail.model.category}
        parts={detail.parts}
        lowStockEnabled={settings?.low_stock_enabled ?? true}
        lowStockThreshold={settings?.low_stock_threshold ?? 2}
        onAdd={addToInventory}
      />
    </div>

    <div class="card overflow-hidden">
      <div class="flex flex-wrap items-center justify-between gap-2 border-b border-stone-200 dark:border-zinc-800 px-4 py-3">
        <h2 class="text-sm font-semibold uppercase tracking-wide text-stone-600 dark:text-zinc-400">
          Parts <span class="font-normal normal-case text-stone-400 dark:text-zinc-500">({detail.parts.length})</span>
        </h2>
        {#if linked.length === 0}
          <span class="text-xs text-stone-500 dark:text-zinc-400">
            No models linked — quantities shown after you
            <a class="underline" href="#/models">link a model</a>.
          </span>
        {:else if linked.length > 1}
          <label class="flex items-center gap-1.5 text-xs text-stone-600 dark:text-zinc-400">
            Add to
            <select class="input !w-auto !py-1 text-xs" bind:value={targetModel} disabled={busy}>
              {#each linked as m (m.id)}
                <option value={m.id}>{m.name}</option>
              {/each}
            </select>
          </label>
        {:else}
          <span class="text-xs text-stone-500 dark:text-zinc-400">
            Adding to <strong>{linked[0].name}</strong>
          </span>
        {/if}
      </div>
      <CatalogPartsList
        parts={detail.parts}
        lowStockEnabled={settings?.low_stock_enabled ?? true}
        lowStockThreshold={settings?.low_stock_threshold ?? 2}
        onAdd={addToInventory}
        onDelete={deletePart}
        busy={busy}
        addLabel={linked.length === 0 ? 'Add…' : 'Add'}
      />
    </div>
  </div>
{/if}
