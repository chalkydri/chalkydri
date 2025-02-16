<script lang="ts">
	import { Button, Card, P, Select, Toggle } from "flowbite-svelte";
	import { type Config } from "$lib/config";

	let saving = $state(false);
	let config = $state({
		subsystems: {
			capriltags: {
				enabled: true,
				tag_family: 'tag36h11',
			},
			apriltags: {
				enabled: true,
			},
			machine_learning: {
				enabled: true,
			},
		},
	} as Config);

	function save() {
		saving = true;
	}
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
