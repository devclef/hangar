<script lang="ts">
  import {
    api,
    errorMessage,
    PART_FORM_FIELDS,
    type PartFormField,
    type Settings,
    type ThemeMode,
  } from '../lib/api';
  import { setThemeMode, themeMode as themeStore } from '../lib/theme';
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
  let themeChoice = $state<ThemeMode>('system');
  let savingTheme = $state(false);

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
    const unsub = themeStore.subscribe((m) => (themeChoice = m));
    return () => unsub();
  });

  /**
   * Persist a change. The header's theme toggle can also write settings,
   * so re-read the document first and only the patched keys change.
   */
  async function saveSettings(patch: Partial<Settings>): Promise<Settings> {
    const fresh = await api.getSettings();
    settings = await api.updateSettings({ ...fresh, ...patch });
    return settings;
  }

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
    const nextFields = PART_FORM_FIELDS.map((f) => f.key).filter((k) => next.has(k));
    savingField = key;
    saveSettings({ part_form_fields: nextFields })
      .then(() => flashOk('Settings saved.'))
      .catch((e) => {
        error = errorMessage(e);
      })
      .finally(() => {
        savingField = null;
      });
  }

  async function toggleLowStock() {
    if (!settings || savingLowStock) return;
    savingLowStock = true;
    error = null;
    try {
      const fresh = await api.getSettings();
      await saveSettings({ low_stock_enabled: !fresh.low_stock_enabled });
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
      const saved = await saveSettings({ low_stock_threshold: t });
      thresholdDraft = String(saved.low_stock_threshold);
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
      const saved = await saveSettings({ currency: code });
      currencyDraft = saved.currency;
      flashOk('Currency saved.');
    } catch (e) {
      error = errorMessage(e);
    } finally {
      savingCurrency = false;
    }
  }

  async function pickTheme(mode: ThemeMode) {
    if (!settings || savingTheme || mode === themeChoice) return;
    savingTheme = true;
    error = null;
    setThemeMode(mode); // optimistic; the store updates themeChoice everywhere
    try {
      await saveSettings({ theme: mode });
      flashOk('Theme saved.');
    } catch (e) {
      error = errorMessage(e);
      // revert to the server-side value
      try {
        const fresh = await api.getSettings();
        setThemeMode(fresh.theme);
      } catch {
        // keep the optimistic choice
      }
    } finally {
      savingTheme = false;
    }
  }
</script>

