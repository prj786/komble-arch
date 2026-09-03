<script>
  import { toasts, dismissToast } from "../stores";
  const styles = {
    success: "border-[var(--success)] bg-[color-mix(in_srgb,var(--success)_14%,transparent)] text-success dark:bg-[color-mix(in_srgb,var(--success)_14%,transparent)] dark:text-success",
    error: "border-[var(--danger)] bg-[color-mix(in_srgb,var(--danger)_14%,transparent)] text-danger dark:bg-[color-mix(in_srgb,var(--danger)_14%,transparent)] dark:text-danger",
    info: "border-hairline bg-white text-muted "
  };
</script>

<div class="fixed bottom-4 right-4 z-50 flex w-96 max-w-[90vw] flex-col gap-2">
  {#each $toasts as t (t.id)}
    <button
      class="rounded-xl border px-4 py-2.5 text-left text-sm shadow-lg backdrop-blur {styles[t.type] || styles.info}"
      on:click={() => dismissToast(t.id)}
    >
      {t.message}
    </button>
  {/each}
</div>
