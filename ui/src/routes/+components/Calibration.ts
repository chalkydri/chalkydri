import { type CalibrationStatus } from '$lib/calibration';

async function _getCalibStatus() {
	let res = await fetch(`/api/calibrate/status`);
	return (await res.json()) as CalibrationStatus;
}

async function _doCalibStep() {
	let res = await fetch(`/api/calibrate/step`);
	return (await res.json()) as CalibrationStatus;
}

export { _getCalibStatus, _doCalibStep };
