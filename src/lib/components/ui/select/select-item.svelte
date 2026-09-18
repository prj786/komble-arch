<script>
	// A Menu row; the selected option carries a check in accent-text and its
	// label in weight 500 (Select card).
	import { Select as SelectPrimitive } from "bits-ui";
	import Icon from "../Icon.svelte";
	import { cn } from "$lib/utils.js";

	let {
		ref = $bindable(null),
		class: className,
		value,
		label,
		children: childrenProp,
		...restProps
	} = $props();
</script>

<SelectPrimitive.Item
	bind:ref
	{value}
	data-slot="select-item"
	class={cn("ewe-menu__item", className)}
	{...restProps}
>
	{#snippet children({ selected, highlighted })}
		<span class="ewe-menu__label" class:font-medium={selected}>
			{#if childrenProp}
				{@render childrenProp({ selected, highlighted })}
			{:else}
				{label || value}
			{/if}
		</span>
		{#if selected}
			<Icon name="check" class="ewe-menu__check" />
		{:else}
			<span class="ewe-menu__spacer"></span>
		{/if}
	{/snippet}
</SelectPrimitive.Item>
