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
ENV EXAMPLE_NAME="${EXAMPLE_NAME:-showcase}" \
  PERSEUS_VERSION="${PERSEUS_VERSION:-0.3.0}" \
  PERSEUS_SIZE_OPT_VERSION="0.1.7" \
  SYCAMORE_VERSION="0.7.1" \
  BINARYEN_VERSION="${BINARYEN_VERSION:-104}" \
  BONNIE_VERSION="${BONNIE_VERSION:-0.3.2}" \
  ESBUILD_VERSION="${ESBUILD_VERSION:-0.14.7}" \
  ESBUILD_TARGET="${ESBUILD_TARGET:-es6}" \
  WASM_PACK_VERSION="${WASM_PACK_VERSION:-0.10.3}" \
  WASM_TARGET="${WASM_TARGET:-wasm32-unknown-unknown}" \
  CARGO_NET_GIT_FETCH_WITH_CLI="${CARGO_NET_GIT_FETCH_WITH_CLI:-false}"

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
  npm \
  openssl \
  pkg-config \
  && rustup target add "${WASM_TARGET}"

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
  && $( \
    NO_FETCHING_VERSION="0.3.0"; \
    FETCHING_VERSION="0.3.1"; \
    LIB_RS="$(pwd)/packages/perseus/src/lib.rs"; \
    if [ "${PERSEUS_VERSION}" = "${NO_FETCHING_VERSION}" ]; then \
      sed -i "\
      s|^\(pub use http::Request as HttpRequest;\)$|\1\npub use wasm_bindgen_futures::spawn_local;|" \
      "${LIB_RS}"; \
      CMD_RS="$(pwd)/packages/perseus-cli/src/cmd.rs"; \
      sed -i "\
      s|^\(use std::process.*\)$|\1\nuse std::time::Duration;|; \
      s|^\(.*spinner\.enable_steady_tick(\)[0-9]\{1,2\}\();\)$|\1Duration::from_millis(50)\2|;" \
      "${CMD_RS}"; \
      mkdir -p "/perseus/examples/fetching"; \
      curl --progress-bar \
      -L "${SRC_URL}perseus${SRC_ROUTE}${FETCHING_VERSION}" \
      | tar -C "/perseus/examples/fetching" -xz --strip-components=3 \
      "perseus-${FETCHING_VERSION}/examples/fetching"; \
    fi; \
    unset NO_FETCHING_VERSION && unset FETCHING_VERSION; \
  ) \
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
  && $( \
    RS_FILES="$(pwd)/packages/perseus-cli/src/parse.rs"; \
    RS_FILES="${RS_FILES} $(pwd)/packages/perseus-cli/src/serve.rs"; \
    RS_FILES="${RS_FILES} $(pwd)/examples/basic/.perseus/server/src/main.rs"; \
    for RS in $RS_FILES; do \
      HAS_PERSEUS_HOST=$( grep -ne "PERSEUS_HOST" "${RS}" | grep -Eo "^[^:]+" ); \
      if [ -n "${HAS_PERSEUS_HOST}" ]; then \
        sed -i "s|HOST|PERSEUS_HOST|g;" "${RS}"; \
      fi; \
      HAS_PERSEUS_PORT=$( grep -ne "PERSEUS_PORT" "${RS}" | grep -Eo "^[^:]+" ); \
      if [ -n "${HAS_PERSEUS_PORT}" ]; then \
        sed -i "s|PORT|PERSEUS_PORT|g;" "${RS}"; \
      fi; \
    done; \
    unset RS_FILES && unset RS && unset HAS_PERSEUS_HOST && unset HAS_PERSEUS_PORT; \
  ) \
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
RUN . /etc/profile \
  && . /usr/local/cargo/env \
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
  && esbuild "$(pwd)/pkg/dist/pkg/perseus_engine.js" \
  --minify \
  --target="${ESBUILD_TARGET}" \
  --outfile="$(pwd)/pkg/dist/pkg/perseus_engine.js" \
  --allow-overwrite \
  && wasm-opt \
  -Os "$(pwd)/pkg/dist/pkg/perseus_engine_bg.wasm" \
  -o "$(pwd)/pkg/dist/pkg/perseus_engine_bg.wasm"

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
  CARGO_NET_FETCH_WITH_CLI

# Export environment variables.
ENV EXAMPLE_NAME="${EXAMPLE_NAME:-fetching}" \
  PERSEUS_VERSION="${PERSEUS_VERSION:-0.3.0}" \
  PERSEUS_SIZE_OPT_VERSION="0.1.7" \
  WEE_ALLOC_VERSION="0.4.5" \
  SYCAMORE_VERSION="0.7.1" \
  BINARYEN_VERSION="${BINARYEN_VERSION:-104}" \
  BONNIE_VERSION="${BONNIE_VERSION:-0.3.2}" \
  ESBUILD_VERSION="${ESBUILD_VERSION:-0.14.7}" \
  ESBUILD_TARGET="${ESBUILD_TARGET:-es6}" \
  WASM_PACK_VERSION="${WASM_PACK_VERSION:-0.10.3}" \
  WASM_TARGET="${WASM_TARGET:-wasm32-unknown-unknown}" \
  CARGO_NET_GIT_FETCH_WITH_CLI="${CARGO_NET_GIT_FETCH_WITH_CLI:-false}"

# Single-threaded perseus CLI mode required for low memory environments.
# ENV PERSEUS_CLI_SEQUENTIAL=true

