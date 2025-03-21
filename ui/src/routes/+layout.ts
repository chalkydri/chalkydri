import { client } from '$lib/api/client.gen';
import { dev } from '$app/environment';

export const prerender = true;
export const ssr = false;

var config = client.getConfig();
if (dev) {
	config.baseUrl = 'http://localhost:6942';
}
client.setConfig(config);
