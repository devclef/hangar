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
  let savingLowStock = $state(false);
  let thresholdDraft = $state('');
  let savingThreshold = $state(false);

  async function load() {
    error = null;
    try {
      const s = await api.getSettings();
      settings = s;
      currencyDraft = s.currency;
      thresholdDraft = String(s.low_stock_threshold);
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
      low_stock_enabled: s.low_stock_enabled,
      low_stock_threshold: s.low_stock_threshold,
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

  async function toggleLowStock() {
    const s = settings;
    if (!s || savingLowStock) return;
    savingLowStock = true;
    error = null;
    try {
      settings = await api.updateSettings({ ...s, low_stock_enabled: !s.low_stock_enabled });
      flashOk('Low stock setting saved.');
    } catch (e) {
      error = errorMessage(e);
    } finally {
      savingLowStock = false;
    }
  }

  async function saveThreshold() {
    const s = settings;
    if (!s || savingThreshold) return;
    const t = Number(thresholdDraft);
    if (thresholdDraft.trim() === '' || !Number.isInteger(t) || t < 0 || t > 1000) {
      error = 'Threshold must be a whole number between 0 and 1000.';
      return;
    }
    if (t === s.low_stock_threshold) return;
    savingThreshold = true;
    error = null;
    try {
      settings = await api.updateSettings({ ...s, low_stock_threshold: t });
      thresholdDraft = String(settings.low_stock_threshold);
      flashOk('Low stock threshold saved.');
    } catch (e) {
      error = errorMessage(e);
    } finally {
      savingThreshold = false;
    }
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
        <h2 class="text-sm font-semibold uppercase tracking-wide text-stone-600">Low stock</h2>
        <p class="mt-1 text-xs text-stone-400">
          Parts at or below the threshold show an amber "low" badge in the parts list. Individual
          parts can opt out on their edit page.
        </p>
      </div>
      <div class="flex items-center justify-between gap-3 border-b border-stone-100 px-5 py-3">
        <div class="min-w-0">
          <span class="block text-sm font-medium text-stone-800">Low stock warnings</span>
          <span class="block truncate text-xs text-stone-400">
            Turn off to never flag parts as low
          </span>
        </div>
        <button
          type="button"
          role="switch"
          aria-checked={settings.low_stock_enabled}
          aria-label="Toggle low stock warnings"
          disabled={savingLowStock}
          class="relative h-5 w-9 shrink-0 rounded-full transition-colors {settings.low_stock_enabled
            ? 'bg-zinc-900'
            : 'bg-stone-300'} disabled:opacity-40"
          onclick={toggleLowStock}
        >
          <span
            class="absolute top-0.5 left-0.5 h-3.5 w-3.5 rounded-full bg-white shadow transition-transform {settings.low_stock_enabled
              ? 'translate-x-4'
              : 'translate-x-0'}"
          ></span>
        </button>
      </div>
      <div class="flex items-end gap-3 px-5 py-4">
        <div class="w-32">
          <label class="label" for="low-threshold">Threshold</label>
          <input
            id="low-threshold"
            class="input"
            type="number"
            min="0"
            max="1000"
            step="1"
            bind:value={thresholdDraft}
            onkeydown={(e) => e.key === 'Enter' && void saveThreshold()}
          />
        </div>
        <button
          type="button"
          class="btn-primary"
          disabled={savingThreshold || Number(thresholdDraft) === settings.low_stock_threshold}
          onclick={() => void saveThreshold()}
        >
          {savingThreshold ? 'Saving…' : 'Save'}
        </button>
      </div>
      <p class="px-5 pb-4 text-xs text-stone-400">
        A part is "low" when its quantity is at or below this value. Set to 0 to never flag parts
        as low (out-of-stock is always shown).
      </p>
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
