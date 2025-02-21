<script lang="ts">
	import { Button, Card, Label, P, Range, Select, Toggle } from "flowbite-svelte";
	import { _loadConfig, _saveConfig } from "./+page";
	import { onMount } from "svelte";
	import type { CameraConfig, Config } from "$lib/config";
	import Slide from "flowbite-svelte/Slide.svelte";

let saving = $state(false);

function save() {
	saving = true;
	_saveConfig(config);
	saving = false;
}

	let config = $state({} as Config);
	let cameras = $state({} as { name: string, value: CameraConfig }[]);
	onMount(async () => {
		config = await _loadConfig();
		cameras = config.cameras.map((val) => {
			return { name: val.name, value: val };
		});
	});

	let camera: CameraConfig|null = $state(null);

	function getSettings(camera: CameraConfig) {
		return camera.caps.map((cap) => {
			return { name: cap.width + 'x' + cap.height + ' @' + (cap.frame_rate.num / cap.frame_rate.den) + 'fps', value: cap };
		});
	}

</script>

<Card padding="sm">
	<P size="lg">Cameras</P>

	<Select bind:value={camera} items={cameras} />
	{#if camera}
		<Select items={getSettings(camera)} />

		<Label class="mt-4 mb-2" for="gamma">Gamma</Label>
		<Range id="gamma" title="Gamma" color="blue" />
	{/if}
</Card>

<!--
{#if config.subsystems}
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
{/if}
-->

<Card>
	<Button color="blue" on:click={save}>{#if saving}Saving...{:else}Save{/if}</Button>
</Card>
