<script>
	import { DropdownMenu as DropdownMenuPrimitive } from "bits-ui";
	import Icon from "../Icon.svelte";
	import { ICONS } from "../icons.js";
	import { cn } from "$lib/utils.js";
	let {
		ref = $bindable(null),
		checked = $bindable(false),
		indeterminate = $bindable(false),
		class: className,
		children: childrenProp,
		...restProps
	} = $props();
</script>

<DropdownMenuPrimitive.CheckboxItem
	bind:ref
	bind:checked
	bind:indeterminate
	data-slot="dropdown-menu-checkbox-item"
	class={cn("ewe-menu__item", className)}
	{...restProps}
>
	{#snippet children({ checked, indeterminate })}
		<span class="ewe-menu__check order-last ml-auto"
			data-slot="dropdown-menu-checkbox-item-indicator"
		>
			{#if indeterminate}
				<Icon name="minus" />
			{:else if checked}
				<Icon name="check" />
			{/if}
		</span>
		{@render childrenProp?.()}
	{/snippet}
</DropdownMenuPrimitive.CheckboxItem>