import { type Config } from "$lib/config";

async function _saveConfig(config: Config) {
	let res = await fetch('/api/configuration', {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
		},
		body: JSON.stringify(config),
	});
	return await res.json() as Config;
}
async function _loadConfig() {
	let res = await fetch('/api/configuration');
	return await res.json() as Config;
}

export { _loadConfig, _saveConfig }
