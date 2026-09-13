<script>
	import { Select as SelectPrimitive } from "bits-ui";
	import Icon from "../Icon.svelte";
	import { ICONS } from "../icons.js";
	import { cn } from "$lib/utils.js";

	let {
		ref = $bindable(null),
		class: className,
		children,
		size = "default",
		...restProps
	} = $props();
</script>

<SelectPrimitive.Trigger
	bind:ref
	data-slot="select-trigger"
	data-size={size}
	class={cn(
		"gap-1.5 rounded-md border-[length:var(--outline-width)] border-input bg-[var(--bg-1)] py-2 pr-2.5 pl-3 text-sm transition-[color,background-color,box-shadow] duration-150 hover:bg-[var(--bg-1-hover)] focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset aria-invalid:border-destructive data-placeholder:text-muted-foreground data-[size=default]:h-9 data-[size=sm]:h-8 *:data-[slot=select-value]:flex *:data-[slot=select-value]:gap-1.5 [&_svg:not([class*='size-'])]:size-4 flex w-fit items-center justify-between whitespace-nowrap outline-none disabled:cursor-not-allowed disabled:opacity-50 *:data-[slot=select-value]:line-clamp-1 *:data-[slot=select-value]:flex *:data-[slot=select-value]:items-center [&_svg]:pointer-events-none [&_svg]:shrink-0",
		className
	)}
	{...restProps}
>
	{@render children?.()}
	<Icon code={ICONS.caretDown} size={12} class="pointer-events-none text-muted-foreground" />
</SelectPrimitive.Trigger>