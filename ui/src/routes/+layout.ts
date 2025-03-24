import { client } from '$lib/api/client.gen';
import { dev } from '$app/environment';

export const prerender = true;
export const ssr = true;
export const csr = true;

var config = client.getConfig();
if (dev) {
	config.baseUrl = 'http://localhost:6942';
}
client.setConfig(config);
