<script lang="ts">
  import {
    api,
    errorMessage,
    PART_FORM_FIELDS,
    type PartInput,
    type Settings,
  } from '../lib/api';
  import ErrorBanner from '../components/ErrorBanner.svelte';

  // If the settings request fails we still want the form to work, so fall
  // back to "show everything" rather than hiding fields we can't confirm.
  const DEFAULT_SETTINGS: Settings = {
    part_form_fields: PART_FORM_FIELDS.map((f) => f.key),
    currency: 'USD',
    low_stock_enabled: true,
    low_stock_threshold: 2,
    theme: 'system',
  };

  let { id }: { id?: number } = $props();
  const editing = $derived(id !== undefined);

  let name = $state('');
  let quantity = $state('1');
  let cost = $state('');
  let vendor = $state('');
  let notes = $state('');
  let link = $state('');
  let photoUrl = $state('');
  let lowStockEnabled = $state(true);
  let settings = $state<Settings>(DEFAULT_SETTINGS);
  let settingsLoaded = $state(false);
  let loading = $state(id !== undefined);
  let busy = $state(false);
  let error = $state<string | null>(null);

  const enabled = $derived(new Set(settings.part_form_fields));

  // Load the part's field-visibility preferences (once per mount).
  $effect(() => {
    api
      .getSettings()
      .then((s) => {
        settings = s;
      })
      .catch(() => {
        // keep DEFAULT_SETTINGS (all fields visible)
      })
      .finally(() => {
        settingsLoaded = true;
      });
  });

  $effect(() => {
    if (id === undefined) return;
    loading = true;
    error = null;
    let cancelled = false;
    api
      .getPart(id)
      .then((d) => {
        if (cancelled) return;
        const p = d.part;
        name = p.name;
        quantity = String(p.quantity);
        cost = p.cost === null ? '' : String(p.cost);
        vendor = p.vendor ?? '';
        notes = p.notes ?? '';
        link = p.link ?? '';
        photoUrl = p.photo_url ?? '';
        lowStockEnabled = p.low_stock_enabled;
        loading = false;
      })
      .catch((e) => {
        if (cancelled) return;
        error = errorMessage(e);
        loading = false;
      });
    return () => {
      cancelled = true;
    };
  });

  function cancel() {
    history.back();
  }

  async function submit() {
    error = null;
    if (!name.trim()) {
      error = 'Name is required.';
      return;
    }
    const qty = Number(quantity);
    if (quantity.trim() === '' || !Number.isInteger(qty) || qty < 0) {
      error = 'Quantity must be a whole number, zero or more.';
      return;
    }
    if (qty > 1_000_000) {
      error = 'Quantity looks too large (max 1,000,000).';
      return;
    }
    let costValue: number | null = null;
    if (cost.trim() !== '') {
      costValue = Number(cost);
      if (!Number.isFinite(costValue) || costValue < 0) {
        error = 'Cost must be a number, zero or more.';
        return;
      }
    }
    busy = true;
    const input: PartInput = {
      name: name.trim(),
      quantity: qty,
      cost: costValue,
      vendor: vendor.trim() || null,
      notes: notes.trim() || null,
      link: link.trim() || null,
      photo_url: photoUrl.trim() || null,
      low_stock_enabled: lowStockEnabled,
    };
    try {
      const saved = editing ? await api.updatePart(id!, input) : await api.createPart(input);
      window.location.hash = `#/parts/${saved.id}`;
    } catch (e) {
      error = errorMessage(e);
      busy = false;
    }
  }
</script>

<div class="mx-auto max-w-xl">
  <div class="mb-4 flex items-center justify-between">
    <a
      class="text-sm text-stone-500 dark:text-zinc-400 hover:text-stone-800 dark:text-zinc-200 dark:hover:text-zinc-200"
      href={editing ? `#/parts/${id}` : '#/parts'}
    >← Back</a>
  </div>

  {#if loading || !settingsLoaded}
    <div class="card p-10 text-center text-sm text-stone-500 dark:text-zinc-400">Loading…</div>
  {:else}
    <h1 class="mb-4 text-xl font-bold text-zinc-900 dark:text-zinc-100">
      {editing ? `Edit: ${name || 'part'}` : 'Add part'}
    </h1>

    {#if error}
      <div class="mb-3"><ErrorBanner message={error} /></div>
    {/if}

    <form
      class="card space-y-4 p-5"
      onsubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <div>
        <label class="label" for="p-name">Name *</label>
        <input id="p-name" class="input" bind:value={name} placeholder="Main rotor blade set" />
      </div>

      <label class="flex items-start gap-2">
        <input
          type="checkbox"
          class="mt-0.5 size-4 shrink-0 accent-zinc-900 dark:accent-amber-400"
          bind:checked={lowStockEnabled}
        />
        <span class="text-sm">
          <span class="font-medium text-stone-800 dark:text-zinc-200">Low stock warning</span>
          <span class="block text-xs text-stone-400 dark:text-zinc-500">
            Flag this part as "low" when its quantity drops to the threshold
            (Settings). Turn off for parts you intentionally keep at one spare.
          </span>
        </span>
      </label>

      {#if enabled.has('quantity')}
        <div>
          <label class="label" for="p-qty">Quantity on hand</label>
          <input
            id="p-qty"
            class="input"
            type="number"
            min="0"
            step="1"
            bind:value={quantity}
          />
        </div>
      {/if}

      {#if enabled.has('cost') && enabled.has('vendor')}
        <div class="grid grid-cols-2 gap-4">
          <div>
            <label class="label" for="p-cost">Cost</label>
            <input
              id="p-cost"
              class="input"
              type="number"
              min="0"
              step="0.01"
              bind:value={cost}
              placeholder="0.00"
            />
          </div>
          <div>
            <label class="label" for="p-vendor">Vendor</label>
            <input id="p-vendor" class="input" bind:value={vendor} placeholder="Heli-Flex, eBay…" />
          </div>
        </div>
      {:else}
        {#if enabled.has('cost')}
          <div>
            <label class="label" for="p-cost">Cost</label>
            <input
              id="p-cost"
              class="input"
              type="number"
              min="0"
              step="0.01"
              bind:value={cost}
              placeholder="0.00"
            />
          </div>
        {/if}
        {#if enabled.has('vendor')}
          <div>
            <label class="label" for="p-vendor">Vendor</label>
            <input id="p-vendor" class="input" bind:value={vendor} placeholder="Heli-Flex, eBay…" />
          </div>
        {/if}
      {/if}

      {#if enabled.has('link')}
        <div>
          <label class="label" for="p-link">Link / SKU</label>
          <input id="p-link" class="input" bind:value={link} placeholder="https://shop.example.com/… or SKU-1234" />
        </div>
      {/if}

      {#if enabled.has('photo_url')}
        <div>
          <label class="label" for="p-photo">Photo URL</label>
          <input id="p-photo" class="input" bind:value={photoUrl} placeholder="https://… or /photos/blade.jpg" />
        </div>
      {/if}

      {#if enabled.has('notes')}
        <div>
          <label class="label" for="p-notes">Notes</label>
          <textarea id="p-notes" class="input" rows="3" bind:value={notes} placeholder="Size, pitch, material…"></textarea>
        </div>
      {/if}

      <div class="flex justify-end gap-2 pt-1">
        <button type="button" class="btn-ghost" onclick={cancel}>Cancel</button>
        <button type="submit" class="btn-primary" disabled={busy}>
          {editing ? 'Save changes' : 'Add part'}
        </button>
      </div>
    </form>
  {/if}
</div>
