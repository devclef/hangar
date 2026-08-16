<script lang="ts">
  import type { ModelStatus } from '../lib/api';

  let { status }: { status: ModelStatus } = $props();

  const styles: Record<ModelStatus, { cls: string; dot: string; label: string }> = {
    active: { cls: 'text-emerald-700 dark:text-emerald-400', dot: 'bg-emerald-500', label: 'Active' },
    retired: { cls: 'text-stone-500 dark:text-zinc-400', dot: 'bg-stone-400', label: 'Retired' },
    sold: { cls: 'text-rose-600 dark:text-rose-400', dot: 'bg-rose-400', label: 'Sold' },
  };
  const s = $derived(styles[status]);
</script>

<span class="inline-flex items-center gap-1.5 text-xs font-medium {s.cls}">
  <span class="h-1.5 w-1.5 rounded-full {s.dot}"></span>
  {s.label}
</span>
