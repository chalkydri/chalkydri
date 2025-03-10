import './api';
import { configuration, type Config } from './api';
import { writable, type Writable } from 'svelte/store';
import { onMount } from 'svelte';

const config: Writable<Config> = writable();
onMount(async () => {
	let loaded_config = (await configuration()).data;
	if (loaded_config) {
		config.set(loaded_config);
	}
});

export { config };
