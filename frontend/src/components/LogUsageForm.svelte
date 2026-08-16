<script lang="ts">
  import { api, errorMessage, type Model, type Part, type UsageRecord } from '../lib/api';
  import Flash from './Flash.svelte';

  /**
   * "Log a usage" form. Exactly one side is fixed by the caller:
   *  - on a model page: `model` is fixed; the caller must pass `parts`
   *    scoped to that model's linked parts
   *  - on a part page:  `part` is fixed, the user picks the model
   *  - on the global log page: neither is fixed; once the user picks a
   *    model, the part options narrow to that model's linked parts
   */
  let {
    part,
    model,
    parts = [],
    models = [],
    onLogged,
  }: {
    part?: { id: number; name: string; quantity?: number };
    model?: { id: number; name: string };
    parts?: Part[];
    models?: Model[];
    onLogged: (record: UsageRecord) => Promise<void>;
  } = $props();

  let partSelection = $state<number | ''>('');
  let modelSelection = $state<number | ''>('');
  let quantity = $state(1);
  let notes = $state('');
  let usedAt = $state('');
  let busy = $state(false);
  let formError = $state<string | null>(null);
  let flash = $state<string | null>(null);

  const effectiveModelId = $derived(
    model !== undefined ? model.id : modelSelection === '' ? null : (modelSelection as number),
  );

  // In free form (global log page) narrow the part options to the parts
  // linked to the chosen model; `null` means "no model chosen / lookup
  // failed", in which case the full `parts` list is shown.
  let linkedParts = $state<Part[] | null>(null);

  $effect(() => {
    if (model !== undefined) return; // caller already scoped `parts`
    const modelId = effectiveModelId;
    linkedParts = null;
    if (modelId === null) return;
    let cancelled = false;
    void (async () => {
      try {
        const linked = await api.listModelParts(modelId);
        if (!cancelled) linkedParts = linked;
      } catch {
        if (!cancelled) linkedParts = null;
      }
    })();
    return () => {
      cancelled = true;
    };
  });

  const partOptions = $derived(model !== undefined ? parts : (linkedParts ?? parts));

  // Drop a part selection that is no longer linked to the chosen model.
  $effect(() => {
    const options = partOptions;
    if (part !== undefined || partSelection === '') return;
    if (!options.some((p) => p.id === partSelection)) partSelection = '';
  });

  const ready = $derived(
    (part !== undefined || partSelection !== '') &&
      (model !== undefined || modelSelection !== '') &&
      quantity >= 1,
  );

  async function submit() {
    if (!ready || busy) return;
    busy = true;
    formError = null;
    try {
      // <input type="datetime-local"> gives minute precision; the API wants seconds.
      const used_at = usedAt ? (usedAt.length === 16 ? `${usedAt}:00` : usedAt) : undefined;
      const partId = part !== undefined ? part.id : (partSelection as number);
      const modelId = model !== undefined ? model.id : (modelSelection as number);
      const record =
        part !== undefined && model !== undefined
          ? await api.logUsageForModel(modelId, {
              part_id: partId,
              quantity,
              notes: notes.trim() || undefined,
              used_at,
            })
          : await api.logUsageForPart(partId, {
              model_id: modelId,
              quantity,
              notes: notes.trim() || undefined,
              used_at,
            });
      partSelection = '';
      modelSelection = '';
      quantity = 1;
      notes = '';
      usedAt = '';
      flash = 'Usage logged.';
      setTimeout(() => (flash = null), 2500);
      await onLogged(record);
    } catch (e) {
      formError = errorMessage(e);
    } finally {
      busy = false;
    }
  }
</script>

<Flash message={flash} />
{#if formError}
  <p class="mb-2 text-sm text-rose-700" role="alert">{formError}</p>
{/if}
<div class="flex flex-wrap items-center gap-2">
  {#if part === undefined}
    <select
      class="input max-w-xs"
      bind:value={partSelection}
      disabled={busy}
      aria-label="Part used"
    >
      <option value="" disabled>Part used…</option>
      {#each partOptions as p (p.id)}
        <option value={p.id}>{p.name} — {p.quantity} in stock</option>
      {/each}
      {#if partOptions.length === 0 && effectiveModelId !== null}
        <option value="" disabled>No parts linked to this model</option>
      {/if}
    </select>
  {:else}
    <span class="text-sm text-stone-500">
      Part: <span class="font-medium text-zinc-900">{part.name}</span>
    </span>
  {/if}

  {#if model === undefined}
    <select
      class="input max-w-xs"
      bind:value={modelSelection}
      disabled={busy}
      aria-label="Model the part was used on"
    >
      <option value="" disabled>Used on…</option>
      {#each models as m (m.id)}
        <option value={m.id}>{m.name} ({m.category})</option>
      {/each}
    </select>
  {:else}
    <span class="text-sm text-stone-500">
      Used on: <span class="font-medium text-zinc-900">{model.name}</span>
    </span>
  {/if}

  <label class="flex items-center gap-1.5 text-sm text-stone-600">
    Qty
    <input
      type="number"
      min="1"
      step="1"
      class="input w-20"
      bind:value={quantity}
      disabled={busy}
    />
  </label>
  <input
    type="text"
    class="input max-w-[14rem]"
    placeholder="Notes (e.g. repair)"
    bind:value={notes}
    disabled={busy}
  />
  <input
    type="datetime-local"
    class="input"
    bind:value={usedAt}
    disabled={busy}
    aria-label="When it was used (defaults to now)"
  />
  <button type="button" class="btn-primary" disabled={!ready || busy} onclick={submit}>
    Log usage
  </button>
</div>
