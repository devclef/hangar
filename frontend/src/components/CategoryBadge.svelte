<script lang="ts">
  import type { Category } from '../lib/api';

  let { category }: { category: Category } = $props();

  const styles: Record<Category, string> = {
    heli: 'bg-emerald-100 dark:bg-emerald-500/15 text-emerald-800 dark:text-emerald-400',
    plane: 'bg-sky-100 dark:bg-sky-500/15 text-sky-800 dark:text-sky-400',
    car: 'bg-amber-100 dark:bg-amber-500/15 text-amber-800 dark:text-amber-400',
    drone: 'bg-violet-100 dark:bg-violet-500/15 text-violet-800 dark:text-violet-400',
    boat: 'bg-cyan-100 dark:bg-cyan-500/15 text-cyan-800 dark:text-cyan-400',
    other: 'bg-stone-200 dark:bg-zinc-700 text-stone-700 dark:text-zinc-300',
  };
</script>

<span
  class="inline-flex rounded px-1.5 py-0.5 text-xs font-semibold uppercase tracking-wide {styles[category]}"
>{category}</span>
