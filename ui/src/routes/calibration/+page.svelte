<script lang="ts">
	import { Button, Card, Hr, P } from "flowbite-svelte";
	import { _loadConfig } from "../settings/+page";

	let calibrating = $state(false);
	async function calibrate() {
		calibrating = !calibrating;
		if (calibrating) {
			let res = await fetch('/api/calibrate?width=1280&height=720');
			let blob = await res.blob();
			calibrating = false;
		}
	}
</script>

<Card title="Hello">
	<P size="xl">Camera calibration</P>
	<P size="sm" class="my-2">
		Your camera must be calibrated before it will work properly.
		Our calibration process is a little more involved than you might be used to.
		We'll try multiple different resolutions to more accurately calculate the proper parameters for each, so you can switch between resolutions without recalibration!
	</P>

	{#if calibrating}
		<P size="lg" class="text-center mb-2">1280x720</P>
	{/if}

	<Button color={calibrating ? "red" : "blue"} on:click={calibrate}>{#if calibrating}Stop calibration{:else}Start calibration{/if}</Button>
</Card>
