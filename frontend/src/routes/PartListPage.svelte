<script lang="ts">
  import {
    api,
    errorMessage,
    type Model,
    type Part,
    type PartBulkEdit,
    type PartSortParam,
  } from '../lib/api';
  import { formatCurrency, isUrl } from '../lib/format';
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
  let sort = $state<PartSortParam>('quantity_asc');
  let currency = $state('USD');
  /** Low-stock settings; `null` until loaded (falls back to server defaults). */
  let lowStock = $state<{ enabled: boolean; threshold: number } | null>(null);

  // -- selection + bulk edit ------------------------------------------------
  let selected = $state<Set<number>>(new Set());
  let models = $state<Model[]>([]);
  let modelsLoaded = $state(false);
  let bulkSaving = $state(false);

  /** `on` = apply this field; empty `value` with `on` clears the field. */
  interface BulkField {
    on: boolean;
    value: string;
  }
  const off = (): BulkField => ({ on: false, value: '' });
  let fQuantity = $state<BulkField>(off());
  let fCost = $state<BulkField>(off());
  let fVendor = $state<BulkField>(off());
  let fLink = $state<BulkField>(off());
  let fPhoto = $state<BulkField>(off());
  let fNotes = $state<BulkField>(off());
  /** '' = skip, 'on' = enable the low badge, 'off' = disable it. */
  let fLowStock = $state('');
  let bulkLinkModel = $state('');
  let bulkUnlinkModel = $state('');

  async function load() {
    error = null;
    try {
      parts = await api.listParts({
        q: q.trim() || undefined,
        sort,
      });
      // Best-effort: settings affect the cost display and the "low" badge.
      api
        .getSettings()
        .then((s) => {
          currency = s.currency;
          lowStock = { enabled: s.low_stock_enabled, threshold: s.low_stock_threshold };
        })
        .catch(() => {});
    } catch (e) {
      error = errorMessage(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void q;
    void sort;
    const t = setTimeout(() => void load(), 200);
    return () => clearTimeout(t);
  });

  function ensureModels() {
    if (modelsLoaded) return;
    modelsLoaded = true;
    api
      .listModels()
      .then((m) => (models = m))
      .catch((e) => (error = errorMessage(e)));
  }

  function toggleSelect(partId: number) {
    const next = new Set(selected);
    if (next.has(partId)) next.delete(partId);
    else next.add(partId);
    selected = next;
    ensureModels();
  }

  function toggleSelectAll() {
    if (parts.length > 0 && selected.size === parts.length) {
      selected = new Set();
    } else {
      selected = new Set(parts.map((p) => p.id));
      ensureModels();
    }
  }

  function clearSelection() {
    selected = new Set();
  }

  function resetBulkForm() {
    fQuantity = off();
    fCost = off();
    fVendor = off();
    fLink = off();
    fPhoto = off();
    fNotes = off();
    fLowStock = '';
    bulkLinkModel = '';
    bulkUnlinkModel = '';
  }

  async function applyBulkEdit() {
    const ids = [...selected];
    if (ids.length === 0 || bulkSaving) return;
    const edit: PartBulkEdit = { part_ids: ids };
    if (fQuantity.on) {
      const n = Number(fQuantity.value);
      if (fQuantity.value.trim() === '' || !Number.isInteger(n) || n < 0) {
        error = 'Quantity: enter a whole number of 0 or more, or uncheck to skip.';
        return;
      }
      edit.quantity = n;
    }
    if (fCost.on) {
      if (fCost.value.trim() === '') {
        edit.cost = null;
      } else {
        const n = Number(fCost.value);
        if (!Number.isFinite(n) || n < 0) {
          error = 'Cost: enter a number of 0 or more, or uncheck to skip.';
          return;
        }
        edit.cost = n;
      }
    }
    if (fVendor.on) edit.vendor = fVendor.value.trim() === '' ? null : fVendor.value;
    if (fLink.on) edit.link = fLink.value.trim() === '' ? null : fLink.value;
    if (fPhoto.on) edit.photo_url = fPhoto.value.trim() === '' ? null : fPhoto.value;
    if (fNotes.on) edit.notes = fNotes.value.trim() === '' ? null : fNotes.value;
    if (fLowStock !== '') edit.low_stock_enabled = fLowStock === 'on';
    if (bulkLinkModel !== '') edit.model_id = Number(bulkLinkModel);
    if (bulkUnlinkModel !== '') edit.unlink_model_ids = [Number(bulkUnlinkModel)];
    bulkSaving = true;
    error = null;
    try {
      const updated = await api.bulkEditParts(edit);
      const byId = new Map(updated.map((p) => [p.id, p]));
      parts = parts.map((p) => byId.get(p.id) ?? p);
      resetBulkForm();
      selected = new Set();
    } catch (e) {
      error = errorMessage(e);
    } finally {
      bulkSaving = false;
    }
  }

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
      selected.delete(part.id);
    } catch (e) {
      error = errorMessage(e);
    }
  }

  const modelNames = (p: Part): string =>
    p.model_names ? p.model_names.split('|').join(', ') : '';

  /** "Low" badge: globally enabled, enabled on this part, and qty <= threshold. */
  const isLow = (p: Part): boolean =>
    p.quantity > 0 &&
    (lowStock?.enabled ?? true) &&
    p.low_stock_enabled &&
    p.quantity <= (lowStock?.threshold ?? 2);
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

