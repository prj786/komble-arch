<script>
	import { DropdownMenu as DropdownMenuPrimitive } from "bits-ui";
	import Icon from "../Icon.svelte";
	import { ICONS } from "../icons.js";
	import { cn } from "$lib/utils.js";

	let {
		ref = $bindable(null),
		class: className,
		inset,
		children,
		...restProps
	} = $props();
</script>

<DropdownMenuPrimitive.SubTrigger
	bind:ref
	data-slot="dropdown-menu-sub-trigger"
	data-inset={inset}
	class={cn("ewe-menu__item", className)}
	{...restProps}
>
	{@render children?.()}
	<Icon name="caretRight" class="ml-auto" />
</DropdownMenuPrimitive.SubTrigger>