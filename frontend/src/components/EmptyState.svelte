<script lang="ts">
  let {
    title,
    hint,
    actionHref,
    actionLabel,
  }: {
    title: string;
    hint?: string;
    actionHref?: string;
    actionLabel?: string;
  } = $props();
</script>

<div class="flex flex-col items-center justify-center gap-1.5 py-16 text-center">
  <svg
    class="mb-1 h-10 w-10 text-stone-300"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.5"
    aria-hidden="true"
  >
    <path
      stroke-linecap="round"
      stroke-linejoin="round"
      d="m21 7.5-9-5.25L3 7.5m18 0-9 5.25m9-5.25v9l-9 5.25M3 7.5l9 5.25M3 7.5v9l9 5.25m0-9v9"
    />
  </svg>
  <p class="text-sm font-medium text-stone-600">{title}</p>
  {#if hint}
    <p class="text-xs text-stone-400">{hint}</p>
  {/if}
  {#if actionHref && actionLabel}
    <a class="btn-primary mt-3" href={actionHref}>{actionLabel}</a>
  {/if}
</div>
