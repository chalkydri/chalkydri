
# NetworkTables API

```text
Chalkydri/
    Robot/
        Position/
            X
            Y
        Rotation
    Devices/
        1/
        2/
        3/
        ...
```

## Chalkydri/Robot/

Topics relevant to the robot as a whole

 - **Chalkydri/Robot/Position/X** *(64-bit float)*
   The robot's current X coord on the field
 - **Chalkydri/Robot/Position/Y** *(64-bit float)*
   The robot's current Y coord on the field
 - **Chalkydri/Robot/Rotation** *(64-bit float)*
   The robot's current rotation

## Chalkydri/Devices/

Each device's device-specific topics are grouped under `Chalkydri/Devices/{device id}/`

 - **Chalkydri/Devices/X/Version** *(string)*
   The device's Chalkydri version

