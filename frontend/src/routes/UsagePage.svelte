<script lang="ts">
  import {
    api,
    errorMessage,
    type Model,
    type Part,
    type UsageRecord,
  } from '../lib/api';
  import LogUsageForm from '../components/LogUsageForm.svelte';
  import UsageLog from '../components/UsageLog.svelte';
  import ErrorBanner from '../components/ErrorBanner.svelte';
  import Spinner from '../components/Spinner.svelte';

  let records = $state<UsageRecord[]>([]);
  let parts = $state<Part[]>([]);
  let models = $state<Model[]>([]);
  let partFilter = $state<number | ''>('');
  let modelFilter = $state<number | ''>('');
  let loading = $state(true);
  let error = $state<string | null>(null);

  async function load() {
    error = null;
    try {
      records = await api.listUsage({
        part_id: partFilter === '' ? undefined : partFilter,
        model_id: modelFilter === '' ? undefined : modelFilter,
      });
    } catch (e) {
      error = errorMessage(e);
    } finally {
      loading = false;
    }
  }

  // Options for the form and the filters; best-effort so the log itself
  // still renders if either list fails.
  async function loadOptions() {
    try {
      [parts, models] = await Promise.all([
        api.listParts({ sort: 'name_asc' }),
        api.listModels(),
      ]);
    } catch {
      // keep whatever already loaded
    }
  }

  $effect(() => {
    void loadOptions();
  });

  $effect(() => {
    void partFilter;
    void modelFilter;
    void load();
  });
</script>

{#if loading && records.length === 0}
  <div class="flex items-center justify-center gap-2 py-16 text-sm text-stone-500 dark:text-zinc-400">
    <Spinner /> Loading…
  </div>
{:else if error && records.length === 0}
  <ErrorBanner message={error} onRetry={load} />
{:else}
  <div class="mb-4 flex flex-wrap items-center justify-between gap-2">
    <h1 class="text-2xl font-bold text-zinc-900 dark:text-zinc-100">Usage log</h1>
    <span class="text-sm text-stone-500 dark:text-zinc-400">
      {records.length} entr{records.length === 1 ? 'y' : 'ies'}
    </span>
  </div>

  {#if error}
    <div class="mb-3"><ErrorBanner message={error} /></div>
  {/if}

  <div class="card p-4">
    <LogUsageForm parts={parts} models={models} onLogged={async () => await load()} />
  </div>

  <div class="card mt-6">
    <div class="flex flex-wrap items-center justify-between gap-2 border-b border-stone-200 dark:border-zinc-800 px-4 py-3">
      <h2 class="text-sm font-semibold uppercase tracking-wide text-stone-600 dark:text-zinc-400">
        History
      </h2>
      <div class="flex flex-wrap items-center gap-2">
        <select
          class="input max-w-[12rem]"
          bind:value={partFilter}
          aria-label="Filter by part"
        >
          <option value="">All parts</option>
          {#each parts as p (p.id)}
            <option value={p.id}>{p.name}</option>
          {/each}
        </select>
        <select
          class="input max-w-[12rem]"
          bind:value={modelFilter}
          aria-label="Filter by model"
        >
          <option value="">All models</option>
          {#each models as m (m.id)}
            <option value={m.id}>{m.name}</option>
          {/each}
        </select>
      </div>
    </div>
    <UsageLog records={records} />
  </div>
{/if}
