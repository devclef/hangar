<script lang="ts">
  import { api, type Settings, type ThemeMode } from './lib/api';
  import { effectiveMode, setThemeMode, themeMode as themeStore } from './lib/theme';
  import { parseRoute } from './lib/router';
  import type { Route } from './lib/router';
  import ModelsPage from './routes/ModelsPage.svelte';
  import ModelDetail from './routes/ModelDetail.svelte';
  import CatalogPage from './routes/CatalogPage.svelte';
  import CatalogModelPage from './routes/CatalogModelPage.svelte';
  import ModelFormPage from './routes/ModelFormPage.svelte';
  import PartListPage from './routes/PartListPage.svelte';
  import PartDetail from './routes/PartDetail.svelte';
  import PartFormPage from './routes/PartFormPage.svelte';
  import UsagePage from './routes/UsagePage.svelte';
  import SettingsPage from './routes/SettingsPage.svelte';

  let route = $state<Route>(parseRoute());
  let settings = $state<Settings | null>(null);
  let themeMode = $state<ThemeMode>('system');

  $effect(() => {
    const onHash = () => {
      route = parseRoute();
      window.scrollTo({ top: 0 });
    };
    window.addEventListener('hashchange', onHash);
    return () => window.removeEventListener('hashchange', onHash);
  });

  // Apply the persisted theme on load; the settings page and the header
  // toggle both go through the shared theme store.
  $effect(() => {
    api
      .getSettings()
      .then((s) => {
        settings = s;
        setThemeMode(s.theme);
      })
      .catch(() => {
        setThemeMode('system');
      });
    const unsub = themeStore.subscribe((m) => (themeMode = m));
    return () => unsub();
  });

  /** Flip to the opposite scheme and persist it as the default mode. */
  async function toggleTheme() {
    const next: ThemeMode = effectiveMode(themeMode) === 'dark' ? 'light' : 'dark';
    setThemeMode(next); // optimistic
    try {
      const fresh = await api.getSettings();
      settings = await api.updateSettings({ ...fresh, theme: next });
    } catch {
      try {
        const fresh = await api.getSettings();
        settings = fresh;
        setThemeMode(fresh.theme);
      } catch {
        // keep the optimistic state; the next load re-syncs
      }
    }
  }

  const isModel = $derived(route.page.startsWith('model'));
  const isPart = $derived(route.page.startsWith('part'));
  const isCatalog = $derived(route.page === 'catalog' || route.page === 'catalog-model');
  const isUsage = $derived(route.page === 'usage');
  const isSettings = $derived(route.page === 'settings');
</script>

<div class="flex min-h-screen flex-col bg-stone-100 dark:bg-zinc-950 text-stone-900 dark:text-zinc-100">
  <header class="bg-zinc-900 text-zinc-100">
    <div class="mx-auto flex h-14 w-full max-w-6xl items-center gap-5 px-4">
      <a href="#/models" class="flex items-center gap-2.5">
        <svg class="h-6 w-6" viewBox="0 0 32 32" aria-hidden="true">
          <g stroke="#f59e0b" stroke-width="2.5">
            <line x1="12" y1="12" x2="6" y2="6" />
            <line x1="20" y1="12" x2="26" y2="6" />
            <line x1="12" y1="20" x2="6" y2="26" />
            <line x1="20" y1="20" x2="26" y2="26" />
          </g>
          <rect x="11" y="11" width="10" height="10" rx="3" fill="#f59e0b" />
          <g fill="#a1a1aa">
            <circle cx="6" cy="6" r="4" />
            <circle cx="26" cy="6" r="4" />
            <circle cx="6" cy="26" r="4" />
            <circle cx="26" cy="26" r="4" />
          </g>
        </svg>
        <span class="text-base font-bold tracking-[0.25em]">HANGAR</span>
        <span class="hidden text-xs text-zinc-400 sm:inline">RC inventory</span>
      </a>
      <nav class="flex gap-1 text-sm">
        <a href="#/models" class="nav-link {isModel
          ? 'bg-zinc-800 text-amber-300'
          : 'text-zinc-300 hover:bg-zinc-800/60 hover:text-white'}">
          Models
        </a>
        <a
          href="#/parts"
          class="nav-link {isPart
            ? 'bg-zinc-800 text-amber-300'
            : 'text-zinc-300 hover:bg-zinc-800/60 hover:text-white'}"
        >
          Parts
        </a>
        <a
          href="#/catalog"
          class="nav-link {isCatalog
            ? 'bg-zinc-800 text-amber-300'
            : 'text-zinc-300 hover:bg-zinc-800/60 hover:text-white'}"
        >
          Catalog
        </a>
        <a
          href="#/usage"
          class="nav-link {isUsage
            ? 'bg-zinc-800 text-amber-300'
            : 'text-zinc-300 hover:bg-zinc-800/60 hover:text-white'}"
        >
          Log
        </a>
        <a
          href="#/settings"
          class="nav-link {isSettings
            ? 'bg-zinc-800 text-amber-300'
            : 'text-zinc-300 hover:bg-zinc-800/60 hover:text-white'}"
        >
          Settings
        </a>
      </nav>
      <div class="ml-auto flex items-center gap-3">
        <button
          type="button"
          class="rounded-md p-1.5 text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-amber-300"
          title={themeMode === 'system'
            ? 'Theme: system (follows your OS) — click to pin the opposite of the current look'
            : 'Theme: ' + themeMode + ' — click to switch to ' + (themeMode === 'dark' ? 'light' : 'dark')}
          aria-label="Toggle light/dark theme"
          onclick={toggleTheme}
        >
          {#if effectiveMode(themeMode) === 'dark'}
            <svg
              class="h-4.5 w-4.5"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <circle cx="12" cy="12" r="4"></circle>
              <path
                d="M12 2v2m0 16v2M4.93 4.93l1.41 1.41m11.32 11.32 1.41 1.41M2 12h2m16 0h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41"
              ></path>
            </svg>
          {:else}
            <svg
              class="h-4.5 w-4.5"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <path
                d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"
              ></path>
            </svg>
          {/if}
        </button>
        <span class="text-xs text-zinc-500">v0.1</span>
      </div>
    </div>
  </header>

  <main class="mx-auto w-full max-w-6xl flex-1 px-4 py-6">
    {#if route.page === 'models'}
      <ModelsPage />
    {:else if route.page === 'model'}
      <ModelDetail id={route.id} />
    {:else if route.page === 'model-form'}
      <ModelFormPage id={route.id} />
    {:else if route.page === 'parts'}
      <PartListPage />
    {:else if route.page === 'part'}
      <PartDetail id={route.id} />
    {:else if route.page === 'part-form'}
      <PartFormPage id={route.id} />
    {:else if route.page === 'catalog'}
      <CatalogPage />
    {:else if route.page === 'catalog-model'}
      <CatalogModelPage id={route.id} />
    {:else if route.page === 'usage'}
      <UsagePage />
    {:else if route.page === 'settings'}
      <SettingsPage />
    {/if}
  </main>

  <footer class="border-t border-stone-200 dark:border-zinc-800 bg-white dark:bg-zinc-900">
    <div class="mx-auto w-full max-w-6xl px-4 py-3 text-xs text-stone-400 dark:text-zinc-500">
      Hangar · single-user, self-hosted
    </div>
  </footer>
</div>