{#if selected.size > 0}
  <div class="card mb-3 p-4">
    <div class="flex flex-wrap items-center gap-2">
      <span class="text-sm font-semibold text-zinc-900 dark:text-zinc-100">
        Bulk edit — {selected.size} part{selected.size === 1 ? '' : 's'}
      </span>
      <button
        type="button"
        class="btn-ghost ml-auto px-2 py-1 text-xs"
        onclick={clearSelection}
      >
        Clear selection
      </button>
    </div>
    <p class="mt-1 text-xs text-stone-500 dark:text-zinc-400">
      Check a field to apply it to every selected part; an empty value clears the field.
    </p>
    <div class="mt-3 grid gap-x-6 gap-y-2 sm:grid-cols-2 lg:grid-cols-3">
      <label class="flex items-center gap-2">
        <input
          type="checkbox"
          class="size-4 shrink-0 accent-zinc-900 dark:accent-amber-400"
          checked={fQuantity.on}
          onchange={(e) => (fQuantity = { ...fQuantity, on: e.currentTarget.checked })}
        />
        <span class="w-16 shrink-0 text-xs font-semibold uppercase tracking-wide text-stone-500 dark:text-zinc-400"
          >Qty</span
        >
        <input
          class="input"
          type="number"
          min="0"
          step="1"
          placeholder={fQuantity.on ? 'set' : 'skip'}
          value={fQuantity.value}
          disabled={!fQuantity.on}
          oninput={(e) => (fQuantity = { ...fQuantity, value: e.currentTarget.value })}
        />
      </label>
      <label class="flex items-center gap-2">
        <input
          type="checkbox"
          class="size-4 shrink-0 accent-zinc-900 dark:accent-amber-400"
          checked={fCost.on}
          onchange={(e) => (fCost = { ...fCost, on: e.currentTarget.checked })}
        />
        <span class="w-16 shrink-0 text-xs font-semibold uppercase tracking-wide text-stone-500 dark:text-zinc-400"
          >Cost</span
        >
        <input
          class="input"
          type="number"
          min="0"
          step="0.01"
          placeholder={fCost.on ? 'empty = clear' : 'skip'}
          value={fCost.value}
          disabled={!fCost.on}
          oninput={(e) => (fCost = { ...fCost, value: e.currentTarget.value })}
        />
      </label>
      <label class="flex items-center gap-2">
        <input
          type="checkbox"
          class="size-4 shrink-0 accent-zinc-900 dark:accent-amber-400"
          checked={fVendor.on}
          onchange={(e) => (fVendor = { ...fVendor, on: e.currentTarget.checked })}
        />
        <span class="w-16 shrink-0 text-xs font-semibold uppercase tracking-wide text-stone-500 dark:text-zinc-400"
          >Vendor</span
        >
        <input
          class="input"
          type="text"
          placeholder={fVendor.on ? 'empty = clear' : 'skip'}
          value={fVendor.value}
          disabled={!fVendor.on}
          oninput={(e) => (fVendor = { ...fVendor, value: e.currentTarget.value })}
        />
      </label>
      <label class="flex items-center gap-2">
        <input
          type="checkbox"
          class="size-4 shrink-0 accent-zinc-900 dark:accent-amber-400"
          checked={fLink.on}
          onchange={(e) => (fLink = { ...fLink, on: e.currentTarget.checked })}
        />
        <span class="w-16 shrink-0 text-xs font-semibold uppercase tracking-wide text-stone-500 dark:text-zinc-400"
          >Link</span
        >
        <input
          class="input"
          type="text"
          placeholder={fLink.on ? 'URL or SKU, empty = clear' : 'skip'}
          value={fLink.value}
          disabled={!fLink.on}
          oninput={(e) => (fLink = { ...fLink, value: e.currentTarget.value })}
        />
      </label>
      <label class="flex items-center gap-2">
        <input
          type="checkbox"
          class="size-4 shrink-0 accent-zinc-900 dark:accent-amber-400"
          checked={fPhoto.on}
          onchange={(e) => (fPhoto = { ...fPhoto, on: e.currentTarget.checked })}
        />
        <span class="w-16 shrink-0 text-xs font-semibold uppercase tracking-wide text-stone-500 dark:text-zinc-400"
          >Photo</span
        >
        <input
          class="input"
          type="text"
          placeholder={fPhoto.on ? 'URL, empty = clear' : 'skip'}
          value={fPhoto.value}
          disabled={!fPhoto.on}
          oninput={(e) => (fPhoto = { ...fPhoto, value: e.currentTarget.value })}
        />
      </label>
      <label class="flex items-center gap-2">
        <input
          type="checkbox"
          class="size-4 shrink-0 accent-zinc-900 dark:accent-amber-400"
          checked={fNotes.on}
          onchange={(e) => (fNotes = { ...fNotes, on: e.currentTarget.checked })}
        />
        <span class="w-16 shrink-0 text-xs font-semibold uppercase tracking-wide text-stone-500 dark:text-zinc-400"
          >Notes</span
        >
        <input
          class="input"
          type="text"
          placeholder={fNotes.on ? 'empty = clear' : 'skip'}
          value={fNotes.value}
          disabled={!fNotes.on}
          oninput={(e) => (fNotes = { ...fNotes, value: e.currentTarget.value })}
        />
      </label>
      <div class="flex items-center gap-2">
        <span class="w-16 shrink-0 whitespace-nowrap text-xs font-semibold uppercase tracking-wide text-stone-500 dark:text-zinc-400"
          >Low stock</span
        >
        <select
          class="input"
          value={fLowStock}
          onchange={(e) => (fLowStock = e.currentTarget.value)}
          aria-label="Low stock warning for all selected parts"
        >
          <option value="">no change</option>
          <option value="on">warn when low</option>
          <option value="off">no warning</option>
        </select>
      </div>
      <div class="flex items-center gap-2">
        <span class="w-16 shrink-0 text-xs font-semibold uppercase tracking-wide text-stone-500 dark:text-zinc-400"
          >Link to</span
        >
        <select
          class="input"
          value={bulkLinkModel}
          onchange={(e) => (bulkLinkModel = e.currentTarget.value)}
          aria-label="Model to link to all selected parts"
        >
          <option value="">no model change</option>
          {#each models as m (m.id)}
            <option value={m.id}>{m.name}</option>
          {/each}
        </select>
      </div>
      <div class="flex items-center gap-2">
        <span class="w-16 shrink-0 text-xs font-semibold uppercase tracking-wide text-stone-500 dark:text-zinc-400"
          >Unlink</span
        >
        <select
          class="input"
          value={bulkUnlinkModel}
          onchange={(e) => (bulkUnlinkModel = e.currentTarget.value)}
          aria-label="Model to unlink from all selected parts"
        >
          <option value="">no model change</option>
          {#each models as m (m.id)}
            <option value={m.id}>{m.name}</option>
          {/each}
        </select>
      </div>
    </div>
    <div class="mt-4">
      <button
        type="button"
        class="btn-primary"
        disabled={bulkSaving}
        onclick={() => applyBulkEdit()}
      >
        {bulkSaving ? 'Applying…' : `Apply to ${selected.size} part${selected.size === 1 ? '' : 's'}`}
      </button>
    </div>
  </div>
{/if}

{#if error}
  <div class="mb-3"><ErrorBanner message={error} onRetry={load} /></div>
{/if}

<div class="card overflow-hidden">
  {#if loading && parts.length === 0}
    <div class="flex items-center justify-center gap-2 py-16 text-sm text-stone-500 dark:text-zinc-400">
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
        <thead class="border-b border-stone-200 dark:border-zinc-800 bg-stone-50 dark:bg-zinc-900/70">
          <tr>
            <th class="th w-10">
              <input
                type="checkbox"
                class="size-4 accent-zinc-900 dark:accent-amber-400"
                aria-label="Select all parts"
                checked={parts.length > 0 && selected.size === parts.length}
                onchange={toggleSelectAll}
              />
            </th>
            <th class="th">Part</th>
            <th class="th">Vendor</th>
            <th class="th">Cost</th>
            <th class="th">Qty</th>
            <th class="th">Models</th>
            <th class="th">Link / SKU</th>
            <th class="th w-24"></th>
          </tr>
        </thead>
        <tbody class="divide-y divide-stone-100 dark:divide-zinc-800">
          {#each parts as p (p.id)}
            <tr
              class="transition-colors hover:bg-stone-50 dark:bg-zinc-900/70 dark:hover:bg-zinc-800/60 {selected.has(p.id) ? 'bg-stone-50 dark:bg-zinc-900/70' : ''}"
            >
              <td class="td w-10">
                <input
                  type="checkbox"
                  class="size-4 accent-zinc-900 dark:accent-amber-400"
                  aria-label="Select {p.name}"
                  checked={selected.has(p.id)}
                  onchange={() => toggleSelect(p.id)}
                />
              </td>
              <td class="td">
                <span class="flex items-center gap-2">
                  <a class="font-medium text-zinc-900 dark:text-zinc-100 hover:underline" href="#/parts/{p.id}">{p.name}</a>
                  {#if p.catalog_part_id}
                    <span
                      class="rounded bg-indigo-100 dark:bg-indigo-500/15 px-1.5 py-0.5 text-xs font-semibold text-indigo-700 dark:text-indigo-400"
                      title="Linked to a reference catalog part"
                    >catalog</span>
                  {/if}
                </span>
                {#if p.notes}
                  <div class="max-w-56 truncate text-xs text-stone-400 dark:text-zinc-500" title={p.notes}>{p.notes}</div>
                {/if}
              </td>
              <td class="td max-w-40 text-stone-600 dark:text-zinc-400">
                {#if p.vendor}
                  <span class="block truncate" title={p.vendor}>{p.vendor}</span>
                {:else}
                  <span class="text-stone-400 dark:text-zinc-500">—</span>
                {/if}
              </td>
              <td class="td text-stone-600 dark:text-zinc-400">
                {#if p.cost !== null}
                  {formatCurrency(p.cost, currency)}
                {:else}
                  <span class="text-stone-400 dark:text-zinc-500">—</span>
                {/if}
              </td>
              <td class="td">
                <div class="flex items-center gap-2">
                  <QuantityStepper qty={p.quantity} onAdjust={(d) => adjustQty(p, d)} />
                  {#if p.quantity === 0}
                    <span class="rounded bg-rose-100 dark:bg-rose-500/15 px-1.5 py-0.5 text-xs font-semibold text-rose-700 dark:text-rose-400">out</span>
                  {:else if isLow(p)}
                    <span class="rounded bg-amber-100 dark:bg-amber-500/15 px-1.5 py-0.5 text-xs font-semibold text-amber-700 dark:text-amber-400">low</span>
                  {/if}
                </div>
              </td>
              <td class="td text-stone-600 dark:text-zinc-400">
                {#if p.model_count}
                  <span title={modelNames(p)}>{p.model_count} linked{p.model_count === 1 ? ' model' : ''}</span>
                {:else}
                  <span class="text-stone-400 dark:text-zinc-500">unlinked</span>
                {/if}
              </td>
              <td class="td max-w-48">
                {#if p.link}
                  {#if isUrl(p.link)}
                    <a
                      class="block truncate text-sky-700 dark:text-sky-400 hover:underline"
                      href={p.link}
                      target="_blank"
                      rel="noreferrer"
                    >{p.link}</a>
                  {:else}
                    <span class="block truncate font-mono text-xs text-stone-600 dark:text-zinc-400" title={p.link}>{p.link}</span>
                  {/if}
                {:else}
                  <span class="text-stone-400 dark:text-zinc-500">—</span>
                {/if}
              </td>
              <td class="td text-right">
                <div class="flex justify-end gap-1">
                  <a class="btn-ghost px-2 py-1 text-xs" href="#/parts/{p.id}/edit">Edit</a>
                  <button
                    type="button"
                    class="btn-ghost px-2 py-1 text-xs text-rose-600 dark:text-rose-400 hover:bg-rose-50 dark:bg-rose-500/10 dark:hover:bg-rose-500/10"
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
