<script lang="ts">
  import {
    api,
    errorMessage,
    PART_FORM_FIELDS,
    type PartFormField,
    type Settings,
  } from '../lib/api';
  import ErrorBanner from '../components/ErrorBanner.svelte';
  import Flash from '../components/Flash.svelte';
  import Spinner from '../components/Spinner.svelte';

  let settings = $state<Settings | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let flash = $state<string | null>(null);
  let savingField = $state<PartFormField | null>(null);
  let currencyDraft = $state('');
  let savingCurrency = $state(false);

  async function load() {
    error = null;
    try {
      const s = await api.getSettings();
      settings = s;
      currencyDraft = s.currency;
    } catch (e) {
      error = errorMessage(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void load();
  });

  function flashOk(msg: string) {
    flash = msg;
    setTimeout(() => (flash = null), 2500);
  }

  const enabled = $derived(new Set(settings?.part_form_fields ?? []));

  function toggle(key: PartFormField) {
    const s = settings;
    if (!s || savingField) return;
    const next = new Set(s.part_form_fields);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    // Preserve the canonical field order in the stored list.
    const nextSettings: Settings = {
      currency: s.currency,
      part_form_fields: PART_FORM_FIELDS.map((f) => f.key).filter((k) => next.has(k)),
    };
    savingField = key;
    api
      .updateSettings(nextSettings)
      .then((saved) => {
        settings = saved;
        flashOk('Settings saved.');
      })
      .catch((e) => {
        error = errorMessage(e);
      })
      .finally(() => {
        savingField = null;
      });
  }

  async function saveCurrency() {
    const s = settings;
    if (!s || savingCurrency) return;
    const code = currencyDraft.trim().toUpperCase();
    if (code === s.currency) return;
    savingCurrency = true;
    error = null;
    try {
      settings = await api.updateSettings({ ...s, currency: code });
      currencyDraft = settings.currency;
      flashOk('Currency saved.');
    } catch (e) {
      error = errorMessage(e);
    } finally {
      savingCurrency = false;
    }
  }
</script>

<div class="mx-auto max-w-2xl">
  <h1 class="mb-4 text-xl font-bold text-zinc-900">Settings</h1>

  {#if loading && !settings}
    <div class="flex items-center justify-center gap-2 py-16 text-sm text-stone-500">
      <Spinner /> Loading…
    </div>
  {:else if settings}
    <Flash message={flash} />
    {#if error}
      <div class="mb-3"><ErrorBanner message={error} /></div>
    {/if}

    <div class="card">
      <div class="border-b border-stone-200 px-5 py-3">
        <h2 class="text-sm font-semibold uppercase tracking-wide text-stone-600">Part form fields</h2>
        <p class="mt-1 text-xs text-stone-400">
          Choose which fields show when you add or edit a part. Hidden fields still exist — they're
          just kept out of the form.
        </p>
      </div>
      <ul class="divide-y divide-stone-100">
        {#each PART_FORM_FIELDS as f (f.key)}
          <li class="flex items-center justify-between gap-3 px-5 py-3">
            <div class="min-w-0">
              <span class="block text-sm font-medium text-stone-800">{f.label}</span>
              <span class="block truncate text-xs text-stone-400">{f.hint}</span>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={enabled.has(f.key)}
              aria-label="Toggle {f.label}"
              disabled={savingField === f.key}
              class="relative h-5 w-9 shrink-0 rounded-full transition-colors {enabled.has(f.key)
                ? 'bg-zinc-900'
                : 'bg-stone-300'} disabled:opacity-40"
              onclick={() => toggle(f.key)}
            >
              <span
                class="absolute top-0.5 left-0.5 h-3.5 w-3.5 rounded-full bg-white shadow transition-transform {enabled.has(f.key)
                  ? 'translate-x-4'
                  : 'translate-x-0'}"
              ></span>
            </button>
          </li>
        {/each}
      </ul>
    </div>

    <div class="card mt-6">
      <div class="border-b border-stone-200 px-5 py-3">
        <h2 class="text-sm font-semibold uppercase tracking-wide text-stone-600">Display</h2>
      </div>
      <div class="flex items-end gap-3 px-5 py-4">
        <div class="w-32">
          <label class="label" for="currency">Currency</label>
          <input
            id="currency"
            class="input uppercase"
            bind:value={currencyDraft}
            placeholder="USD"
            maxlength="8"
            spellcheck="false"
            onkeydown={(e) => e.key === 'Enter' && void saveCurrency()}
          />
        </div>
        <button
          type="button"
          class="btn-primary"
          disabled={savingCurrency || currencyDraft.trim().toUpperCase() === settings.currency}
          onclick={() => void saveCurrency()}
        >
          {savingCurrency ? 'Saving…' : 'Save'}
        </button>
      </div>
      <p class="px-5 pb-4 text-xs text-stone-400">
        ISO-4217 code used to show part costs, e.g. USD, EUR, or GBP.
      </p>
    </div>
  {:else if error}
    <ErrorBanner message={error} onRetry={load} />
  {/if}
</div>
