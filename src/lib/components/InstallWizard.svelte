<script>
  // macOS-DMG-style install wizard for dropped .AppImage files.
  import { wizardFile, progress, toast } from "../stores";
  import { settings } from "../stores";
  import { refreshInstalled } from "../actions";
  import * as api from "../api";
  import { slugify, formatBytes } from "../utils";

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
      ? "Adding to your app menu…"
      : "Copying into place…";

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

<svelte:window on:keydown={(e) => e.key === "Escape" && step !== "installing" && close()} />

{#if $wizardFile && info}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-6 backdrop-blur-sm">
    <div class="card w-full max-w-md overflow-hidden !bg-white text-center shadow-2xl dark:!bg-zinc-900">
      {#if step === "done"}
        <div class="p-8">
          <div class="mx-auto flex h-16 w-16 items-center justify-center rounded-full bg-green-500/15">
            <svg viewBox="0 0 24 24" class="h-8 w-8 text-green-500" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
              <path d="M20 6L9 17l-5-5" />
            </svg>
          </div>
          <h2 class="mt-4 text-lg font-bold">{name} installed</h2>
          <p class="mt-1 text-sm text-zinc-400">
            You'll find it in your application menu. Komble will keep it in the
            Installed list.
          </p>
          <button class="btn-primary mt-6 w-full" on:click={close}>Done</button>
        </div>
      {:else if step === "installing"}
        <div class="p-8">
          <div
            class="mx-auto flex h-16 w-16 items-center justify-center rounded-2xl text-2xl font-bold text-white"
            style="background: var(--accent)"
          >
            {name.slice(0, 1).toUpperCase()}
          </div>
          <h2 class="mt-4 text-lg font-bold">Installing {name}…</h2>
          <p class="mt-1 text-sm text-zinc-400">{phaseLabel}</p>
          <div class="mt-5 h-1 w-full overflow-hidden rounded-full bg-zinc-200 dark:bg-zinc-700">
            <div class="h-full w-1/3 animate-pulse rounded-full" style="background: var(--accent)"></div>
          </div>
        </div>
      {:else}
        <div class="p-8 pb-6">
          <div
            class="mx-auto flex h-16 w-16 items-center justify-center rounded-2xl text-2xl font-bold text-white"
            style="background: var(--accent)"
          >
            {(name || "?").slice(0, 1).toUpperCase()}
          </div>
          <h2 class="mt-4 text-lg font-bold">Install this AppImage?</h2>
          <p class="mx-auto mt-1 max-w-xs truncate text-xs text-zinc-400" title={info.path}>
            {info.path}{info.size ? ` · ${formatBytes(info.size)}` : ""}
          </p>

          <div class="mt-5 space-y-3 text-left">
            <label class="block">
              <span class="mb-1 block text-xs font-medium text-zinc-500 dark:text-zinc-400">Name</span>
              <input class="input" bind:value={name} />
            </label>
            <label class="block">
              <span class="mb-1 block text-xs font-medium text-zinc-500 dark:text-zinc-400">Version</span>
              <input class="input" bind:value={version} placeholder="local" />
            </label>
          </div>

          {#if error}
            <p class="mt-3 text-xs text-red-500">{error}</p>
          {/if}

          <div class="mt-6 flex gap-2">
            <button class="btn-ghost flex-1" on:click={close}>Cancel</button>
            <button class="btn-primary flex-1" disabled={!name.trim()} on:click={install}>
              Install
            </button>
          </div>
          <p class="mt-4 text-[11px] leading-relaxed text-zinc-400">
            The file is copied to your AppImage folder, made executable, and gets
            a menu entry + icon. No root needed.
          </p>
        </div>
      {/if}
    </div>
  </div>
{/if}
