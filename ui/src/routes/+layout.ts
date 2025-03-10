import { client } from '$lib/api/client.gen';

export const prerender = true;
export const ssr = false;

var config = client.getConfig();
config.baseUrl = 'http://localhost:6942';
client.setConfig(config);
