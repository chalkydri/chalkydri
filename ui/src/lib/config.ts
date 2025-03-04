// place files you want to import through the `$lib` alias in this folder.

export type TagFamily = 'tag36h11';

export interface CameraSettings {
	width: number;
	height: number;
	frame_rate: {
		num: number;
		den: number;
	};
}

export interface CameraConfig {
	name: string;
	display_name: string;
	settings: CameraSettings | null;
	possible_settings: CameraSettings[];
}

export interface Config {
	team_number: number | null;
	device_name: string | null;
	cameras: [CameraConfig];
	subsystems: {
		capriltags: {
			enabled: boolean;
			gamma: number | null;
		};
		ml: {
			enabled: boolean;
		};
	};
}

export {};