<div class="mx-auto max-w-2xl">
  <h1 class="mb-4 text-xl font-bold text-zinc-900 dark:text-zinc-100">Settings</h1>

  {#if loading && !settings}
    <div class="flex items-center justify-center gap-2 py-16 text-sm text-stone-500 dark:text-zinc-400">
      <Spinner /> Loading…
    </div>
  {:else if settings}
    <Flash message={flash} />
    {#if error}
      <div class="mb-3"><ErrorBanner message={error} /></div>
    {/if}

    <div class="card">
      <div class="border-b border-stone-200 dark:border-zinc-800 px-5 py-3">
        <h2 class="text-sm font-semibold uppercase tracking-wide text-stone-600 dark:text-zinc-400">Part form fields</h2>
        <p class="mt-1 text-xs text-stone-400 dark:text-zinc-500">
          Choose which fields show when you add or edit a part. Hidden fields still exist — they're
          just kept out of the form.
        </p>
      </div>
      <ul class="divide-y divide-stone-100 dark:divide-zinc-800">
        {#each PART_FORM_FIELDS as f (f.key)}
          <li class="flex items-center justify-between gap-3 px-5 py-3">
            <div class="min-w-0">
              <span class="block text-sm font-medium text-stone-800 dark:text-zinc-200">{f.label}</span>
              <span class="block truncate text-xs text-stone-400 dark:text-zinc-500">{f.hint}</span>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={enabled.has(f.key)}
              aria-label="Toggle {f.label}"
              disabled={savingField === f.key}
              class="relative h-5 w-9 shrink-0 rounded-full transition-colors {enabled.has(f.key)
                ? 'bg-zinc-900 dark:bg-amber-500'
                : 'bg-stone-300 dark:bg-zinc-700'} disabled:opacity-40"
              onclick={() => toggle(f.key)}
            >
              <span
                class="absolute top-0.5 left-0.5 h-3.5 w-3.5 rounded-full bg-white dark:bg-zinc-900 shadow transition-transform {enabled.has(f.key)
                  ? 'translate-x-4'
                  : 'translate-x-0'}"
              ></span>
            </button>
          </li>
        {/each}
      </ul>
    </div>

    <div class="card mt-6">
      <div class="border-b border-stone-200 dark:border-zinc-800 px-5 py-3">
        <h2 class="text-sm font-semibold uppercase tracking-wide text-stone-600 dark:text-zinc-400">Low stock</h2>
        <p class="mt-1 text-xs text-stone-400 dark:text-zinc-500">
          Parts at or below the threshold show an amber "low" badge in the parts list. Individual
          parts can opt out on their edit page.
        </p>
      </div>
      <div class="flex items-center justify-between gap-3 border-b border-stone-100 dark:border-zinc-800 px-5 py-3">
        <div class="min-w-0">
          <span class="block text-sm font-medium text-stone-800 dark:text-zinc-200">Low stock warnings</span>
          <span class="block truncate text-xs text-stone-400 dark:text-zinc-500">
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
            ? 'bg-zinc-900 dark:bg-amber-500'
            : 'bg-stone-300 dark:bg-zinc-700'} disabled:opacity-40"
          onclick={toggleLowStock}
        >
          <span
            class="absolute top-0.5 left-0.5 h-3.5 w-3.5 rounded-full bg-white dark:bg-zinc-900 shadow transition-transform {settings.low_stock_enabled
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
      <p class="px-5 pb-4 text-xs text-stone-400 dark:text-zinc-500">
        A part is "low" when its quantity is at or below this value. Set to 0 to never flag parts
        as low (out-of-stock is always shown).
      </p>
    </div>

    <div class="card mt-6">
      <div class="border-b border-stone-200 dark:border-zinc-800 px-5 py-3">
        <h2 class="text-sm font-semibold uppercase tracking-wide text-stone-600 dark:text-zinc-400">Appearance</h2>
        <p class="mt-1 text-xs text-stone-400 dark:text-zinc-500">
          Default color theme. The sun/moon button in the header flips between light and dark at
          any time.
        </p>
      </div>
      <ul class="divide-y divide-stone-100 dark:divide-zinc-800">
        {#each [
          { m: 'system', label: 'System', hint: 'Follow your OS light/dark preference' },
          { m: 'light', label: 'Light', hint: 'Always the light theme' },
          { m: 'dark', label: 'Dark', hint: 'Always the dark theme' },
        ] as Array<{ m: ThemeMode; label: string; hint: string }> as opt (opt.m)}
          <li class="flex items-center justify-between gap-3 px-5 py-3">
            <label class="flex min-w-0 cursor-pointer items-start gap-2.5">
              <input
                type="radio"
                name="theme"
                class="mt-0.5 size-4 shrink-0 accent-zinc-900 dark:accent-amber-400"
                checked={themeChoice === opt.m}
                disabled={savingTheme}
                onchange={() => void pickTheme(opt.m)}
              />
              <span class="min-w-0">
                <span class="block text-sm font-medium text-stone-800 dark:text-zinc-200">{opt.label}</span>
                <span class="block truncate text-xs text-stone-400 dark:text-zinc-500">{opt.hint}</span>
              </span>
            </label>
          </li>
        {/each}
      </ul>
    </div>

    <div class="card mt-6">
      <div class="border-b border-stone-200 dark:border-zinc-800 px-5 py-3">
        <h2 class="text-sm font-semibold uppercase tracking-wide text-stone-600 dark:text-zinc-400">Display</h2>
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
      <p class="px-5 pb-4 text-xs text-stone-400 dark:text-zinc-500">
        ISO-4217 code used to show part costs, e.g. USD, EUR, or GBP.
      </p>
    </div>
  {:else if error}
    <ErrorBanner message={error} onRetry={load} />
  {/if}
</div>
