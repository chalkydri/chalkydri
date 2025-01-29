
# AprilTags

We're using our own custom AprilTag library, `chalkydri-apriltags`, built from the ground up.
We'll refer to it as "CAT".

[API documentation (rustdoc)](/doc/chalkydri_apriltags/index.html)

## Why?

We have a few reasons for writing our own, rather than using what's already out there.
The reference C library:
 - is very resource intensive
 - uses a lot of advanced math which isn't covered in high school classes, making it harder for students to understand how it works
 - has memory leaks (needs a citation)

## Design overview

CAT is based on existing algorithms, but with some tweaks specific to our use case.

 1. Get a frame
 2. [Grayscale & threshold](#grayscale--threshold)
 3. [Detect corners](#corner-detection)
 4. [Edge checking](#edge-checking)
 5. [Decode tags](#decode-tags)

## Grayscale & threshold

Converting RGB to grayscale and thresholding are combined into one step for performance.

I can't find the original reference I used for grayscale values.

We need to implement "iterative tri-class adaptive thresholding" based on Otsu's method.

## Corner detection

Corner detection is done using the aptly named FAST algorithm.
Another advantage: it's very simple and easy to understand.

|    |    |    |    |    |    |    |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
|    |    | 16 |  1 |  2 |    |    |
|    | 15 |    |    |    |  3 |    |
| 14 |    |    |    |    |    |  4 |
| 13 |    |    |  p |    |    |  5 |
| 12 |    |    |    |    |    |  6 |
|    | 11 |    |    |    |  7 |    |
|    |    | 10 |  9 |  8 |    |    |

## Edge checking

"Edge checking" reuses some of our corner detection algorithm.

We simply check a few points alongside the paths of each imaginary line between two corners.

![Insert photo here]()

## Decode tags

Decoding tags is done pretty much the same way the C library does it.

## Important references

 - [Real-time Quadrilateral Object Corner Detection Algorithm Based on Deep Learning](/assets/C83.pdf)

