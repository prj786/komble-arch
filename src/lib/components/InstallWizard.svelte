<script>
  // The install wizard for a dropped or opened .AppImage (Install wizard
  // card): the sheep and a Step indicator on top, one decision per step, a
  // footer with "Step n of 3", ghost Back/Cancel and a primary xl action. In
  // a floating window over a scrim; Esc cancels except while installing.
  import { Dialog } from "bits-ui";
  import { wizardFile, progress, toast } from "../stores";
  import { settings } from "../stores";
  import { refreshInstalled } from "../actions";
  import * as api from "../api";
  import { slugify, formatBytes } from "../utils";
  import sheep from "../../assets/sheep.svg?raw";
  import Icon from "./ui/Icon.svelte";

  let info = null;
  let name = "";
  let version = "";
  let step = "info"; // info | installing | done
  let error = "";
  let lastPath = null;

  $: if ($wizardFile && $wizardFile !== lastPath) load($wizardFile);
  $: wizId = slugify(name);
  $: prog = $progress[wizId];
  $: phaseLabel =
    prog?.phase === "integrating"
      ? "Adding it to your app menu…"
      : "Copying it into place…";
  $: current = step === "info" ? 0 : step === "installing" ? 1 : 2;
  const steps = ["Details", "Install", "Done"];

  async function load(path) {
    lastPath = path;
    step = "info";
    error = "";
    info = null;
    try {
      info = await api.appimageFileInfo(path);
      name = info.suggestedName;
      version = info.suggestedVersion;
    } catch (e) {
      toast(e, "error");
      close();
    }
  }

  async function install() {
    step = "installing";
    error = "";
    try {
      await api.installLocalAppimage(
        info.path,
        name.trim() || info.suggestedName,
        version.trim(),
        $settings.appimageDir
      );
      refreshInstalled();
      step = "done";
    } catch (e) {
      error = String(e);
      step = "info";
    }
  }

  function close() {
    wizardFile.set(null);
    lastPath = null;
    step = "info";
    error = "";
  }
</script>

{#if $wizardFile && info}
  <Dialog.Root open={true} onOpenChange={(v) => !v && step !== "installing" && close()}>
    <Dialog.Portal>
      <Dialog.Overlay class="scrim" />
      <Dialog.Content class="ewe-wizard is-floating" interactOutsideBehavior="ignore" escapeKeydownBehavior={step === "installing" ? "ignore" : "close"}>
        <div class="ewe-wizard__top">
          <span class="ewe-sidenav__logo" aria-hidden="true">{@html sheep}</span>
          <ol class="ewe-steps" aria-label="Step {current + 1} of {steps.length}">
            {#each steps as label, i}
              {#if i > 0}<span class="ewe-steps__line" class:is-done={i <= current} aria-hidden="true"></span>{/if}
              <li class="ewe-step" class:is-done={i < current} class:is-current={i === current} aria-current={i === current ? "step" : undefined}>
                <span class="ewe-step__dot">{#if i < current}<Icon name="check" />{:else}{i + 1}{/if}</span>{label}
              </li>
            {/each}
          </ol>
        </div>

        <div class="ewe-wizard__body">
          {#if step === "done"}
            <span class="ewe-wizard__done"><Icon name="success" /></span>
            <Dialog.Title class="ewe-wizard__title">Installed {name}</Dialog.Title>
            <Dialog.Description class="ewe-wizard__desc">
              It’s in your app menu, and Komble keeps it in the Installed list.
            </Dialog.Description>
          {:else if step === "installing"}
            <Dialog.Title class="ewe-wizard__title">Installing {name}…</Dialog.Title>
            <Dialog.Description class="ewe-wizard__desc">{phaseLabel}</Dialog.Description>
            <div class="ewe-progress ewe-progress--lg ewe-progress--indeterminate" role="progressbar" aria-label="Installing {name}">
              <div class="ewe-progress__track"><div class="ewe-progress__fill"></div></div>
            </div>
          {:else}
            <Dialog.Title class="ewe-wizard__title">Install this AppImage?</Dialog.Title>
            <Dialog.Description class="ewe-wizard__desc">
              Komble copies it to your AppImage folder, makes it executable and adds a menu entry with its icon. No password needed.
            </Dialog.Description>
            <div class="ewe-file">
              <Icon name="fileBox" />
              <div class="ewe-file__body">
                <span class="ewe-file__name" title={info.path}>{info.path.split("/").pop()}</span>
                <span class="ewe-file__meta">{info.path.split("/").slice(0, -1).join("/")}{info.size ? ` · ${formatBytes(info.size)}` : ""}</span>
              </div>
            </div>
            <div class="form-grid">
              <label class="ewe-field">
                <span class="ewe-field__label">Name</span>
                <span class="ewe-input ewe-input--lg" class:is-error={!name.trim()}><input class="field-text" bind:value={name} /></span>
                {#if !name.trim()}<span class="ewe-field__helper ewe-field__helper--error"><Icon name="alert" />Give the app a name.</span>{/if}
              </label>
              <label class="ewe-field">
                <span class="ewe-field__label">Version <span class="ewe-field__optional">Optional</span></span>
                <span class="ewe-input ewe-input--lg"><input class="field-text" bind:value={version} placeholder="local" /></span>
              </label>
            </div>
            {#if error}
              <div class="ewe-alert ewe-alert--danger" role="alert">
                <Icon name="alert" />
                <div class="ewe-alert__body">
                  <div class="ewe-alert__title">Couldn’t install {name}</div>
                  <div class="ewe-alert__desc">{error}</div>
                </div>
              </div>
            {/if}
          {/if}
        </div>

        <div class="ewe-wizard__foot">
          <span class="ewe-meta">Step {current + 1} of {steps.length}</span>
          {#if step === "done"}
            <button class="ewe-btn ewe-btn--xl ewe-btn--primary" on:click={close}>Done</button>
          {:else if step === "installing"}
            <button class="ewe-btn ewe-btn--xl ewe-btn--primary" disabled aria-busy="true">
              <span class="ewe-spinner ewe-spinner--sm ewe-spinner--on-accent" aria-hidden="true"></span>Installing…
            </button>
          {:else}
            <button class="ewe-btn ewe-btn--xl ewe-btn--ghost" on:click={close}>Cancel</button>
            <button class="ewe-btn ewe-btn--xl ewe-btn--primary" disabled={!name.trim()} on:click={install}>
              <Icon name="download" />Install
            </button>
          {/if}
        </div>
      </Dialog.Content>
    </Dialog.Portal>
  </Dialog.Root>
{/if}
