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
	class={cn(
		"gap-2 rounded-[calc(var(--radius-control)-4px)] px-2 py-1.5 text-sm focus:bg-primary focus:text-primary-foreground not-data-[variant=destructive]:focus:**:text-primary-foreground data-inset:pl-8 data-open:bg-primary data-open:text-primary-foreground [&_svg:not([class*='size-'])]:size-4 flex cursor-default items-center outline-hidden select-none data-[inset]:pl-8 [&_svg]:pointer-events-none [&_svg]:shrink-0",
		className
	)}
	{...restProps}
>
	{@render children?.()}
	<Icon code={ICONS.caretRight} size={12} class="ml-auto" />
</DropdownMenuPrimitive.SubTrigger>