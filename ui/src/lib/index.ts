import './api';
import { configuration, info, type Config, type Info } from './api';
import { writable, type Writable } from 'svelte/store';
import { onMount } from 'svelte';

const config: Writable<Config> = writable();
//onMount(async () => {
document.addEventListener('onload', async function () {
	try {
		let loaded_config = (await configuration()).data;
		if (loaded_config) {
			config.set(loaded_config);
		}
	} catch (e) {}
});
//});

const connected: Writable<boolean> = writable();
const sys_info: Writable<Info | null> = writable();

connected.set(false);

//onMount(() => {
setInterval(async function () {
	await info().then(
		(res) => {
			if (res.data) {
				sys_info.set(res.data);
			}
			connected.set(true);
		},
		(res) => {
			sys_info.set(null);
			connected.set(false);
		}
	);
}, 500);
//});

export { config, connected, sys_info };
