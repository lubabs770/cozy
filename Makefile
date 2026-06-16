# cozy — convenience wrappers around the Docker verification harness.
#
# The host is macOS/arm64, so cozy cannot build or run here directly; everything
# happens inside the Linux container defined in docker/Dockerfile.

IMAGE  := cozy-verify
OUT    := out

.PHONY: image verify shell lint fmt clean

# Build the verification image (slow first time; cached after).
image:
	docker build -f docker/Dockerfile -t $(IMAGE) .

# Build cozy + run it under headless sway + capture frames into ./out.
# Pass extra cozy args via ARGS, e.g.  make verify ARGS="--config test.toml"
verify: image
	mkdir -p $(OUT)
	docker run --rm \
		-v "$(PWD)":/work \
		-v "$(PWD)/$(OUT)":/out \
		-e COZY_ARGS="$(ARGS)" \
		$(IMAGE)

# Drop into the container for poking around.
shell: image
	docker run --rm -it -v "$(PWD)":/work --entrypoint /bin/bash $(IMAGE)

# rustfmt + clippy inside the container.
lint: image
	docker run --rm -v "$(PWD)":/work --entrypoint /bin/bash $(IMAGE) -c \
		"cargo fmt --manifest-path /work/Cargo.toml --check && \
		 cargo clippy --manifest-path /work/Cargo.toml -- -D warnings"

fmt: image
	docker run --rm -v "$(PWD)":/work --entrypoint /bin/bash $(IMAGE) -c \
		"cargo fmt --manifest-path /work/Cargo.toml"

clean:
	rm -rf $(OUT) target
