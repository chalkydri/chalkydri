<script lang="ts">
	import type { Camera } from '$lib/api';
	import { client } from '$lib/api/client.gen';
	import { Card } from 'flowbite-svelte';

	let { camera = $bindable(null) }: { camera: Camera | null } = $props();
</script>

{#if camera}
	<Card class="h-min" padding="xs">
		<img
			src={`${client.getConfig().baseUrl ? client.getConfig().baseUrl : ''}/stream/${camera.id}`}
			alt={camera.name}
		/>
	</Card>
{/if}
