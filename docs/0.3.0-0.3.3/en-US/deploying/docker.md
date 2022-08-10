# Docker Deployment

For situations where [serverful deployment](:deploying/serverful) is required, or in case there is a need to deploy one of the examples found on GitHub without prior setup of all necessary dependencies, below are `Dockerfile` examples meant to serve for different deployment scenarios. These steps can also serve as guidelines for production deployments.

Note that the following examples should be modified for your particular use-case rather than being used as-is. Also, these `Dockerfile`s are standalone because they use `curl` to download examples directly from the Perseus repository (of course, you'll probably want to use your own code in production).

Before proceeding with this section, you should be familiar with Docker's [multi-stage builds system](https://docs.docker.com/develop/develop-images/multistage-build) and Perseus' [code size optimizations](:deploying/size).

<details>
<summary>Production example using the size optimizations plugin</summary>

```dockerfile
# Pull base image.
FROM rust:1.57-slim AS base

# Define optional command-line build arguments we can pass to docker.
ARG EXAMPLE_NAME \
  PERSEUS_VERSION \
  BINARYEN_VERSION \
  BONNIE_VERSION \
  ESBUILD_VERSION \
  ESBUILD_TARGET \
  WASM_PACK_VERSION \
  WASM_TARGET \
  CARGO_NET_GIT_FETCH_WITH_CLI

# Export environment variables.
ENV EXAMPLE_NAME=${EXAMPLE_NAME:-showcase} \
  PERSEUS_VERSION=${PERSEUS_VERSION:-0.3.0} \
  PERSEUS_SIZE_OPT_VERSION=0.1.7 \
  SYCAMORE_VERSION=0.7.1 \
  BINARYEN_VERSION=${BINARYEN_VERSION:-104} \
  BONNIE_VERSION=${BONNIE_VERSION:-0.3.2} \
  ESBUILD_VERSION=${ESBUILD_VERSION:-0.14.7} \
  ESBUILD_TARGET=${ESBUILD_TARGET:-es6} \
  WASM_PACK_VERSION=${WASM_PACK_VERSION:-0.10.3} \
  WASM_TARGET=${WASM_TARGET:-wasm32-unknown-unknown} \
  CARGO_NET_GIT_FETCH_WITH_CLI=${CARGO_NET_GIT_FETCH_WITH_CLI:-false}

# Single-threaded perseus CLI mode required for low memory environments.
# ENV PERSEUS_CLI_SEQUENTIAL=true

# Work from the root of the container.
WORKDIR /

# Install build dependencies.
RUN apt-get update \
  && apt-get -y install --no-install-recommends \
  apt-transport-https \
  build-essential \
  curl \
  gawk \
  git \
  libssl-dev \
  lsb-release \
  openssl \
  pkg-config \
  shellcheck \
  && rustup target add ${WASM_TARGET}

# Create a build stage for `binaryen` we can run in parallel.
FROM base as binaryen

# Work from the chosen install path for `binaryen`.
WORKDIR /binaryen

# Download, extract, and remove compressed tar of `binaryen`.
RUN curl --progress-bar -Lo binaryen-${BINARYEN_VERSION}.tar.gz \
  https://github.com/WebAssembly/binaryen/releases/download/version_${BINARYEN_VERSION}/binaryen-version_${BINARYEN_VERSION}-x86_64-linux.tar.gz \
  && tar --strip-components=1 -xzf binaryen-${BINARYEN_VERSION}.tar.gz \
  && rm -f binaryen-${BINARYEN_VERSION}.tar.gz

# Create a build stage for `bonnie` we can run in parallel.
FROM base AS bonnie

# Work from the chosen install path for `bonnie`.
WORKDIR /bonnie

# Install crate `bonnie` into the work path.
RUN cargo install bonnie --version $BONNIE_VERSION \
  && mv /usr/local/cargo/bin/bonnie .

# Create a build stage for `esbuild` we can run in parallel.
FROM base as esbuild

# Work from the chosen install path for `esbuild`.
WORKDIR /esbuild

# Download, extract, and remove compressed tar of `esbuild`.
RUN curl --progress-bar -Lo esbuild-${ESBUILD_VERSION}.tar.gz \
  https://registry.npmjs.org/esbuild-linux-64/-/esbuild-linux-64-${ESBUILD_VERSION}.tgz \
  && tar --strip-components=1 -xzf esbuild-${ESBUILD_VERSION}.tar.gz \
  && rm -f esbuild-${ESBUILD_VERSION}.tar.gz

# Create a build stage for `wasm-pack` that we can run in parallel.
FROM base as wasm-pack

# Work from the chosen install path for `wasm-pack`.
WORKDIR /wasm-pack

# Install crate `wasm-pack` into the work path.
RUN cargo install wasm-pack --version $WASM_PACK_VERSION \
  && mv /usr/local/cargo/bin/wasm-pack .

# Create a build stage for the codebase of the `perseus` framework that we can run in parallel.
FROM base as framework

# Work from the root of the codebase.
WORKDIR /perseus

# Download and make modifications to the codebase of the perseus framework.
RUN . /etc/profile \
  && . /usr/local/cargo/env \
  && SRC_URL="https://codeload.github.com/arctic-hen7/" \
  && SRC_ROUTE="/tar.gz/v" \
  && curl --progress-bar \
  -L "${SRC_URL}perseus${SRC_ROUTE}${PERSEUS_VERSION}" \
  | tar -xz --strip-components=1 \
  && mkdir -p "$(pwd)/packages/perseus-size-opt" \
  && curl --progress-bar \
  -L "${SRC_URL}perseus-size-opt${SRC_ROUTE}${PERSEUS_SIZE_OPT_VERSION}" \
  | tar -C "$(pwd)/packages/perseus-size-opt" -xz --strip-components=1 \
  && rm -rf "$(pwd)/packages/perseus-size-opt/examples" \
  && PERSEUS_SIZE_OPT_TOML="$(pwd)/packages/perseus-size-opt/Cargo.toml" \
  && GREP_A=$( \
      grep -ne "^\[workspace\]$" "${PERSEUS_SIZE_OPT_TOML}" \
      | grep -Eo "^[^:]+" \
    ) \
  && GREP_B=$( \
      grep -ne "^\]$" "${PERSEUS_SIZE_OPT_TOML}" \
      | grep -Eo "^[^:]+" \
    ) \
  && sed -i "${GREP_A},${GREP_B}d" "${PERSEUS_SIZE_OPT_TOML}" \
  && unset PERSEUS_SIZE_OPT_TOML && unset GREP_A && unset GREP_B \
  && sed -i "\
  s|HOST|PERSEUS_HOST|g; \
  s|PORT|PERSEUS_PORT|g;" "$(pwd)/packages/perseus-cli/src/parse.rs" \
  && sed -i "\
  s|HOST|PERSEUS_HOST|g; \
  s|PORT|PERSEUS_PORT|g;" "$(pwd)/packages/perseus-cli/src/serve.rs" \
  && sed -i "\
  s|HOST|PERSEUS_HOST|g; \
  s|PORT|PERSEUS_PORT|g;" "$(pwd)/examples/basic/.perseus/server/src/main.rs" \
  && mkdir -p /tmp/cargo_toml \
  && touch /tmp/cargo_toml/list \
  && find "$(pwd)" -maxdepth 4 -type d -print0 \
  | xargs -0 sh -c \
  'for d; do if [ -f "${d}/Cargo.toml" ]; then echo "${d}/Cargo.toml" >> /tmp/cargo_toml/list; fi; done;' \
  && RX_O="[{\\\"]\\{1\\}" \
  && RX_P="\\(perseus[a-z-]\\{0,\\}\\)" \
  && RX_OPT="\\(perseus-size-opt\\)" \
  && RX_S="\\(sycamore[a-z-]\\{0,\\}\\)" \
  && RX_V="\\([\^0-9\.a-t-]\\{3,\\}\\)\\{0,1\\}" \
  && RX_SP="\\( \\)\\{0,1\\}" \
  && RX_PATH="\\(path = \\\"[^ ]\\{1,\\}\\\"[, ]\\{0,2\\}\\)\\{0,1\\}" \
  && RX_VERS="\\(version = \\\"[\^0-9\.a-t-]\\{3,\\}\\\"[, ]\\{0,2\\}\\)\\{0,1\\}" \
  && RX_FEAT="\\(features = \\[.*\\][, ]\\{0,2\\}\\)\\{0,1\\}" \
  && RX_OPTN="\\(optional = true\\)\\{0,1\\}" \
  && RX_C="[\\\"}]\\{1\\}" \
  && RX_SIZE_OPT="${RX_OPT} = ${RX_O}${RX_V}${RX_SP}${RX_PATH}${RX_VERS}${RX_FEAT}${RX_SP}${RX_C}" \
  && RX_PERSEUS="${RX_P} = ${RX_O}${RX_V}${RX_SP}${RX_PATH}${RX_VERS}${RX_FEAT}${RX_OPTN}${RX_SP}${RX_C}" \
  && RX_SYCAMORE="${RX_S} = ${RX_O}${RX_V}${RX_SP}${RX_VERS}${RX_FEAT}${RX_SP}${RX_C}" \
  && while IFS= read -r TOML; do \
    sed -i "\
    s|perseus-engine|perseus_engine|g; \
    s|^$( echo ${RX_SIZE_OPT} | tr -d '\n' )$|perseus_size_opt = { path = \"/perseus/packages/perseus_size_opt\", version = \"=${PERSEUS_SIZE_OPT_VERSION}\", \6 }|g; \
    s|^$( echo ${RX_PERSEUS} | tr -d '\n' )$|\1 = { path = \"/perseus/packages/\1\", version = \"=${PERSEUS_VERSION}\", \6, \7 }|g; \
    s|^$( echo ${RX_SYCAMORE} | tr -d '\n' )$|\1 = { version = \"=${SYCAMORE_VERSION}\", \5 }|g; \
    s|perseus_engine|perseus-engine|g; \
    s|perseus_size_opt|perseus-size-opt|g; \
    s|[,]\{1\}[ ]\{1\}[,]\{1\}[ ]\{1,\}|, |g; \
    s|[,]\{1\}[ ]\{1,\}}| }|g;" "${TOML}"; \
  done < /tmp/cargo_toml/list \
  && unset RX_O && unset RX_P && unset RX_OPT && unset RX_S && unset RX_V && unset RX_SP \
  && unset RX_PATH && unset RX_VERS && unset RX_FEAT && unset RX_OPTN && unset RX_C \
  && unset RX_SIZE_OPT && unset RX_PERSEUS && unset RX_SYCAMORE \
  && rm -rf /tmp/cargo_toml \
  && sed -i "\
  s|^\(use std::process.*\)$|\1\nuse std::time::Duration;|; \
  s|^\(.*spinner\.enable_steady_tick(\)[0-9]\{1,2\}\();\)$|\1Duration::from_millis(50)\2|;" \
  "$(pwd)/packages/perseus-cli/src/cmd.rs" \
  && sed -i "\
  s|\(cargo build\)|\1 --bin perseus --release|; \
  s|\`|'|g;" "$(pwd)/bonnie.toml"

# Create a build stage for `perseus-cli` that we can run in parallel.
FROM framework as perseus-cli

# Copy bonnie to satisfy implementation.
COPY --from=bonnie /bonnie/bonnie /usr/bin/

# Work from the root of the codebase.
WORKDIR /perseus

# Compile the release binary target of package `perseus-cli`.
RUN bonnie setup

# Create a build stage for building our app.
FROM framework as builder

# Copy the tools we previously prepared in parallel.
COPY --from=binaryen /binaryen/bin/ /usr/bin/
COPY --from=binaryen /binaryen/include/ /usr/include/
COPY --from=binaryen /binaryen/lib/ /usr/lib/
COPY --from=esbuild /esbuild/bin/esbuild /usr/bin/
COPY --from=perseus-cli /perseus/target/release/perseus /usr/bin/
COPY --from=wasm-pack /wasm-pack/wasm-pack /usr/bin/

# Work from the root of our app.
WORKDIR /perseus/examples/showcase

# Patch in implementation of perseus-size-opt prior to deploying our app.
RUN . /etc/profile && . /usr/local/cargo/env \
  && $( \
    LIB_RS="$(pwd)/src/lib.rs"; \
    PAD="\ \ \ \ "; \
    USE_LINE=$( grep -ne "^use perseus.*$" "${LIB_RS}" | grep -Eo "^[^:]+" ); \
    HAS_PLUGINS=$( grep -ne "^use.*Plugins[,]\{0,\}.*$" "${LIB_RS}" ); \
    if [ -z "${HAS_PLUGINS}" ]; then \
      sed -i "${USE_LINE}s|^\(use perseus::\)\(.*\)\(;\)|\1{\2, Plugins}\3|" \
      "${LIB_RS}"; \
      END_LINE=$( grep -ne "^}$" "${LIB_RS}" | grep -Eo "^[^:]+" ); \
      sed -i "$(( END_LINE - 1 ))s|^\(.*\)$|\1,|" "${LIB_RS}"; \
      sed -i "${END_LINE}i \
      plugins: Plugins::new()\n\
      ${PAD}.plugin(\n\
      ${PAD}${PAD}perseus_size_opt,\n\
      ${PAD}${PAD}SizeOpts {\n\
      ${PAD}${PAD}${PAD}codegen_units: 1,\n\
      ${PAD}${PAD}${PAD}enable_fluent_bundle_patch: false,\n\
      ${PAD}${PAD}${PAD}lto: true,\n\
      ${PAD}${PAD}${PAD}opt_level: \"s\".to_string(),\n\
      ${PAD}${PAD}${PAD}wee_alloc: true,\n\
      ${PAD}${PAD}}\n\
      ${PAD})" "${LIB_RS}"; \
    fi; \
    sed -i "${USE_LINE}a use perseus_size_opt::{perseus_size_opt, SizeOpts};" \
    "${LIB_RS}"; \
    unset LIB_RS && unset PAD && unset USE_LINE && unset HAS_PLUGINS && unset END_LINE; \
  ) \
  && sed -i "\
  s|^\(\[dependencies\]\)$|\1\nperseus-size-opt = { path = \"/perseus/packages/perseus-size-opt\" }|" \
  "$(pwd)/Cargo.toml" \
  && perseus clean \
  && perseus prep \
  && perseus tinker \
  && $( \
    FILE_PATH="$(pwd)/.perseus/src/lib.rs"; \
    LINE_NUM=$( grep -ne "clippy" "${FILE_PATH}" | grep -Eo "^[^:]+" ); \
    if [ -n "${LINE_NUM}" ] && [ $LINE_NUM -ne 1 ]; then \
      awk -i inplace \
      -v line_num="${LINE_NUM}" \
      -v inner_attr=$( sed "${LINE_NUM}q;d" "${FILE_PATH}" | cut -d " " -f 1 ) \
      "NR==1 { print inner_attr } NR!=line_num { print }" "${FILE_PATH}"; \
    fi; \
    unset FILE_PATH && unset LINE_NUM; \
  ) \
  && PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig \
  && perseus deploy \
  && esbuild ./pkg/dist/pkg/perseus_engine.js \
  --minify \
  --target=${ESBUILD_TARGET} \
  --outfile=./pkg/dist/pkg/perseus_engine.js \
  --allow-overwrite \
  && wasm-opt \
  -Os ./pkg/dist/pkg/perseus_engine_bg.wasm \
  -o ./pkg/dist/pkg/perseus_engine_bg.wasm

# Prepare the final image where the app will be deployed.
FROM debian:stable-slim

# Work from a chosen install path for the deployed app.
WORKDIR /app

# Copy the app into its chosen install path.
COPY --from=builder /perseus/examples/showcase/pkg /app/

# Bind the server to `localhost`.
ENV PERSEUS_HOST=0.0.0.0

# Bind the container to the default port of 8080.
ENV PERSEUS_PORT=8080

# Configure the container to automatically serve the deployed app while running.
CMD ["./server"]
```

</details>

<details>
<summary>Production examples using `wee_alloc` manually</summary>

```dockerfile
# Pull the base image.
FROM rust:1.57-slim AS build

# Install build dependencies.
RUN apt update \
  && apt -y install --no-install-recommends \
  apt-transport-https \
  build-essential \
  curl \
  libssl-dev \
  lsb-release \
  openssl \
  pkg-config

# Export environment variables.
ENV PERSEUS_VERSION=0.3.3 \
  SYCAMORE_VERSION=0.7.1 \
  WEE_ALLOC_VERSION=0.4.5 \
  BINARYEN_VERSION=104 \
  ESBUILD_VERSION=0.14.7 \
  WASM_PACK_VERSION=0.10.3 \
  RUST_RELEASE_CHANNEL=stable

# Work from the root of the project.
WORKDIR /app

# Perform the following steps:
# - Install latest `rust` from `stable` release channel.
# - Set `rust:stable` as default toolchain.
# - Download the target for `wasm`.
RUN rustup install $RUST_RELEASE_CHANNEL \
  && rustup default $RUST_RELEASE_CHANNEL \
  && target add wasm32-unknown-unknown

# Install crate `perseus-cli`
RUN cargo install perseus-cli --version $PERSEUS_VERSION

# Install crate `wasm-pack`.
RUN cargo install wasm-pack --version $WASM_PACK_VERSION

# Retrieve the src of the project and remove unnecessary boilerplate.
RUN curl -L# \
  https://codeload.github.com/arctic-hen7/perseus/tar.gz/v${PERSEUS_VERSION} \
  | tar -xz --strip=3 perseus-${PERSEUS_VERSION}/examples/comprehensive/tiny

# Download, unpack, symlink, and verify install of `binaryen`.
RUN curl -L#o binaryen-${BINARYEN_VERSION}.tar.gz \
  https://github.com/WebAssembly/binaryen/releases/download/version_${BINARYEN_VERSION}/binaryen-version_${BINARYEN_VERSION}-x86_64-linux.tar.gz \
  && tar -xzf binaryen-${BINARYEN_VERSION}.tar.gz \
  && ln -s $(pwd)/binaryen-version_${BINARYEN_VERSION}/bin/wasm-opt /usr/bin/wasm-opt \
  && wasm-opt --version

# Download, unpack, symlink, and verify install of `esbuild`.
RUN curl -L#o esbuild-${ESBUILD_VERSION}.tar.gz \
  https://registry.npmjs.org/esbuild-linux-64/-/esbuild-linux-64-${ESBUILD_VERSION}.tgz \
  && tar -xzf esbuild-${ESBUILD_VERSION}.tar.gz \
  && ln -s $(pwd)/package/bin/esbuild /usr/bin/esbuild \
  && esbuild --version

# Work from the src of the project.
WORKDIR /app/tiny

# Specify precise dependency versions in the project's `Cargo.toml` file.
RUN sed -i "\
  s|^\(perseus =\).*$|\1 \"=${PERSEUS_VERSION}\"|; \
  s|^\(sycamore =\).*$|\1 \"=${SYCAMORE_VERSION}\"|;
  s|^\(\[dependencies\]\)$|\1\nwee_alloc = \"=${WEE_ALLOC_VERSION}\"|;" \
  ./Cargo.toml && cat ./Cargo.toml

# Prepend modifications to the src of the project to implement `wee_alloc` in `lib.rs`.
RUN sed -i "1i \
  #[global_allocator]\n\
  static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;\n" \
  ./src/lib.rs && cat ./src/lib.rs

# Update dependencies to their precise, required versions.
RUN cargo update -p perseus --precise $PERSEUS_VERSION \
  && cargo update -p sycamore --precise $SYCAMORE_VERSION \
  && cargo update -p wee_alloc --precise $WEE_ALLOC_VERSION

# Clean any pre-existing generated `./perseus` subdirectory from the project,
# then prepare the project prior to ejecting it from the CLI.
RUN perseus clean \
  && perseus prep \
  && perseus eject

# Append necessary modifications to the `Cargo.toml` file in the prepared project.
RUN sed -i "s|^\(perseus =\).*$|\1 \"${PERSEUS_VERSION}\"|g" .perseus/Cargo.toml \
  && printf '%s\n' \
  "" "" \
  "[profile.release]" \
  "codegen-units = 1" \
  "opt-level = \"s\"" \
  "lto = true" >> .perseus/Cargo.toml \
  && cat .perseus/Cargo.toml

# Patch `clippy` inner attribute syntax error in `lib.rs` (if found).
RUN sed -i "s|\(#\)!\(\[allow(clippy::unused_unit)\]\)|\1\2|;" ./.perseus/src/lib.rs

# Single-threaded perseus CLI mode required for low memory environments.
# ENV PERSEUS_CLI_SEQUENTIAL=true

# Export variables required by `wasm-bindgen` and deploy the app from the project.
RUN export PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig \
  && perseus deploy

# Run `esbuild` against `bundle.js` to optimize it into minified format.
RUN esbuild ./pkg/dist/pkg/perseus_engine.js \
  --minify \
  --target=esnext \
  --outfile=./pkg/dist/pkg/perseus_engine.js \
  --allow-overwrite \
  && ls -lha ./pkg/dist/pkg

# Run `wasm-opt` against `bundle.wasm` to optimize it based on bytesize.
RUN wasm-opt \
  -Os ./pkg/dist/pkg/perseus_engine_bg.wasm \
  -o ./pkg/dist/pkg/perseus_engine_bg.wasm \
  && ls -lha ./pkg/dist/pkg

# Prepare the final image where the app will be deployed.
FROM debian:stable-slim

# Work from a chosen install path for the deployed app.
WORKDIR /app

# Copy the app into its chosen install path.
COPY --from=build /app/tiny/pkg /app/

# Bind the server to `localhost`.
ENV HOST=0.0.0.0

# Bind the container to the default port of 8080.
ENV PORT=8080

# Configure the container to automatically serve the deployed app while running.
CMD ["./server"]
```

</details>

<details>
<summary>Test example for deploying a specific branch from the Perseus repository</summary>

```dockerfile
# Pull base image.
FROM rust:1.57-slim AS build

# Install build dependencies.
RUN apt update \
  && apt -y install --no-install-recommends \
  apt-transport-https \
  build-essential \
  curl \
  libssl-dev \
  lsb-release \
  nodejs \
  npm \
  openssl \
  pkg-config

# Export environment variables.
ENV PERSEUS_BRANCH=main \
  EXAMPLE_CATEGORY=comprehensive \
  EXAMPLE_NAME=tiny \
  BONNIE_VERSION=0.3.2 \
  BINARYEN_VERSION=104 \
  ESBUILD_VERSION=0.14.7 \
  WASM_PACK_VERSION=0.10.3 \
  RUST_RELEASE_CHANNEL=stable

# Work from the root of the project.
WORKDIR /app

# Download the target for `wasm`.
RUN rustup install $RUST_RELEASE_CHANNEL \
  && rustup default $RUST_RELEASE_CHANNEL \
  && rustup target add wasm32-unknown-unknown

# Install crate `bonnie`.
RUN cargo install bonnie --version $BONNIE_VERSION

# Install crate `wasm-pack`.
RUN cargo install wasm-pack --version $WASM_PACK_VERSION

# Install dependencies required by package `perseus-website`.
RUN npm i -g browser-sync concurrently serve tailwindcss

# Download, unpack, symlink, and verify install of `binaryen`.
RUN curl -L#o binaryen-${BINARYEN_VERSION}.tar.gz \
  https://github.com/WebAssembly/binaryen/releases/download/version_${BINARYEN_VERSION}/binaryen-version_${BINARYEN_VERSION}-x86_64-linux.tar.gz \
  && tar -xzf binaryen-${BINARYEN_VERSION}.tar.gz \
  && ln -s $(pwd)/binaryen-version_${BINARYEN_VERSION}/bin/wasm-opt /usr/bin/wasm-opt \
  && wasm-opt --version

# Download, unpack, symlink, and verify install of `esbuild`.
RUN curl -L#o esbuild-${ESBUILD_VERSION}.tar.gz \
  https://registry.npmjs.org/esbuild-linux-64/-/esbuild-linux-64-${ESBUILD_VERSION}.tgz \
  && tar -xzf esbuild-${ESBUILD_VERSION}.tar.gz \
  && ln -s $(pwd)/package/bin/esbuild /usr/bin/esbuild \
  && esbuild --version

# Retrieve the current state of a branch in the `perseus` repo.
RUN curl -L# \
  https://codeload.github.com/arctic-hen7/perseus/tar.gz/${PERSEUS_BRANCH} \
  | tar -xz

# Work from the requested branch of `perseus`.
WORKDIR /app/perseus-${PERSEUS_BRANCH}

# Perform the following steps:
# - Patch `bonnie.toml` to remove backticks.
#   - These break echoed strings and cause builds to fail.
# - Instruct `cargo` to only compile the binary target `perseus`.
#   - Prevents "no space left on device" error in `docker`.
RUN sed -i "\
  s|\(cargo build\)|\1 --bin perseus|; \
  s|\`|'|g" ./bonnie.toml

# Compile and install `perseus-cli` as defined by the current state of the repo's branch.
RUN bonnie setup

# Clean any pre-existing generated `./perseus` subdirectory from the project.
RUN bonnie dev example $EXAMPLE_CATEGORY $EXAMPLE_NAME clean

# Prepare the project prior to deployment.
RUN bonnie dev example $EXAMPLE_CATEGORY $EXAMPLE_NAME prep

# Patch `clippy` inner attribute syntax error in `lib.rs` (if found).
RUN sed -i "s|\(#\)!\(\[allow(clippy::unused_unit)\]\)|\1\2|;" ./.perseus/src/lib.rs

# Single-threaded perseus CLI mode required for low memory environments.
# ENV PERSEUS_CLI_SEQUENTIAL=true

# Export variables required by `wasm-bindgen` and deploy the app from the project.
RUN export PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig \
  && bonnie dev example $EXAMPLE_CATEGORY $EXAMPLE_NAME deploy

# Work from the path containing the deployed app.
WORKDIR /app/perseus-${PERSEUS_BRANCH}/examples/${EXAMPLE_CATEGORY}/${EXAMPLE_NAME}

# Run `esbuild` against `bundle.js` to optimize it into minified format.
RUN esbuild ./pkg/dist/pkg/perseus_engine.js \
  --minify \
  --target=es6 \
  --outfile=./pkg/dist/pkg/perseus_engine.js \
  --allow-overwrite \
  && ls -lha ./pkg/dist/pkg

# Run `wasm-opt` against `bundle.wasm` to optimize it based on bytesize.
RUN wasm-opt \
  -Os ./pkg/dist/pkg/perseus_engine_bg.wasm \
  -o ./pkg/dist/pkg/perseus_engine_bg.wasm \
  && ls -lha ./pkg/dist/pkg

# Rename the dynamic path containing the deployed app to a static path.
RUN mv /app/perseus-${PERSEUS_BRANCH} /app/perseus-branch

# Prepare the final image where the app will be deployed.
FROM debian:stable-slim

# Work from a chosen install path for the deployed app.
WORKDIR /app

# Copy the deployed app into its chosen install path.
COPY --from=build /app/perseus-branch/examples/comprehensive/tiny/pkg /app/

# Bind the container to `localhost`.
ENV HOST=0.0.0.0

# Bind the container to the default port of `8080`.
ENV PORT=8080

# Configure the container to automatically serve the deployed app while running.
CMD ["./server"]
```

</details>
