<script lang="ts">
  import type { Category } from '../lib/api';

  let { category }: { category: Category } = $props();

  const styles: Record<Category, string> = {
    heli: 'bg-emerald-100 text-emerald-800',
    plane: 'bg-sky-100 text-sky-800',
    car: 'bg-amber-100 text-amber-800',
    drone: 'bg-violet-100 text-violet-800',
    boat: 'bg-cyan-100 text-cyan-800',
    other: 'bg-stone-200 text-stone-700',
  };
</script>

<span
  class="inline-flex rounded px-1.5 py-0.5 text-xs font-semibold uppercase tracking-wide {styles[category]}"
>{category}</span>
