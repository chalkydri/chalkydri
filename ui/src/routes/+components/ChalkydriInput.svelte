<script lang="ts">
	import type { Camera } from '$lib/api';
	import { client } from '$lib/api/client.gen';
	import { Card, Label } from 'flowbite-svelte';
	import type { Snippet } from 'svelte';

	let { id = $bindable(), label = $bindable(null), children }: { id: string, label: string | null, children: Snippet } = $props();
</script>

<div>
	<Label for="field" class="mt-2 mb-1">{ label }</Label>
  {@render children()}
</div>
