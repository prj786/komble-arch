<script>
	import { Checkbox as CheckboxPrimitive } from "bits-ui";
	import Icon from "../Icon.svelte";
	import { ICONS } from "../icons.js";
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
		"flex size-[18px] items-center justify-center rounded-[5px] border-[length:var(--outline-width)] border-input bg-[var(--card-hover)] transition-colors duration-150 group-has-disabled/field:opacity-50 focus-visible:ring-2 focus-visible:ring-ring aria-invalid:border-destructive data-checked:border-primary data-checked:bg-primary data-checked:text-primary-foreground peer relative shrink-0 outline-none after:absolute after:-inset-x-3 after:-inset-y-2 disabled:cursor-not-allowed disabled:opacity-50",
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
				<Icon code={ICONS.check} size={12} />
			{:else if indeterminate}
				<Icon code={ICONS.minus} size={12} />
			{/if}
		</div>
	{/snippet}
</CheckboxPrimitive.Root>