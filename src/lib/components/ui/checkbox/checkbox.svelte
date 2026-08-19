<script>
	import { Checkbox as CheckboxPrimitive } from "bits-ui";
	import PhIcon from "../PhIcon.svelte";
	import { PH } from "../ph.js";
	import { cn } from "$lib/utils.js";

	let {
		ref = $bindable(null),
		checked = $bindable(false),
		indeterminate = $bindable(false),
		class: className,
		...restProps
	} = $props();
</script>

<CheckboxPrimitive.Root
	bind:ref
	data-slot="checkbox"
	class={cn(
		"flex size-4 items-center justify-center rounded-[4px] border border-input shadow-xs transition-shadow group-has-disabled/field:opacity-50 focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 aria-invalid:border-destructive aria-invalid:ring-3 aria-invalid:ring-destructive/20 aria-invalid:aria-checked:border-primary dark:bg-input/30 dark:aria-invalid:border-destructive/50 dark:aria-invalid:ring-destructive/40 data-checked:border-primary data-checked:bg-primary data-checked:text-primary-foreground dark:data-checked:bg-primary peer relative shrink-0 outline-none after:absolute after:-inset-x-3 after:-inset-y-2 disabled:cursor-not-allowed disabled:opacity-50",
		className
	)}
	bind:checked
	bind:indeterminate
	{...restProps}
>
	{#snippet children({ checked, indeterminate })}
		<div
			data-slot="checkbox-indicator"
			class="grid place-content-center text-current transition-none"
		>
			{#if checked}
				<PhIcon code={PH.check} size={11} />
			{:else if indeterminate}
				<!-- Phosphor Fill has no verified codepoint for `minus` in the
				     vendored subset, and a hand-drawn bar is pixel-identical. -->
				<span class="block h-[1.5px] w-2.5 rounded-full bg-current"></span>
			{/if}
		</div>
	{/snippet}
</CheckboxPrimitive.Root>