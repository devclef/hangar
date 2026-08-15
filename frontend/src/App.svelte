<script lang="ts">
  import { parseRoute } from './lib/router';
  import type { Route } from './lib/router';
  import ModelsPage from './routes/ModelsPage.svelte';
  import ModelDetail from './routes/ModelDetail.svelte';
  import ModelFormPage from './routes/ModelFormPage.svelte';
  import PartListPage from './routes/PartListPage.svelte';
  import PartDetail from './routes/PartDetail.svelte';
  import PartFormPage from './routes/PartFormPage.svelte';
  import SettingsPage from './routes/SettingsPage.svelte';

  let route = $state<Route>(parseRoute());

  $effect(() => {
    const onHash = () => {
      route = parseRoute();
      window.scrollTo({ top: 0 });
    };
    window.addEventListener('hashchange', onHash);
    return () => window.removeEventListener('hashchange', onHash);
  });

  const isModel = $derived(route.page.startsWith('model'));
  const isPart = $derived(route.page.startsWith('part'));
  const isSettings = $derived(route.page === 'settings');
</script>

<div class="flex min-h-screen flex-col bg-stone-100 text-stone-900">
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
          href="#/settings"
          class="nav-link {isSettings
            ? 'bg-zinc-800 text-amber-300'
            : 'text-zinc-300 hover:bg-zinc-800/60 hover:text-white'}"
        >
          Settings
        </a>
      </nav>
      <span class="ml-auto text-xs text-zinc-500">v0.1</span>
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
    {:else if route.page === 'settings'}
      <SettingsPage />
    {/if}
  </main>

  <footer class="border-t border-stone-200 bg-white">
    <div class="mx-auto w-full max-w-6xl px-4 py-3 text-xs text-stone-400">
      Hangar · single-user, self-hosted
    </div>
  </footer>
</div>
