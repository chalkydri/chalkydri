
# Chalkydri system design

Chalkydri has a somewhat complicated design, as most vision things do, but I really hate poor documentation.

Once the robot is powered on, each Chalkydri device will:
 - Boot up (almost certainly faster than everything else, because Alpine is awesome like that)
 - Attempt to connect to the roboRIO's NetworkTables server
 - Initialize camera(s)
 - Initialize ML accelerator(s) if applicable
 - Prepare backends

Chalkydri waits until it connects to the NetworkTables server successfully to actually start running.
It will negotiate with the roboRIO and start processing frames.

