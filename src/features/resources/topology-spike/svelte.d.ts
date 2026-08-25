declare module "*.svelte" {
	import type { Component } from "svelte";

	const component: Component<object>;
	export default component;
}
