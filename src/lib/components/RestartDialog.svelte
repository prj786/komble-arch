<script>
  // "The update is installed but not running yet." Shown once after an
  // upgrade that touched the kernel / the session / the shell, with the one
  // action that finishes it. Later = the Updates view keeps a card.
  import { restartNeed, toast } from "../stores";
  import * as api from "../api";
  let show = false;
  let last = null;
  $: if ($restartNeed && $restartNeed !== last) {
    last = $restartNeed;
    show = $restartNeed.level !== "none";
  }
  const titles = {
    reboot: "Restart the computer to finish updating",
    logout: "Log out and back in to finish updating",
    shell: "Restart the desktop shell to finish updating",
    komble: "Komble was updated — relaunch it"
  };
  const buttons = {
    reboot: "Restart now",
    logout: "Log out",
    shell: "Restart the shell",
    komble: "Relaunch Komble"
  };
  async function go() {
    const kind = $restartNeed.level;
    try {
      await api.restartAction(kind);
      if (kind === "shell") {
        toast("Desktop shell restarting…", "success");
        restartNeed.set(null);
      }
      show = false;
    } catch (e) {
      toast(e, "error");
    }
  }
</script>

<svelte:window on:keydown={(e) => show && e.key === "Escape" && (show = false)} />

{#if show && $restartNeed}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-3 backdrop-blur-sm sm:p-6" role="dialog" aria-modal="true">
    <div class="card flex w-full max-w-lg flex-col overflow-hidden !bg-white shadow-2xl dark:!bg-[var(--bg-3)]">
      <div class="border-b border-hairline p-5">
        <h2 class="text-base font-semibold">{titles[$restartNeed.level]}</h2>
        <p class="mt-1 text-sm text-dim">The new files are on disk; what is running is still the old version.</p>
      </div>
      <ul class="flex flex-col gap-1 p-5 text-sm">
        {#each $restartNeed.reasons as r}
          <li class="flex gap-2"><span class="text-dim">•</span><span>{r}</span></li>
        {/each}
      </ul>
      <div class="flex justify-end gap-2 border-t border-hairline p-4">
        <button class="btn-ghost" on:click={() => (show = false)}>Later</button>
        <button class="btn-primary" on:click={go}>{buttons[$restartNeed.level]}</button>
      </div>
    </div>
  </div>
{/if}
