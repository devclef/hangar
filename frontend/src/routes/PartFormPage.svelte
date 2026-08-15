<script lang="ts">
  import { api, errorMessage, type PartInput } from '../lib/api';
  import ErrorBanner from '../components/ErrorBanner.svelte';

  let { id }: { id?: number } = $props();
  const editing = $derived(id !== undefined);

  let name = $state('');
  let partType = $state('');
  let quantity = $state('1');
  let notes = $state('');
  let link = $state('');
  let photoUrl = $state('');
  let loading = $state(id !== undefined);
  let busy = $state(false);
  let error = $state<string | null>(null);

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
    if (!Number.isInteger(qty) || qty < 0) {
      error = 'Quantity must be a whole number, zero or more.';
      return;
    }
    if (qty > 1_000_000) {
      error = 'Quantity looks too large (max 1,000,000).';
      return;
    }
    busy = true;
    const input: PartInput = {
      name: name.trim(),
      part_type: partType.trim() || null,
      quantity: qty,
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

  {#if loading}
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
      <div>
        <label class="label" for="p-link">Link / SKU</label>
        <input id="p-link" class="input" bind:value={link} placeholder="https://shop.example.com/… or SKU-1234" />
      </div>
      <div>
        <label class="label" for="p-photo">Photo URL</label>
        <input id="p-photo" class="input" bind:value={photoUrl} placeholder="https://… or /photos/blade.jpg" />
      </div>
      <div>
        <label class="label" for="p-notes">Notes</label>
        <textarea id="p-notes" class="input" rows="3" bind:value={notes} placeholder="Size, pitch, material…"></textarea>
      </div>
      <div class="flex justify-end gap-2 pt-1">
        <button type="button" class="btn-ghost" onclick={cancel}>Cancel</button>
        <button type="submit" class="btn-primary" disabled={busy}>
          {editing ? 'Save changes' : 'Add part'}
        </button>
      </div>
    </form>
  {/if}
</div>
