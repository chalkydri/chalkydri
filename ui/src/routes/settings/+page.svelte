<script lang="ts">
	import { Button, Card, P, Select, Toggle } from "flowbite-svelte";
	import { _loadConfig } from "./+page";
	import { onMount } from "svelte";
	import type { Config } from "$lib/config";

let saving = $state(false);

function save() {
	saving = true;
}

	let config = $state({} as Config);
	onMount(async () => {
		config = await _loadConfig();
	});
</script>

<Card padding="sm">
	<P size="lg">Subsystems</P>
	<Card padding="xs">
		<Toggle color="blue" disabled={saving} bind:checked={config.subsystems.capriltags.enabled}>C AprilTags</Toggle>
		{#if config.subsystems.capriltags.enabled}
			<Select bind:value={config.subsystems.capriltags.tag_family} items={[
				{ name: '36h11', value: 'tag36h11' },
			]} />
		{/if}
	</Card>
	<Card padding="xs">
		<Toggle color="blue" disabled={saving} bind:checked={config.subsystems.apriltags.enabled}>AprilTags</Toggle>
	</Card>
	<Card padding="xs">
		<Toggle color="blue" disabled={saving} bind:checked={config.subsystems.machine_learning.enabled}>Machine Learning</Toggle>
	</Card>
</Card>

<Card>
	<P>{JSON.stringify(config)}</P>
	<Button color="blue" on:click={save}>{#if saving}Saving...{:else}Save{/if}</Button>
</Card>
