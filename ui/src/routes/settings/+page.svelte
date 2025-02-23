<script lang="ts">
	import { Button, Card, Label, MultiSelect, P, Select, Toggle, } from "flowbite-svelte";
	import { _loadConfig, _saveConfig } from "./+page";
	import { onMount } from "svelte";
	import type { CameraConfig, Config } from "$lib/config";

	let saving = $state(false);
	
	function save() {
		saving = true;
		_saveConfig(config);
		saving = false;
	}

	let config = $state({} as Config);
	let camera_mapping = $state({} as { name: string, value: CameraConfig }[]);
	onMount(async () => {
		config = await _loadConfig();
		camera_mapping = config.cameras.map((val) => {
			return { name: val.display_name, value: val };
		});
	});

	let camera: CameraConfig|null = $state(null);

	function getSettings(camera: CameraConfig) {
		return camera.possible_settings.map((s) => {
			return { name: `${s.width}x${s.height} @${s.frame_rate.num / s.frame_rate.den}fps`, value: s };
		});
	}
</script>

<Card padding="sm">
	<P size="lg">Cameras</P>

	{#each config.cameras as camera}
		<Card padding="xs" class="my-2">
			<P>{camera.display_name}</P>

			<Label for="res_fps" class="mt-2 mb-1">Resolution / Frame Rate</Label>
			<Select id="res_fps" items={getSettings(camera)} bind:value={camera.settings} />


			<Label for="subsystems" class="mt-2 mb-1">Subsystems</Label>
			<MultiSelect id="subsystems" items={[
				{ name: 'CAprilTags', value: 'capriltags' },
				{ name: 'Machine Learning', value: 'ml' },
			]}  />

			<!--
			<Label class="mt-4 mb-2" for="gamma">Gamma</Label>
			<Range id="gamma" title="Gamma" color="blue" />
			-->
		</Card>
	{/each}
</Card>

{#if config.subsystems}
<Card padding="sm" class="mt-2">
	<P size="lg">Subsystems</P>
	<Card padding="xs" class="mt-2">
		<Toggle color="blue" disabled={saving} bind:checked={config.subsystems.capriltags.enabled}>C AprilTags</Toggle>
		<!--
		{#if config.subsystems.capriltags.enabled}
			<Select bind:value={config.subsystems.capriltags.tag_family} items={[
				{ name: '36h11', value: 'tag36h11' },
			]} />
		{/if}
		-->
	</Card>
	<!--
	<Card padding="xs">
		<Toggle color="blue" disabled={saving} bind:checked={config.subsystems.apriltags.enabled}>AprilTags</Toggle>
	</Card>
	-->
	<Card padding="xs" class="mt-2">
		<Toggle color="blue" disabled={saving} bind:checked={config.subsystems.ml.enabled}>Machine Learning</Toggle>
	</Card>
</Card>
{/if}

<Card class="mt-2">
	<Button color="blue" on:click={save}>{#if saving}Saving...{:else}Save{/if}</Button>
</Card>
