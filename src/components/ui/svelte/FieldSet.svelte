<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<fieldset
	data-slot="field-set"
	class={cnfast(
		"flex flex-col gap-4 has-[>[data-slot=checkbox-group]]:gap-3 has-[>[data-slot=radio-group]]:gap-3",
		className,
	)}
	{...rest}
>
	{@render children?.()}
</fieldset>
