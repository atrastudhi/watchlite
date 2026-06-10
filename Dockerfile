# Builds a ~1 MB image containing only the static binary.
#
# For real host metrics the container must see the host's namespaces:
#   docker run -d --name watchlite \
#     --pid=host --net=host \
#     -v /var/run/docker.sock:/var/run/docker.sock:ro \
#     ghcr.io/atrastudhi/watchlite --bind 0.0.0.0:8077
#
# Without --pid/--net=host it reports the container's own view (fine for a demo).

FROM rust:alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM scratch
COPY --from=build /src/target/release/watchlite /watchlite
EXPOSE 8077
ENTRYPOINT ["/watchlite"]
CMD ["--bind", "0.0.0.0:8077"]
