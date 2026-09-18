<script>
	// Checkbox (design/system/components/Checkbox): bits-ui for role, keyboard
	// and the tri-state; .ewe-check__box for the look. Wrapped in .ewe-check so
	// hovering the label highlights the box too.
	import { Checkbox as CheckboxPrimitive } from "bits-ui";
	import Icon from "../Icon.svelte";
	import { cn } from "$lib/utils.js";

	let {
		ref = $bindable(null),
		checked = $bindable(false),
		indeterminate = $bindable(false),
		class: className,
		label = "",
		...restProps
	} = $props();
</script>

<label class={cn("ewe-check", checked && "is-checked", className)}>
	<CheckboxPrimitive.Root
		bind:ref
		data-slot="checkbox"
		class="ewe-check__box"
		bind:checked
		bind:indeterminate
		{...restProps}
	>
		{#snippet children({ checked, indeterminate })}
			{#if indeterminate}
				<Icon name="minus" size="xs" />
			{:else if checked}
				<Icon name="check" size="xs" />
			{/if}
		{/snippet}
	</CheckboxPrimitive.Root>
	{#if label}<span class="ewe-check__label">{label}</span>{/if}
</label>
