<script>
	import { DropdownMenu as DropdownMenuPrimitive } from "bits-ui";
	import Icon from "../Icon.svelte";
	import { ICONS } from "../icons.js";
	import { cn } from "$lib/utils.js";

	let {
		ref = $bindable(null),
		class: className,
		children: childrenProp,
		...restProps
	} = $props();
</script>

<DropdownMenuPrimitive.RadioItem
	bind:ref
	data-slot="dropdown-menu-radio-item"
	class={cn("ewe-menu__item", className)}
	{...restProps}
>
	{#snippet children({ checked })}
		<span class="ewe-menu__check order-last ml-auto"
			data-slot="dropdown-menu-radio-item-indicator"
		>
			{#if checked}
				<Icon name="check" />
			{/if}
		</span>
		{@render childrenProp?.({ checked })}
	{/snippet}
</DropdownMenuPrimitive.RadioItem>