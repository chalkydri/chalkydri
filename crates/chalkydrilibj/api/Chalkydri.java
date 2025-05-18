package me.waterga.chalkydri;

/**
 * The entrypoint for chalkydrilib
 */
public class Chalkydri {
	static {
		System.loadLibrary("chalkydrilib");
	}

	/**
	 * Get the Camera with the given name
	 */
	public static native Camera getCamera(String name);

	private static native double[] calculateRobotPose();

	public static Pose2d getRobotPose() {
		double[] poseCoords = Chalkydri.calculateRobotPose();
		return new Pose2d(poseCoords[0], poseCoords[1], poseCoords[2]);
	}
}
