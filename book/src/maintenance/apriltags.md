
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
 4. [Cluster & filter outliers](#cluster--filter-outliers) (TODO)
 5. [Find convex hulls](#find-convex-hulls)
 6. [Check convex hulls](#check-convex-hulls)
 7. [Decode tags](#decode-tags)

## Grayscale & threshold

Converting RGB to grayscale and thresholding are combined into one step for performance.

I can't find the original reference I used for grayscale values.

For adaptive thresholding, "Bradley's method" seems like the best option. It's described in [Adaptive Thresholding Using the Integral Image](https://people.scs.carleton.ca/~roth/iit-publications-iti/docs/gerh-50002.pdf).

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

## Cluster & filter outliers

OPTICS[^1] is an algorithm for clustering and outlier detection.

It's similar to [DBSCAN](), which is described pretty well by Wikipedia:
 > given a set of points in some space, it groups together points that are closely packed (points with many nearby neighbors), and marks as outliers points that lie alone in low-density regions (those whose nearest neighbors are too far away).

You can read more about it in [the Wikipedia article](https://en.wikipedia.org/wiki/OPTICS_algorithm).

## Find convex hulls

Gift wrapping algorithm

## Check convex hulls

TODO

## Decode tags

Decoding tags is done pretty much the same way the C library does it.

## Important references

 - [Real-time Quadrilateral Object Corner Detection Algorithm Based on Deep Learning](/assets/C83.pdf)

[^1]: https://en.wikipedia.org/wiki/OPTICS_algorithm ([PDF](/assets/OPTICS_algorithm.pdf))
