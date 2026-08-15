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
  };

  let { id }: { id?: number } = $props();
  const editing = $derived(id !== undefined);

  let name = $state('');
  let partType = $state('');
  let quantity = $state('1');
  let cost = $state('');
  let vendor = $state('');
  let notes = $state('');
  let link = $state('');
  let photoUrl = $state('');
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
        partType = p.part_type ?? '';
        quantity = String(p.quantity);
        cost = p.cost === null ? '' : String(p.cost);
        vendor = p.vendor ?? '';
        notes = p.notes ?? '';
        link = p.link ?? '';
        photoUrl = p.photo_url ?? '';
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
      part_type: partType.trim() || null,
      quantity: qty,
      cost: costValue,
      vendor: vendor.trim() || null,
      notes: notes.trim() || null,
      link: link.trim() || null,
      photo_url: photoUrl.trim() || null,
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
      class="text-sm text-stone-500 hover:text-stone-800"
      href={editing ? `#/parts/${id}` : '#/parts'}
    >← Back</a>
  </div>

  {#if loading || !settingsLoaded}
    <div class="card p-10 text-center text-sm text-stone-500">Loading…</div>
  {:else}
    <h1 class="mb-4 text-xl font-bold text-zinc-900">
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

      {#if enabled.has('part_type') && enabled.has('quantity')}
        <div class="grid grid-cols-2 gap-4">
          <div>
            <label class="label" for="p-type">Type</label>
            <input id="p-type" class="input" bind:value={partType} placeholder="rotor blade, ESC, radio…" />
          </div>
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
        </div>
      {:else}
        {#if enabled.has('part_type')}
          <div>
            <label class="label" for="p-type">Type</label>
            <input id="p-type" class="input" bind:value={partType} placeholder="rotor blade, ESC, radio…" />
          </div>
        {/if}
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
