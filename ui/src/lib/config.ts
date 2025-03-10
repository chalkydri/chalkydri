import { configuration, configure, type Config } from './api';
import { writable, type Writable } from 'svelte/store';

export var config: Config | null = null;

export async function updateConfig(cfg: Config) {
	let loaded_config = (
		await configure({
			body: cfg
		})
	).data;
	if (loaded_config) {
		config = loaded_config;
	}
}
export async function loadConfig() {
	let loaded_config = (await configuration()).data;
	if (loaded_config) {
		config = loaded_config;
	}
}

export {};
