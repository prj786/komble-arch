<script>
	import { cn } from "$lib/utils.js";
	let {
		ref = $bindable(null),
		class: className,
		children,
		...restProps
	} = $props();
</script>

<div bind:this={ref} data-slot="select-label" class={cn("ewe-menu__group", className)} {...restProps}>
	{@render children?.()}
</div>
