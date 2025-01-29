
# Chalkydri system design

Chalkydri has a somewhat complicated design, as most vision things do.

Once the robot is powered on, each Chalkydri device will:
 - Boot up
 - Attempt to connect to the roboRIO's NetworkTables server
 - Initialize camera(s)
 - Start subsystems

Chalkydri waits until it connects to the NetworkTables server successfully to actually start running.
It will negotiate with the roboRIO and start processing frames.

