# Verification image for cozy.
#
# We never ship a binary from here — this image only *verifies* cozy by running
# it as a real wlr-layer-shell client against a headless sway compositor with
# software rendering (Mesa llvmpipe), then screenshotting with grim.
#
# Source is bind-mounted at /work and built inside; output PNGs go to /out.
FROM rust:1-bookworm

# rustfmt + clippy aren't guaranteed for the image's resolved toolchain; add
# them so `make lint` works regardless of the base image.
RUN rustup component add rustfmt clippy

RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config \
        libwayland-dev \
        libwayland-egl1 \
        libxkbcommon-dev \
        wayland-protocols \
        libegl-dev \
        libgles-dev \
        libgl1-mesa-dri \
        libglvnd-dev \
        sway \
        grim \
    && rm -rf /var/lib/apt/lists/*

# Force software rendering everywhere (no GPU in the container).
# sway refuses to run as root, so we run as an unprivileged user and let the
# headless backend skip seat/session management entirely.
ENV LIBGL_ALWAYS_SOFTWARE=1 \
    GALLIUM_DRIVER=llvmpipe \
    WLR_BACKENDS=headless \
    WLR_RENDERER=pixman \
    WLR_LIBINPUT_NO_DEVICES=1 \
    XDG_RUNTIME_DIR=/run/user/1000 \
    WAYLAND_DISPLAY=wayland-1 \
    CARGO_HOME=/work/.cargo-home

# Unprivileged user; make the toolchain dirs writable so cargo can cache there.
RUN useradd -m -u 1000 dev \
    && mkdir -p /run/user/1000 \
    && chown dev:dev /run/user/1000 \
    && chmod 0700 /run/user/1000 \
    && chmod -R a+w /usr/local/cargo /usr/local/rustup

COPY docker/sway.conf /etc/cozy/sway.conf
COPY docker/verify.sh /usr/local/bin/verify.sh
RUN chmod +x /usr/local/bin/verify.sh

USER dev
WORKDIR /work
ENTRYPOINT ["/usr/local/bin/verify.sh"]
