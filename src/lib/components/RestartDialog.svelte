<script>
  // "The update is installed but not running yet." Shown once after an
  // upgrade that touched the kernel / the session / the shell, with the one
  // action that finishes it. Later = the Updates view keeps an alert.
  // Dialog card, confirm: the title is the question, the primary button
  // repeats its verb, "Later" is the way out; Esc is Later.
  import { Dialog } from "bits-ui";
  import { restartNeed, toast } from "../stores";
  import * as api from "../api";
  import Icon from "./ui/Icon.svelte";
  let show = false;
  let last = null;
  $: if ($restartNeed && $restartNeed !== last) {
    last = $restartNeed;
    show = $restartNeed.level !== "none";
  }
  const titles = {
    reboot: "Restart to finish updating?",
    logout: "Sign out to finish updating?",
    shell: "Restart the shell to finish updating?",
    komble: "Relaunch Komble to finish updating?"
  };
  const buttons = {
    reboot: "Restart now",
    logout: "Sign out",
    shell: "Restart shell",
    komble: "Relaunch Komble"
  };
  async function go() {
    const kind = $restartNeed.level;
    try {
      await api.restartAction(kind);
      if (kind === "shell") {
        toast("Restarting the shell…", "info");
        restartNeed.set(null);
      }
      show = false;
    } catch (e) {
      toast(e, "error");
    }
  }
</script>

{#if show && $restartNeed}
  <Dialog.Root open={true} onOpenChange={(v) => !v && (show = false)}>
    <Dialog.Portal>
      <Dialog.Overlay class="scrim" />
      <Dialog.Content class="ewe-dialog is-floating" interactOutsideBehavior="ignore">
        <div class="ewe-dialog__head">
          <span class="ewe-dialog__icon"><Icon name={$restartNeed.level === "logout" ? "logOut" : "rotate"} /></span>
          <div class="ewe-dialog__titles">
            <Dialog.Title class="ewe-dialog__title">{titles[$restartNeed.level]}</Dialog.Title>
            <Dialog.Description class="ewe-dialog__desc">The new files are installed, but the old version is still running.</Dialog.Description>
          </div>
        </div>
        {#if $restartNeed.reasons?.length}
          <div class="ewe-dialog__body">
            <ul class="reasons">
              {#each $restartNeed.reasons as r}<li>{r}</li>{/each}
            </ul>
          </div>
        {/if}
        <div class="ewe-dialog__foot">
          <button class="ewe-btn ewe-btn--ghost" on:click={() => (show = false)}>Later</button>
          <button class="ewe-btn ewe-btn--primary" on:click={go}>{buttons[$restartNeed.level]}</button>
        </div>
      </Dialog.Content>
    </Dialog.Portal>
  </Dialog.Root>
{/if}
