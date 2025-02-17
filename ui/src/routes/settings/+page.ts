import { type Config } from "$lib/config";

async function _loadConfig() {
	let req = new Request('/api/configuration');
	return await req.json() as Config;
}

export { _loadConfig }