# Work from the root of the container.
WORKDIR /

# Install build dependencies.
RUN apt update \
  && apt -y install --no-install-recommends \
  apt-transport-https \
  build-essential \
  curl \
  gawk \
  git \
  libssl-dev \
  lsb-release \
  npm \
  openssl \
  pkg-config \
  && rustup target add "${WASM_TARGET}"

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
  && $( \
    NO_FETCHING_VERSION="0.3.0"; \
    FETCHING_VERSION="0.3.1"; \
    LIB_RS="$(pwd)/packages/perseus/src/lib.rs"; \
    if [ "${PERSEUS_VERSION}" = "${NO_FETCHING_VERSION}" ]; then \
      sed -i "\
      s|^\(pub use http::Request as HttpRequest;\)$|\1\npub use wasm_bindgen_futures::spawn_local;|" \
      "${LIB_RS}"; \
      CMD_RS="$(pwd)/packages/perseus-cli/src/cmd.rs"; \
      sed -i "\
      s|^\(use std::process.*\)$|\1\nuse std::time::Duration;|; \
      s|^\(.*spinner\.enable_steady_tick(\)[0-9]\{1,2\}\();\)$|\1Duration::from_millis(50)\2|;" \
      "${CMD_RS}"; \
      mkdir -p "/perseus/examples/fetching"; \
      curl --progress-bar \
      -L "${SRC_URL}perseus${SRC_ROUTE}${FETCHING_VERSION}" \
      | tar -C "/perseus/examples/fetching" -xz --strip-components=3 \
      "perseus-${FETCHING_VERSION}/examples/fetching"; \
      unset CMD_RS; \
    fi; \
    unset NO_FETCHING_VERSION && unset FETCHING_VERSION && unset LIB_RS; \
  ) \
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
  && $( \
    RS_FILES="$(pwd)/packages/perseus-cli/src/parse.rs"; \
    RS_FILES="${RS_FILES} $(pwd)/packages/perseus-cli/src/serve.rs"; \
    RS_FILES="${RS_FILES} $(pwd)/examples/basic/.perseus/server/src/main.rs"; \
    for RS in $RS_FILES; do \
      HAS_PERSEUS_HOST=$( grep -ne "PERSEUS_HOST" "${RS}" | grep -Eo "^[^:]+" ); \
      if [ -n "${HAS_PERSEUS_HOST}" ]; then \
        sed -i "s|HOST|PERSEUS_HOST|g;" "${RS}"; \
      fi; \
      HAS_PERSEUS_PORT=$( grep -ne "PERSEUS_PORT" "${RS}" | grep -Eo "^[^:]+" ); \
      if [ -n "${HAS_PERSEUS_PORT}" ]; then \
        sed -i "s|PORT|PERSEUS_PORT|g;" "${RS}"; \
      fi; \
    done; \
    unset RS_FILES && unset RS && unset HAS_PERSEUS_HOST && unset HAS_PERSEUS_PORT; \
  ) \
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
WORKDIR /perseus/examples/fetching

# Patch in implementation of wee_alloc prior to deploying our app.
RUN . /etc/profile \
  && . /usr/local/cargo/env \
  && curl --progress-bar \
  -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.1/install.sh \
  | bash \
  && . "${HOME}/.bashrc" \
  && nvm install node \
  && npm update -g \
  && . /etc/profile \
  && . "${HOME}/.bashrc" \
  && . /usr/local/cargo/env \
  && npm i -g browser-sync concurrently serve tailwindcss \
  && $( \
    LIB_RS="$(pwd)/src/lib.rs"; \
    sed -i "1i \
    #[global_allocator]\n\
    static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;\n" \
    "${LIB_RS}"; \
    unset LIB_RS; \
  ) \
  && sed -i "\
  s|^\(\[dependencies\]\)$|\1\nwee_alloc = \"=${WEE_ALLOC_VERSION}\"|" \
  "$(pwd)/Cargo.toml" \
  && perseus clean \
  && perseus prep \
  && perseus eject \
  && printf '%s\n' \
  "" "" \
  "[profile.release]" \
  "codegen-units = 1" \
  "opt-level = \"s\"" \
  "lto = true" >> "$(pwd)/.perseus/Cargo.toml" \
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
  && esbuild "$(pwd)/pkg/dist/pkg/perseus_engine.js" \
  --minify \
  --target="${ESBUILD_TARGET}" \
  --outfile="$(pwd)/pkg/dist/pkg/perseus_engine.js" \
  --allow-overwrite \
  && wasm-opt \
  -Os "$(pwd)/pkg/dist/pkg/perseus_engine_bg.wasm" \
  -o "$(pwd)/pkg/dist/pkg/perseus_engine_bg.wasm" \
  && $( \
    if [ ! -f "$(pwd)/pkg/message.txt" ]; then \
      cp "$(pwd)/message.txt" "$(pwd)/pkg/message.txt"; \
    fi; \
  )

# Prepare the final image where the app will be deployed.
FROM debian:stable-slim

# Work from a chosen install path for the deployed app.
WORKDIR /app

# Copy the app into its chosen install path.
COPY --from=builder /perseus/examples/fetching/pkg /app/

# Bind the server to `localhost`.
ENV PERSEUS_HOST=0.0.0.0

# Bind the container to the default port of 8080.
ENV PERSEUS_PORT=8080

# Configure the container to automatically serve the deployed app while running.
CMD ["./server"]
```

</details>
