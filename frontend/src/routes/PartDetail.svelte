<script lang="ts">
  import {
    api,
    errorMessage,
    type Model,
    type PartDetail as PartDetailT,
  } from '../lib/api';
  import { isUrl } from '../lib/format';
  import CategoryBadge from '../components/CategoryBadge.svelte';
  import QuantityStepper from '../components/QuantityStepper.svelte';
  import Flash from '../components/Flash.svelte';
  import ErrorBanner from '../components/ErrorBanner.svelte';
  import Spinner from '../components/Spinner.svelte';

  let { id }: { id: number } = $props();

  let detail = $state<PartDetailT | null>(null);
  let allModels = $state<Model[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let flash = $state<string | null>(null);
  let linkSelection = $state<number | ''>('');
  let busy = $state(false);

  async function load() {
    error = null;
    try {
      detail = await api.getPart(id);
      allModels = await api.listModels();
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

  const unlinkedModels = $derived.by(() => {
    const d = detail;
    if (!d) return [];
    return allModels.filter((m) => !d.models.some((x) => x.id === m.id));
  });
</script>

{#if loading && !detail}
  <div class="flex items-center justify-center gap-2 py-16 text-sm text-stone-500">
    <Spinner /> Loading…
  </div>
{:else if error && !detail}
  <ErrorBanner message={error} onRetry={load} />
{:else if detail}
  <div class="mb-4 flex flex-wrap items-center justify-between gap-2">
    <a href="#/parts" class="text-sm text-stone-500 hover:text-stone-800">← All parts</a>
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
      <h1 class="text-2xl font-bold text-zinc-900">{detail.part.name}</h1>
      {#if detail.part.part_type}
        <span class="rounded bg-stone-200 px-2 py-0.5 text-xs font-semibold uppercase tracking-wide text-stone-600">
          {detail.part.part_type}
        </span>
      {/if}
    </div>
    <div class="mt-4 flex flex-wrap items-center gap-x-8 gap-y-3 text-sm">
      <div>
        <span class="label">Quantity on hand</span>
        <QuantityStepper qty={detail.part.quantity} onAdjust={adjustQty} />
      </div>
      <div>
        <span class="label">Compatible with</span>
        <span class="text-stone-700">
          {detail.models.length === 0
            ? 'no models yet'
            : `${detail.models.length} model${detail.models.length === 1 ? '' : 's'}`}
        </span>
      </div>
      {#if detail.part.link}
        <div>
          <span class="label">Link / SKU</span>
          {#if isUrl(detail.part.link)}
            <a class="text-sky-700 hover:underline" href={detail.part.link} target="_blank" rel="noreferrer">
              {detail.part.link}
            </a>
          {:else}
            <span class="font-mono text-xs text-stone-700">{detail.part.link}</span>
          {/if}
        </div>
      {/if}
    </div>
    {#if detail.part.notes}
      <p class="mt-3 whitespace-pre-wrap text-sm text-stone-600">{detail.part.notes}</p>
    {/if}
    {#if detail.part.photo_url}
      <img
        class="mt-4 max-h-72 rounded-md border border-stone-200"
        src={detail.part.photo_url}
        alt={detail.part.name}
        loading="lazy"
      />
    {/if}
  </div>

  <div class="card mt-6">
    <div class="border-b border-stone-200 px-4 py-3">
      <h2 class="text-sm font-semibold uppercase tracking-wide text-stone-600">
        Compatible models <span class="font-normal normal-case text-stone-400">({detail.models.length})</span>
      </h2>
    </div>
    <div class="flex flex-wrap gap-2 border-b border-stone-100 px-4 py-3">
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
      <div class="px-4 py-12 text-center text-sm text-stone-500">
        This part is not linked to any model yet.
      </div>
    {:else}
      <div class="overflow-x-auto">
        <table class="w-full min-w-[36rem]">
          <thead class="border-b border-stone-100 bg-stone-50">
            <tr>
              <th class="th">Model</th>
              <th class="th">Category</th>
              <th class="th">Manufacturer</th>
              <th class="th w-12"></th>
            </tr>
          </thead>
          <tbody class="divide-y divide-stone-100">
            {#each detail.models as m (m.id)}
              <tr class="transition-colors hover:bg-stone-50">
                <td class="td">
                  <a class="font-medium text-zinc-900 hover:underline" href="#/models/{m.id}">{m.name}</a>
                </td>
                <td class="td"><CategoryBadge category={m.category} /></td>
                <td class="td text-stone-600">{m.manufacturer ?? '—'}</td>
                <td class="td text-right">
                  <button
                    type="button"
                    class="text-stone-400 transition-colors hover:text-rose-600 disabled:opacity-40"
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
{/if}
