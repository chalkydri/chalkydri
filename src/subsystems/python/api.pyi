'''
Chalkydri Python API definitions
'''

# https://pyo3.rs/v0.23.5/class.html
# https://thegreenalliance.dev/
# https://www.chiefdelphi.com/t/reef-vision-4145/482921
# https://www.chiefdelphi.com/t/is-full-field-localization-with-april-tags-good-enough-for-auto-alignment/484094/3
# https://www.chiefdelphi.com/t/squaring-and-aligning-robot-with-april-tag/494159/6
# https://www.chiefdelphi.com/t/introducing-the-green-alliance/492911

import numpy as np

def run(image: np.array) -> dict[str, any]:
    '''
    Frame processing function

    This will be executed on each frame.
    '''
    pass
