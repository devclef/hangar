<script lang="ts">
  let {
    qty,
    onAdjust,
  }: {
    qty: number;
    onAdjust: (delta: number) => Promise<void>;
  } = $props();

  let busy = $state(false);

  async function step(delta: number) {
    if (busy) return;
    busy = true;
    try {
      await onAdjust(delta);
    } finally {
      busy = false;
    }
  }
</script>

<div class="inline-flex items-center rounded-md border border-stone-300 bg-white">
  <button
    type="button"
    class="h-7 w-7 select-none text-stone-500 transition-colors hover:text-rose-600 disabled:opacity-30"
    disabled={busy || qty <= 0}
    onclick={() => step(-1)}
    aria-label="Decrease quantity"
  >−</button>
  <span
    class="w-8 text-center text-sm tabular-nums {qty === 0
      ? 'font-bold text-rose-600'
      : qty <= 2
        ? 'font-semibold text-amber-700'
        : 'text-stone-800'}"
  >{qty}</span>
  <button
    type="button"
    class="h-7 w-7 select-none text-stone-500 transition-colors hover:text-emerald-700 disabled:opacity-30"
    disabled={busy}
    onclick={() => step(1)}
    aria-label="Increase quantity"
  >+</button>
</div>
