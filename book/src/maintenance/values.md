
# Chalkydri ~~Manifesto~~ code quality standards

We're trying to make vision less of a black box, so all FRC teams can use it.
We also want it to be easier to use effectively with less hassle.

To do this, we need to set some standards for our codebase.

### KISS
Whenever possible and logical, opt for a simpler algorithm that doesn't use higher level math.
The less code, the better.

### Write good documentation
Documentation should be easy to understand for people that aren't super familiar with the codebase.
With complex concepts, it's appropriate to give a higher-level summary and link to a Wikipedia article.

### Minimize unsafe (/low-level) code
It's easier to make mistakes in unsafe code.
While Chalkydri core forbids unsafe code, some of our supporting crates contain unsafe code.
Whenever possible without a significant performance impact, stick to safe code.

### Think of the user
The UI should be easy and straightforward to navigate, even for people that aren't familiar with programming or computer vision.

