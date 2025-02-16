// place files you want to import through the `$lib` alias in this folder.

export type TagFamily = 'tag36h11';

export interface Config {
	team_number: number,
	subsystems: {
		capriltags: {
			enabled: boolean,
			tag_family: TagFamily,
		},
		apriltags: {
			enabled: boolean,
		},
		machine_learning: {
			enabled: boolean,
		},
	},
}

export { }
