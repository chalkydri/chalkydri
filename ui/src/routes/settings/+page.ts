import { type Config } from "$lib/config";

async function _saveConfig(config: Config) {
	let res = await fetch('/api/configuration', {
		method: 'POST',
		body: JSON.stringify(config),
	});
	return await res.json() as Config;
}
async function _loadConfig() {
	let req = new Request('/api/configuration');
	return await req.json() as Config;
}

export { _loadConfig, _saveConfig }
