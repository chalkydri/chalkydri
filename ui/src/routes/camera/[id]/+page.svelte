<script lang="ts">
	import Calibration from '../../+components/Calibration.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<Calibration cam_name={data.id} />
