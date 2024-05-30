
# AprilTags

We're using our own custom AprilTag library.

## Why?

We have a few reasons for writing our own, rather than using what's already out there:
 - The reference C library is very resource intensive.

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

## Important references

 - [Real-time Quadrilateral Object Corner Detection Algorithm Based on Deep Learning](https://ceie.szu.edu.cn/heyejun/paper/C83.pdf)

