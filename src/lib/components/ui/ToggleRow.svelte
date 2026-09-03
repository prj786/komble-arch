<script>
  import Toggle from "./Toggle.svelte";
  export let title = "";
  export let sub = "";
  export let on = false;
  export let dim = false;
  export let toggled = () => {};
</script>

<div class="flex items-center justify-between gap-3 px-4 py-3 {dim ? 'opacity-50' : ''}">
  <div class="min-w-0">
    <div class="text-sm font-medium">{title}</div>
    {#if sub}<div class="text-xs text-dim dark:text-dim">{sub}</div>{/if}
  </div>
  <Toggle {on} disabled={dim} {toggled} />
</div>
