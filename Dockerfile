FROM rustlang/rust:nightly-alpine3.16 as build

# create a new empty shell project
RUN cargo new --bin archivanima
WORKDIR /archivanima

# copy over your manifests
COPY ./Cargo.toml ./Cargo.toml
COPY ./Cargo.lock ./Cargo.lock

# this build step will cache your dependencies
RUN cargo fetch --target x86_64-unknown-linux-musl

# copy your source tree
COPY ./src ./src
COPY ./templates ./templates
COPY ./Docker.Rocket.toml ./Rocket.toml
COPY ./assets.json ./assets.json
COPY ./static ./static
COPY ./package.json ./package.json
COPY ./package-lock.json ./package-lock.json
COPY ./migrations ./migrations
COPY ./.sqlx ./.sqlx

# install packages
RUN apk add libressl-dev gcc npm libc-dev sassc
RUN npm install -g typescript

# build for release
RUN cargo install sqlx-cli
RUN cargo build --release --target x86_64-unknown-linux-musl

# prepare assets
RUN mkdir -p ./internal/static
RUN mkdir -p ./internal/internal
RUN npm install
RUN RUST_LOG=debug ./target/x86_64-unknown-linux-musl/release/archivanima_bin pack

# our final base
FROM alpine:3

# copy the build artifact from the build stage
WORKDIR /app
COPY --from=build ./archivanima/target/x86_64-unknown-linux-musl/release/archivanima_bin ./archivanima_bin
COPY --from=build ./archivanima/internal/static ./internal/static
COPY --from=build ./archivanima/internal/assets_cache.json ./internal/assets_cache.json
COPY --from=build ./archivanima/migrations ./migrations
COPY --from=build ./archivanima/Rocket.toml ./Rocket.toml
COPY --from=build /usr/local/cargo/bin/sqlx ./sqlx
RUN mkdir ./data
RUN mkdir ./datapublic

# set the startup command to run your binary
EXPOSE 8001/tcp
CMD ["/app/archivanima_bin", "run"]
