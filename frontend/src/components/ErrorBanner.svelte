<script lang="ts">
  let {
    message,
    onRetry,
  }: {
    message: string;
    onRetry?: () => void;
  } = $props();
</script>

<div class="flex items-center justify-between gap-3 rounded-md border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
  <span class="min-w-0 break-words">{message}</span>
  {#if onRetry}
    <button type="button" class="shrink-0 font-medium underline hover:no-underline" onclick={onRetry}>
      Retry
    </button>
  {/if}
</div>
