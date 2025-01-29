.POSIX:

DOCKER_COMMAND := docker

# Build dev container
dev:
	${DOCKER_COMMAND} build -t chalkydri:dev -f Containerfile

# Build release builder container
builder:
	${DOCKER_COMMAND} build -t chalkydri:builder -f Containerfile.builder

# Build release images
build: builder
	${DOCKER_COMMAND} run --name chalkydri-builder chalkydri:builder

all: dev

.PHONY: dev builder build all