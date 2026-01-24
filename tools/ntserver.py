#!/usr/bin/env -S uv run --project tools
'''
Run a NetworkTables server
'''

from ntcore import NetworkTableInstance as NTInstance
import time
import logging

def main():
    logging.basicConfig(filename='ntserver.log', level=logging.DEBUG)
    inst = NTInstance.getDefault()
    inst.configPythonLogging(min=NTInstance.LogLevel.kLogDebug4)

    inst.startServer()

    while True:
        topics = inst.getTable("/chalkydri").getTopics()
        if len(topics) > 0:
            print(topics)
        time.sleep(0.5)


if __name__ == '__main__':
    main()
