<script lang="ts">
  import { api, errorMessage, type Category, type ModelInput, type ModelStatus } from '../lib/api';
  import ErrorBanner from '../components/ErrorBanner.svelte';

  const CATEGORIES: Category[] = ['heli', 'plane', 'car', 'drone', 'boat', 'other'];
  const STATUSES: ModelStatus[] = ['active', 'retired', 'sold'];

  let { id }: { id?: number } = $props();
  const editing = $derived(id !== undefined);

  let name = $state('');
  let category = $state<Category>('heli');
  let manufacturer = $state('');
  let status = $state<ModelStatus>('active');
  let dateAcquired = $state('');
  let notes = $state('');
  let photoUrl = $state('');
  let loading = $state(editing);
  let busy = $state(false);
  let error = $state<string | null>(null);

  $effect(() => {
    if (id === undefined) return;
    loading = true;
    error = null;
    let cancelled = false;
    api
      .getModel(id)
      .then((d) => {
        if (cancelled) return;
        const m = d.model;
        name = m.name;
        category = m.category;
        manufacturer = m.manufacturer ?? '';
        status = m.status;
        dateAcquired = m.date_acquired ?? '';
        notes = m.notes ?? '';
        photoUrl = m.photo_url ?? '';
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
    busy = true;
    const input: ModelInput = {
      name: name.trim(),
      category,
      manufacturer: manufacturer.trim() || null,
      status,
      date_acquired: dateAcquired || null,
      notes: notes.trim() || null,
      photo_url: photoUrl.trim() || null,
    };
    try {
      const saved = editing ? await api.updateModel(id!, input) : await api.createModel(input);
      window.location.hash = `#/models/${saved.id}`;
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
      href={editing ? `#/models/${id}` : '#/models'}
    >← Back</a>
  </div>

  {#if loading}
    <div class="card p-10 text-center text-sm text-stone-500 dark:text-zinc-400">Loading…</div>
  {:else}
    <h1 class="mb-4 text-xl font-bold text-zinc-900 dark:text-zinc-100">
      {editing ? `Edit: ${name || 'model'}` : 'Add model'}
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
        <label class="label" for="m-name">Name *</label>
        <input id="m-name" class="input" bind:value={name} placeholder="Kraken 580" />
      </div>
      <div class="grid grid-cols-2 gap-4">
        <div>
          <label class="label" for="m-category">Category</label>
          <select id="m-category" class="input" bind:value={category}>
            {#each CATEGORIES as c}
              <option value={c} class="capitalize">{c}</option>
            {/each}
          </select>
        </div>
        <div>
          <label class="label" for="m-status">Status</label>
          <select id="m-status" class="input" bind:value={status}>
            {#each STATUSES as s}
              <option value={s} class="capitalize">{s}</option>
            {/each}
          </select>
        </div>
      </div>
      <div class="grid grid-cols-2 gap-4">
        <div>
          <label class="label" for="m-manufacturer">Manufacturer</label>
          <input id="m-manufacturer" class="input" bind:value={manufacturer} placeholder="Vortex" />
        </div>
        <div>
          <label class="label" for="m-date">Date acquired</label>
          <input id="m-date" class="input" type="date" bind:value={dateAcquired} />
        </div>
      </div>
      <div>
        <label class="label" for="m-photo">Photo URL</label>
        <input id="m-photo" class="input" bind:value={photoUrl} placeholder="https://… or /photos/kraken.jpg" />
      </div>
      <div>
        <label class="label" for="m-notes">Notes</label>
        <textarea id="m-notes" class="input" rows="3" bind:value={notes} placeholder="Firmware, build quirks, where it's stored…"></textarea>
      </div>
      <div class="flex justify-end gap-2 pt-1">
        <button type="button" class="btn-ghost" onclick={cancel}>Cancel</button>
        <button type="submit" class="btn-primary" disabled={busy}>
          {editing ? 'Save changes' : 'Add model'}
        </button>
      </div>
    </form>
  {/if}
</div>
