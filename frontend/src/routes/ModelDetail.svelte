<script lang="ts">
  import {
    api,
    errorMessage,
    type CatalogManufacturer,
    type CatalogModel,
    type CatalogModelDetail,
    type CatalogPartView,
    type ModelDetail as ModelDetailT,
    type Part,
    type Settings,
    type UsageRecord,
  } from '../lib/api';
  import { formatDate } from '../lib/format';
  import CategoryBadge from '../components/CategoryBadge.svelte';
  import StatusBadge from '../components/StatusBadge.svelte';
  import QuantityStepper from '../components/QuantityStepper.svelte';
  import DiagramViewer from '../components/DiagramViewer.svelte';
  import CatalogPartsList from '../components/CatalogPartsList.svelte';
  import LogUsageForm from '../components/LogUsageForm.svelte';
  import UsageLog from '../components/UsageLog.svelte';
  import Flash from '../components/Flash.svelte';
  import ErrorBanner from '../components/ErrorBanner.svelte';
  import Spinner from '../components/Spinner.svelte';

  let { id }: { id: number } = $props();

  let detail = $state<ModelDetailT | null>(null);
  let allParts = $state<Part[]>([]);
  let usage = $state<UsageRecord[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let flash = $state<string | null>(null);
  let linkSelection = $state<number | ''>('');
  let busy = $state(false);

  // Reference catalog: the embedded summary (detail.model.catalog) tells us
  // the model is linked; the full diagram + parts list is fetched from the
  // catalog endpoint, scoped to this model's owned quantities.
  let catalogDetail = $state<CatalogModelDetail | null>(null);
  let catalogLoading = $state(false);
  let catalogError = $state<string | null>(null);
  let settings = $state<Settings | null>(null);
  let catBusy = $state(false);
  // "Link to catalog" picker (only used while not linked)
  let catMfrs = $state<CatalogManufacturer[]>([]);
  let catMfrSel = $state<number | ''>('');
  let catModels = $state<CatalogModel[]>([]);
  let catModelSel = $state<number | ''>('');

  const catalogModelId = $derived(detail?.model.catalog_model_id ?? null);

  async function load() {
    error = null;
    try {
      [detail, allParts, usage, settings] = await Promise.all([
        api.getModel(id),
        api.listParts({ sort: 'name_asc' }),
        api.listUsage({ model_id: id }),
        api.getSettings(),
      ]);
    } catch (e) {
      error = errorMessage(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void id;
    void load();
    if (catMfrs.length === 0) {
      api.listCatalogManufacturers().then((m) => (catMfrs = m)).catch(() => {});
    }
  });

  function flashOk(msg: string) {
    flash = msg;
    setTimeout(() => (flash = null), 2500);
  }

  async function addLink() {
    if (linkSelection === '' || !detail) return;
    const partId = linkSelection;
    busy = true;
    try {
      await api.linkPart(id, partId);
      linkSelection = '';
      detail = await api.getModel(id);
      flashOk('Part linked.');
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  async function removeLink(partId: number) {
    if (!detail) return;
    busy = true;
    try {
      await api.unlinkPart(id, partId);
      detail.parts = detail.parts.filter((p) => p.id !== partId);
      flashOk('Part unlinked.');
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  async function adjustQty(part: Part, delta: number) {
    if (!detail) return;
    try {
      const updated = await api.adjustQuantity(part.id, delta);
      part.quantity = updated.quantity;
    } catch (e) {
      error = errorMessage(e);
    }
  }

  async function afterLogged() {
    try {
      [detail, usage] = await Promise.all([
        api.getModel(id),
        api.listUsage({ model_id: id }),
      ]);
    } catch (e) {
      error = errorMessage(e);
    }
  }

  async function loadCatalog() {
    catalogError = null;
    if (catalogModelId === null) {
      catalogDetail = null;
      return;
    }
    catalogLoading = true;
    try {
      // Scoped to this model: quantities reflect this model's inventory only.
      catalogDetail = await api.getCatalogModel(catalogModelId, id);
    } catch (e) {
      catalogError = errorMessage(e);
    } finally {
      catalogLoading = false;
    }
  }

  $effect(() => {
    void catalogModelId;
    if (detail) void loadCatalog();
  });

  async function onCatalogMfrChange() {
    catModelSel = '';
    catModels = [];
    const cat = detail?.model.category;
    if (catMfrSel === '' || !cat) return;
    try {
      // Only offer catalog models matching this model's category: the
      // server would reject the link otherwise.
      const all = await api.listCatalogModels(catMfrSel);
      catModels = all.filter((m) => m.category === cat);
    } catch (e) {
      catalogError = errorMessage(e);
    }
  }

  async function linkToCatalog() {
    if (catModelSel === '') return;
    catBusy = true;
    try {
      await api.linkCatalog(id, catModelSel);
      catMfrSel = '';
      catModelSel = '';
      detail = await api.getModel(id);
      await loadCatalog();
      flashOk(`Linked to catalog model ${detail.catalog?.catalog_model_name ?? ''}.`);
    } catch (e) {
      catalogError = errorMessage(e);
    } finally {
      catBusy = false;
    }
  }

  async function unlinkFromCatalog() {
    if (catalogModelId === null) return;
    catBusy = true;
    try {
      await api.unlinkCatalog(id);
      detail = await api.getModel(id);
      catalogDetail = null;
      // Offer the picker again now that we're unlinked.
      if (catMfrs.length === 0) catMfrs = await api.listCatalogManufacturers().catch(() => []);
      flashOk('Catalog link removed.');
    } catch (e) {
      catalogError = errorMessage(e);
    } finally {
      catBusy = false;
    }
  }

  async function addCatalogPart(part: CatalogPartView) {
    catBusy = true;
    try {
      await api.addToInventory(part.id, id);
      // Refresh both the catalog section and the linked-parts table (a new
      // part row may have been created).
      [detail, catalogDetail] = await Promise.all([
        api.getModel(id),
        api.getCatalogModel(catalogModelId as number, id),
      ]);
      flashOk(`${part.name} added to inventory.`);
    } catch (e) {
      catalogError = errorMessage(e);
    } finally {
      catBusy = false;
    }
  }

  async function remove() {
    if (!detail) return;
    const ok = window.confirm(
      `Delete "${detail.model.name}"? Linked parts stay in your inventory.`,
    );
    if (!ok) return;
    busy = true;
    try {
      await api.deleteModel(id);
      window.location.hash = '#/models';
    } catch (e) {
      error = errorMessage(e);
      busy = false;
    }
  }

  const unlinkedParts = $derived.by(() => {
    const d = detail;
    if (!d) return [];
    return allParts.filter((p) => !d.parts.some((x) => x.id === p.id));
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
    <a href="#/models" class="text-sm text-stone-500 dark:text-zinc-400 hover:text-stone-800 dark:text-zinc-200 dark:hover:text-zinc-200">← All models</a>
    <div class="flex gap-2">
      <a class="btn-ghost" href="#/models/{id}/edit">Edit</a>
      <button type="button" class="btn-danger" disabled={busy} onclick={remove}>Delete</button>
    </div>
  </div>

  <Flash message={flash} />
  {#if error}
    <div class="mb-3"><ErrorBanner message={error} /></div>
  {/if}

  <div class="card p-5">
    <div class="flex flex-wrap items-center gap-3">
      <h1 class="text-2xl font-bold text-zinc-900 dark:text-zinc-100">{detail.model.name}</h1>
      <CategoryBadge category={detail.model.category} />
      <StatusBadge status={detail.model.status} />
    </div>
    <dl class="mt-4 grid grid-cols-2 gap-x-8 gap-y-3 text-sm sm:grid-cols-3">
      <div>
        <dt class="label">Manufacturer</dt>
        <dd class="text-stone-700 dark:text-zinc-300">{detail.model.manufacturer ?? '—'}</dd>
      </div>
      <div>
        <dt class="label">Acquired</dt>
        <dd class="text-stone-700 dark:text-zinc-300">{formatDate(detail.model.date_acquired)}</dd>
      </div>
      <div>
        <dt class="label">Status</dt>
        <dd><StatusBadge status={detail.model.status} /></dd>
      </div>
    </dl>
    {#if detail.model.notes}
      <p class="mt-3 whitespace-pre-wrap text-sm text-stone-600 dark:text-zinc-400">{detail.model.notes}</p>
    {/if}
    {#if detail.model.photo_url}
      <img
        class="mt-4 max-h-72 rounded-md border border-stone-200 dark:border-zinc-800"
        src={detail.model.photo_url}
        alt={detail.model.name}
        loading="lazy"
      />
    {/if}
  </div>

  <div class="card mt-6">
    <div class="flex items-center justify-between border-b border-stone-200 dark:border-zinc-800 px-4 py-3">
      <h2 class="text-sm font-semibold uppercase tracking-wide text-stone-600 dark:text-zinc-400">
        Parts in stock <span class="font-normal normal-case text-stone-400 dark:text-zinc-500">({detail.parts.length})</span>
      </h2>
    </div>
    <div class="flex flex-wrap gap-2 border-b border-stone-100 dark:border-zinc-800 px-4 py-3">
      <select
        class="input max-w-xs"
        bind:value={linkSelection}
        disabled={busy}
        aria-label="Select a part to link"
      >
        <option value="" disabled>Link a part…</option>
        {#each unlinkedParts as p (p.id)}
          <option value={p.id}>{p.name} — {p.quantity} in stock</option>
        {/each}
      </select>
      <button type="button" class="btn-primary" disabled={busy || linkSelection === ''} onclick={addLink}>
        Link
      </button>
      <a class="btn-ghost" href="#/parts/new">New part</a>
    </div>

    {#if detail.parts.length === 0}
      <div class="px-4 py-12 text-center text-sm text-stone-500 dark:text-zinc-400">
        No parts linked to this model yet.
      </div>
    {:else}
      <div class="overflow-x-auto">
        <table class="w-full min-w-[44rem]">
          <thead class="border-b border-stone-100 dark:border-zinc-800 bg-stone-50 dark:bg-zinc-900/70">
            <tr>
              <th class="th">Part</th>
              <th class="th">Qty</th>
              <th class="th">Notes</th>
              <th class="th w-12"></th>
            </tr>
          </thead>
          <tbody class="divide-y divide-stone-100 dark:divide-zinc-800">
            {#each detail.parts as p (p.id)}
              <tr class="transition-colors hover:bg-stone-50 dark:bg-zinc-900/70 dark:hover:bg-zinc-800/60">
                <td class="td">
                  <a class="font-medium text-zinc-900 dark:text-zinc-100 hover:underline" href="#/parts/{p.id}">{p.name}</a>
                </td>
                <td class="td">
                  <QuantityStepper qty={p.quantity} onAdjust={(d) => adjustQty(p, d)} />
                </td>
                <td class="td max-w-xs truncate text-stone-500 dark:text-zinc-400" title={p.notes ?? ''}>{p.notes ?? '—'}</td>
                <td class="td text-right">
                  <button
                    type="button"
                    class="text-stone-400 dark:text-zinc-500 transition-colors hover:text-rose-600 dark:text-rose-400 dark:hover:text-rose-400 disabled:opacity-40"
                    disabled={busy}
                    title="Unlink from this model"
                    aria-label="Unlink part"
                    onclick={() => removeLink(p.id)}
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
    <div class="flex items-center justify-between border-b border-stone-200 dark:border-zinc-800 px-4 py-3">
      <h2 class="text-sm font-semibold uppercase tracking-wide text-stone-600 dark:text-zinc-400">
        Catalog
      </h2>
      {#if detail.catalog}
        <button type="button" class="btn-ghost !py-1 text-xs" disabled={catBusy} onclick={unlinkFromCatalog}>
          Unlink from catalog
        </button>
      {/if}
    </div>
    {#if catalogError}
      <div class="p-4"><ErrorBanner message={catalogError} /></div>
    {/if}

    {#if detail.catalog}
      {#if catalogDetail}
        <div class="grid gap-4 p-4 lg:grid-cols-2">
          <div>
            <p class="mb-2 text-xs text-stone-500 dark:text-zinc-400">
              Linked to <strong class="text-stone-700 dark:text-zinc-300">{catalogDetail.model.manufacturer} {catalogDetail.model.name}</strong>
              — pins are colored by this model's owned quantities.
            </p>
            <DiagramViewer
              asset={catalogDetail.diagram_asset ?? catalogDetail.model.diagram_asset}
              category={catalogDetail.model.category}
              parts={catalogDetail.parts}
              lowStockEnabled={settings?.low_stock_enabled ?? true}
              lowStockThreshold={settings?.low_stock_threshold ?? 2}
              onAdd={addCatalogPart}
            />
          </div>
          <CatalogPartsList
            parts={catalogDetail.parts}
            lowStockEnabled={settings?.low_stock_enabled ?? true}
            lowStockThreshold={settings?.low_stock_threshold ?? 2}
            onAdd={addCatalogPart}
            busy={catBusy}
            addLabel="Add"
          />
        </div>
      {:else if catalogLoading}
        <div class="flex items-center justify-center gap-2 py-10 text-sm text-stone-500 dark:text-zinc-400">
          <Spinner /> Loading catalog…
        </div>
      {/if}
    {:else}
      <div class="p-4">
        <p class="mb-3 text-sm text-stone-600 dark:text-zinc-400">
          Link this model to a reference catalog model to see its official parts diagram and
          one-click add parts to your inventory.
        </p>
        {#if catMfrs.length === 0}
          <div class="flex items-center gap-2 text-sm text-stone-500 dark:text-zinc-400">
            <Spinner /> Loading catalog manufacturers…
          </div>
        {:else}
          <div class="flex flex-wrap items-end gap-3">
            <label class="flex flex-col gap-1">
              <span class="label !mb-0">Manufacturer</span>
              <select class="input max-w-52" bind:value={catMfrSel} oninput={onCatalogMfrChange} disabled={catBusy}>
                <option value="" disabled>Choose…</option>
                {#each catMfrs as m (m.id)}
                  <option value={m.id}>{m.name} ({m.model_count})</option>
                {/each}
              </select>
            </label>
            <label class="flex flex-col gap-1">
              <span class="label !mb-0">Model ({detail.model.category})</span>
              <select class="input max-w-52" bind:value={catModelSel} disabled={catBusy || catModels.length === 0}>
                <option value="" disabled>{catMfrSel === '' ? 'Pick a manufacturer first' : 'No matching catalog models'}</option>
                {#each catModels as m (m.id)}
                  <option value={m.id}>{m.name}</option>
                {/each}
              </select>
            </label>
            <button type="button" class="btn-primary" disabled={catBusy || catModelSel === ''} onclick={linkToCatalog}>
              Link
            </button>
          </div>
        {/if}
      </div>
    {/if}
  </div>

  <div class="card mt-6">
    <div class="border-b border-stone-200 dark:border-zinc-800 px-4 py-3">
      <h2 class="text-sm font-semibold uppercase tracking-wide text-stone-600 dark:text-zinc-400">
        Recent usage <span class="font-normal normal-case text-stone-400 dark:text-zinc-500">({usage.length})</span>
      </h2>
    </div>
    <div class="border-b border-stone-100 dark:border-zinc-800 px-4 py-3">
      <LogUsageForm model={detail.model} parts={detail.parts} onLogged={afterLogged} />
    </div>
    <UsageLog
      records={usage}
      emptyTitle="No parts have been logged as used on this model yet."
      emptyHint="Log a usage above when you repair or modify the model."
    />
  </div>
{/if}
