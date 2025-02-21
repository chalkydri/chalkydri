// place files you want to import through the `$lib` alias in this folder.

export type TagFamily = 'tag36h11';

export interface CameraSettings {
	width: number,
	height: number,
	frame_rate: {
		num: number,
		den: number,
	},
}

export interface CameraConfig {
	name: string,
	settings: CameraSettings|null,
	caps: CameraSettings[],
}

export interface Config {
	team_number: number,
	cameras: [CameraConfig],
	//subsystems: {
	//	capriltags: {
	//		enabled: boolean,
	//		tag_family: TagFamily,
	//	},
	//	apriltags: {
	//		enabled: boolean,
	//	},
	//	machine_learning: {
	//		enabled: boolean,
	//	},
	//},
}

export { }
