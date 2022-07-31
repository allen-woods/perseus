# Docker Deployment

For situations where [serverful deployment](:reference/deploying/serverful) is required, or in case there is a need to deploy one of the examples found on GitHub without prior setup of all necessary dependencies, below are `Dockerfile` examples meant to serve for different deployment scenarios. These steps can also serve as guidelines for production deployments.

Note that the following examples should be modified for your particular use-case rather than being used as-is. Also, these `Dockerfile`s are standalone because they use `curl` to download examples directly from the Perseus repository (of course, you'll probably want to use your own code in production).

Before proceeding with this section, you should be familiar with Docker's [multi-stage builds system](https://docs.docker.com/develop/develop-images/multistage-build) and Perseus' [code size optimizations](:reference/deploying/size).

<details>
<summary>Production example using the size optimizations plugin</summary>

## Building and Running

Save the `Dockerfile` below to a directory of your choice.

To build a container using the `Dockerfile`, navigate to the directory where the `Dockerfile` was saved and use a `docker` command similar to the following:

```shellscript
docker buildx build --no-cache \
--build-arg PERSEUS_VERSION=0.3.5 \
--build-arg PERSEUS_SIZE_OPT_VERSION=0.1.9 \
--tag="my-perseus-example:0.1.0" \
-f Dockerfile .
```

To run your newly built container, use the following command:

```shellscript
docker run --init -p 80:8080 my-perseus-example:0.1.0
```

Point your browser to `localhost` to interact with the app! To stop the server, press `Ctrl+C` in your terminal.

---

## Dockerfile

```dockerfile
# Pull base image.
FROM rust:1.57-slim AS base

# Define optional command-line build arguments we can pass to docker.
ARG EXAMPLE_CATEGORY \
  EXAMPLE_NAME \
  PERSEUS_VERSION \
  PERSEUS_SIZE_OPT_VERSION \
  SYCAMORE_VERSION \
  BINARYEN_VERSION \
  ESBUILD_VERSION \
  ESBUILD_TARGET \
  WASM_PACK_VERSION \
  WASM_TARGET \
  RUST_RELEASE_CHANNEL \
  CARGO_NET_GIT_FETCH_WITH_CLI

# Export environment variables.
ENV EXAMPLE_CATEGORY=${EXAMPLE_CATEGORY:-core} \
  EXAMPLE_NAME=${EXAMPLE_NAME:-state_generation} \
  PERSEUS_VERSION=${PERSEUS_VERSION:-0.3.4} \
  PERSEUS_SIZE_OPT_VERSION=${PERSEUS_SIZE_OPT_VERSION:-0.1.8} \
  SYCAMORE_VERSION=${SYCAMORE_VERSION:-0.7.1} \
  BINARYEN_VERSION=${BINARYEN_VERSION:-104} \
  ESBUILD_VERSION=${ESBUILD_VERSION:-0.14.7} \
  ESBUILD_TARGET=${ESBUILD_TARGET:-es6} \
  WASM_PACK_VERSION=${WASM_PACK_VERSION:-0.10.3} \
  WASM_TARGET=${WASM_TARGET:-wasm32-unknown-unknown} \
  RUST_RELEASE_CHANNEL=${RUST_RELEASE_CHANNEL:-stable} \
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
  && rustup install ${RUST_RELEASE_CHANNEL} \
  && rustup default ${RUST_RELEASE_CHANNEL} \
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

# Create a build stage for `esbuild` we can run in parallel.
FROM base as esbuild

# Work from the chosen install path for `esbuild`.
WORKDIR /esbuild

# Download, extract, and remove compressed tar of `esbuild`.
RUN curl --progress-bar -Lo esbuild-${ESBUILD_VERSION}.tar.gz \
  https://registry.npmjs.org/esbuild-linux-64/-/esbuild-linux-64-${ESBUILD_VERSION}.tgz \
  && tar --strip-components=1 -xzf esbuild-${ESBUILD_VERSION}.tar.gz \
  && rm -f esbuild-${ESBUILD_VERSION}.tar.gz

# Create a build stage for `perseus-size-opt` that we can run in parallel.
FROM base as perseus-size-opt

# Work from the chosen install path for `perseus-size-opt`.
WORKDIR /perseus-size-opt

# Download and make adjustments to the source of `perseus-size-opt`.
RUN curl --progress-bar -L https://codeload.github.com/arctic-hen7/perseus-size-opt/tar.gz/v${PERSEUS_SIZE_OPT_VERSION} \
  | tar -xz --strip-components=1 \
  && rm -rf ./examples \
  && sed -i "s|^\(perseus =\).*$|\1 { path = \\\"/perseus/packages/perseus\\\" }|;" ./Cargo.toml \
  && printf '%s\n' \
  '#!/bin/sh' \
  'rm_workspace () {' \
  '  local a=$( grep -ne "^\[workspace\]$" ./Cargo.toml | grep -Eo "^[^:]+" )' \
  '  local b=$( grep -ne "^\]$" ./Cargo.toml | grep -Eo "^[^:]+" )' \
  '  sed -i "${a},${b}d" ./Cargo.toml' \
  '}' \
  'rm_workspace' > ./rm_workspace.sh \
  && chmod +x ./rm_workspace.sh && . ./rm_workspace.sh && rm -f ./rm_workspace.sh

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

# COPY ./sh/patch_lib_rs.sh .

# Download and make adjustments to the codebase of the framework.
RUN curl --progress-bar -L https://codeload.github.com/arctic-hen7/perseus/tar.gz/v${PERSEUS_VERSION} \
  | tar -xz --strip-components=1 \
  && sed -i "\
  s|\(println!.*\)$|// \1|; \
  s|\.\./\.\.examples/core/basic/\.perseus|/perseus/examples/core/basic/\.perseus|g; \
  s|\(fs::remove_dir_all(dest\.join(\"\.perseus/dist\"))\.unwrap();\)|// \1|; \
  s|PERSEUS_VERSION|path = \\\\\"/perseus/packages/perseus\\\\\"|g; \
  s|PERSEUS_ACTIX_WEB_VERSION|path = \\\\\"/perseus/packages/perseus-actix-web\\\\\"|g; \
  s|PERSEUS_WARP_VERSION|path = \\\\\"/perseus/packages/perseus-warp\\\\\"|g;" \
  ./packages/perseus-cli/build.rs \
  && echo "* * SED 1 * *" \
  && cat -n ./packages/perseus-cli/build.rs \
  && sed -i "\
  s|^\(perseus = \"\)\([\^0-9\.beta-]\{3,\}\)\(.*\)$|\1=${PERSEUS_VERSION}\3|g; \
  s|^\(perseus = { version = \"\)\([\^0-9\.beta-]\{3,\}\)\(.*\)$|\1=${PERSEUS_VERSION}\3|g; \
  s|^\(sycamore = \"\)\([\^0-9\.beta-]\{3,\}\)\(.*\)$|\1=${SYCAMORE_VERSION}\3|g; \
  s|^\(sycamore = { version = \"\)\([\^0-9\.beta-]\{3,\}\)\(.*\)$|\1=${SYCAMORE_VERSION}\3|g;" \
  ./examples/${EXAMPLE_CATEGORY}/${EXAMPLE_NAME}/Cargo.toml \
  && echo "* * SED 2 * *" \
  && cat -n ./examples/${EXAMPLE_CATEGORY}/${EXAMPLE_NAME}/Cargo.toml \
  && sed -i "s|^\(\[dependencies\]\)$|\1\nperseus-size-opt = { path = \"/perseus-size-opt\" }|" \
  ./examples/${EXAMPLE_CATEGORY}/${EXAMPLE_NAME}/Cargo.toml \
  && echo "* * SED 3 * *" \
  && cat -n ./examples/${EXAMPLE_CATEGORY}/${EXAMPLE_NAME}/Cargo.toml \
  && printf '%s\n' \
  '#!/bin/sh' \
  '' \
  'patch_lib_rs () {' \
  '   local file_path' \
  '   file_path=./examples/${EXAMPLE_CATEGORY}/${EXAMPLE_NAME}/src/lib.rs' \
  '   local pad' \
  '   pad="\ \ \ \ "' \
  '   local mod_line' \
  '   mod_line=$( grep -ne "^mod error_pages;$" "${file_path}" | grep -Eo "^[^:]+" )' \
  '   local use_line' \
  '   use_line=$( grep -ne "^use perseus.*$" "${file_path}" | grep -Eo "^[^:]+" )' \
  '   local err_line' \
  '   err_line=$( grep -ne "^.*\.error_pages(.*)$" "${file_path}" | grep -Eo "^[^:]+" )' \
  '   local end_line' \
  '   local has_errors' \
  '   has_errors=$( grep -ne "^use.*ErrorPages[,]\{0,\}.*;$" "${file_path}" )' \
  '   local has_plugins' \
  '   has_plugins=$( grep -ne "^use.*Plugins[,]\{0,\}.*;$" "${file_path}" )' \
  '   local has_sycamore' \
  '   has_sycamore=$( grep -ne "^use sycamore.*$" "${file_path}" )' \
  '' \
  '   if [ -z "${has_errors}" ] && [ -n "${err_line}" ]' \
  '   then' \
  '     sed -i "${use_line}s|^\(use perseus::{\)\(.*\)\(};\)|\1ErrorPages, \2\3|" "${file_path}"' \
  '     sed -i "${err_line}d" "${file_path}"' \
  '     sed -i "$(( $err_line - 1 ))a \\ ' \
  '     ${pad}.error_pages\(|| ErrorPages::new\(|url, status, err, _| { \\ ' \
  '     ${pad}${pad}view! { \\ ' \
  '     ${pad}${pad}${pad}p { \\ ' \
  '     ${pad}${pad}${pad}${pad}\(format!\( \\ ' \
  '     ${pad}${pad}${pad}${pad}${pad}\"An error with HTTP code {} occured at {}: {}.\", \\ ' \
  '     ${pad}${pad}${pad}${pad}${pad}status, \\ ' \
  '     ${pad}${pad}${pad}${pad}${pad}url, \\ ' \
  '     ${pad}${pad}${pad}${pad}${pad}err \\ ' \
  '     ${pad}${pad}${pad}${pad}\)\) \\ ' \
  '     ${pad}${pad}${pad}} \\ ' \
  '     ${pad}${pad}} \\ ' \
  '     ${pad}}\)\)" "${file_path}"' \
  '   fi' \
  '' \
  '   if [ -z "${has_plugins}" ]' \
  '   then' \
  '     sed -i "${use_line}s|^\(use perseus::{\)\(.*\)\(};\)|\1\2, Plugins\3|" "${file_path}"' \
  '     end_line=$( grep -ne "^}\$" "${file_path}" | grep -Eo "^[^:]+" )' \
  '     sed -i "${end_line}i \\ ' \
  '     ${pad}.plugins\( \\ ' \
  '     ${pad}${pad}Plugins::new\(\) \\ ' \
  '     ${pad}${pad}${pad}.plugin\( \\ ' \
  '     ${pad}${pad}${pad}${pad}perseus_size_opt, \\ ' \
  '     ${pad}${pad}${pad}${pad}SizeOpts { \\ ' \
  '     ${pad}${pad}${pad}${pad}${pad}codegen_units: 1, \\ ' \
  '     ${pad}${pad}${pad}${pad}${pad}enable_fluent_bundle_patch: false, \\ ' \
  '     ${pad}${pad}${pad}${pad}${pad}lto: true, \\ ' \
  '     ${pad}${pad}${pad}${pad}${pad}opt_level: \"s\".to_string\(\), \\ ' \
  '     ${pad}${pad}${pad}${pad}${pad}wee_alloc: true, \\ ' \
  '     ${pad}${pad}${pad}${pad}} \\ ' \
  '     ${pad}${pad}${pad}\) \\ ' \
  '     ${pad}\)" "${file_path}"' \
  '   fi' \
  '' \
  '   if [ -z "${has_sycamore}" ]' \
  '   then' \
  '     sed -i "${use_line}a use sycamore::view;" "${file_path}"' \
  '   fi' \
  '' \
  '   sed -i "${use_line}a use perseus_size_opt::{perseus_size_opt, SizeOpts};" "${file_path}"' \
  '   sed -i "${mod_line}d" "${file_path}"' \
  '}' \
  '' \
  'patch_lib_rs' > ./patch_lib_rs.sh \
  && sed -i "s|^\(.*\)[\ ]\{1\}$|\1|g" ./patch_lib_rs.sh \
  && echo "* * SH CONTENTS * *" \
  && cat -n ./patch_lib_rs.sh \
  && chmod +x ./patch_lib_rs.sh && . ./patch_lib_rs.sh && rm -f ./patch_lib_rs.sh \
  && echo "* * SH 1 * *" \
  && cat -n ./examples/${EXAMPLE_CATEGORY}/${EXAMPLE_NAME}/src/lib.rs \
  && sed -i "\
  s|^\(sycamore = \"\)\([\^0-9\.beta-]\{3,\}\)\(.*\)$|\1=${SYCAMORE_VERSION}\3|g; \
  s|^\(sycamore = { version = \"\)\([\^0-9\.beta-]\{3,\}\)\(.*\)$|\1=${SYCAMORE_VERSION}\3|g;" \
  ./examples/core/basic/Cargo.toml \
  && echo "* * SED 4 * *" \
  && cat -n ./examples/core/basic/Cargo.toml \
  && sed -i "\
  s|^\(sycamore = \"\)\([\^0-9\.beta-]\{3,\}\)\(.*\)$|\1=${SYCAMORE_VERSION}\3|g; \
  s|^\(sycamore = { version = \"\)\([\^0-9\.beta-]\{3,\}\)\(.*\)$|\1=${SYCAMORE_VERSION}\3|g; \
  s|^\(sycamore-router = \"\)\([\^0-9\.beta-]\{3,\}\)\(.*\)$|\1=${SYCAMORE_VERSION}\3|g; \
  s|^\(sycamore-router = { version = \"\)\([\^0-9\.beta-]\{3,\}\)\(.*\)$|\1=${SYCAMORE_VERSION}\3|g;" \
  ./examples/core/basic/.perseus/Cargo.toml \
  && echo "* * SED 5 * *" \
  && cat -n ./examples/core/basic/.perseus/Cargo.toml

# Create a build stage for `perseus-cli` that we can run in parallel.
FROM framework as perseus-cli

# Copy perseus-size-opt to satisfy dependencies.
COPY --from=perseus-size-opt /perseus-size-opt/ /perseus-size-opt/

# Work from the root of the package.
WORKDIR /perseus/packages/perseus-cli

# Compile the release binary target of package `perseus-cli`.
RUN cargo build --bin perseus --release

# Create a build stage for building our app.
FROM framework as builder

# Copy the tools we previously prepared in parallel.
COPY --from=binaryen /binaryen/bin/ /usr/bin/
COPY --from=binaryen /binaryen/include/ /usr/include/
COPY --from=binaryen /binaryen/lib/ /usr/lib/
COPY --from=esbuild /esbuild/bin/esbuild /usr/bin/
COPY --from=perseus-cli /perseus/target/release/perseus /usr/bin/
COPY --from=perseus-size-opt /perseus-size-opt/ /perseus-size-opt/
COPY --from=wasm-pack /wasm-pack/wasm-pack /usr/bin/

# Work from the root of our app.
WORKDIR /perseus/examples/core/state_generation

# Execute all necessary commands for deploying our app.
RUN . /etc/profile && . /usr/local/cargo/env \
  && perseus clean \
  && perseus prep \
  && perseus tinker \
  && printf '%s\n' \
  '#!/bin/sh' \
  'parse_file () {' \
  '  local file_path=./.perseus/src/lib.rs' \
  '  local line_num=$( grep -ne "clippy" $file_path | grep -Eo "^[^:]+" )' \
  '  if [ ! -z "${line_num}" ] && [ $line_num -ne 1 ]' \
  '  then' \
  '    awk -i inplace \\ ' \
  '    -v line_num=$line_num \\ ' \
  '    -v inner_attr=$( sed "${line_num}q;d" $file_path | cut -d " " -f 1 ) \\ ' \
  '    "NR==1 { print inner_attr } NR!=line_num { print }" $file_path' \
  '  fi' \
  '}' \
  'parse_file' > ./parse_file.sh \
  && chmod +x ./parse_file.sh && . ./parse_file.sh && rm -f ./parse_file.sh \
  && export PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig \
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
COPY --from=builder /perseus/examples/core/state_generation/pkg /app/

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

## Building and Running

Save the `Dockerfile` below to a directory of your choice.

To build a container using the `Dockerfile`, navigate to the directory where the `Dockerfile` was saved and use a `docker` command similar to the following:

```shellscript
docker buildx build --no-cache \
--build-arg PERSEUS_VERSION=0.3.5 \
--tag="my-perseus-example:0.1.0" \
-f Dockerfile .
```

To run your newly built container, use the following command:

```shellscript
docker run --init -p 80:8080 my-perseus-example:0.1.0
```

Point your browser to `localhost` to interact with the app! To stop the server, press `Ctrl+C` in your terminal.

---

## Dockerfile

```dockerfile
# Pull the base image.
FROM rust:1.57-slim AS base

# Define optional command-line build arguments we can pass to docker.
ARG EXAMPLE_CATEGORY \
  EXAMPLE_NAME \
  PERSEUS_VERSION \
  SYCAMORE_VERSION \
  WEE_ALLOC_VERSION \
  BINARYEN_VERSION \
  ESBUILD_VERSION \
  ESBUILD_TARGET \
  WASM_PACK_VERSION \
  WASM_TARGET \
  RUST_RELEASE_CHANNEL \
  CARGO_NET_GIT_FETCH_WITH_CLI

# Export environment variables.
ENV EXAMPLE_CATEGORY=${EXAMPLE_CATEGORY:-demos} \
  EXAMPLE_NAME=${EXAMPLE_NAME:-fetching} \
  PERSEUS_VERSION=${PERSEUS_VERSION:-0.3.4} \
  SYCAMORE_VERSION=${SYCAMORE_VERSION:-0.7.1} \
  WEE_ALLOC_VERSION=${WEE_ALLOC_VERSION:-0.4.5} \
  BINARYEN_VERSION=${BINARYEN_VERSION:-104} \
  ESBUILD_VERSION=${ESBUILD_VERSION:-0.14.7} \
  WASM_PACK_VERSION=${WASM_PACK_VERSION:-0.10.3} \
  WASM_TARGET=${WASM_TARGET:-wasm32-unknown-unknown} \
  RUST_RELEASE_CHANNEL=${RUST_RELEASE_CHANNEL:-stable} \
  CARGO_NET_GIT_FETCH_WITH_CLI=${CARGO_NET_GIT_FETCH_WITH_CLI:-false}

# Single-threaded perseus CLI mode required for low memory environments.
# ENV PERSEUS_CLI_SEQUENTIAL=true

# Work from the root of the project.
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
  && rustup install ${RUST_RELEASE_CHANNEL} \
  && rustup default ${RUST_RELEASE_CHANNEL} \
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

# Download and make adjustments to the codebase of the framework.
RUN curl --progress-bar -L https://codeload.github.com/arctic-hen7/perseus/tar.gz/v${PERSEUS_VERSION} \
  | tar -xz --strip-components=1 \
  && sed -i "\
  s|\(println!.*\)$|// \1|; \
  s|\.\./\.\.examples/core/basic/\.perseus|/perseus/examples/core/basic/\.perseus|g; \
  s|\(fs::remove_dir_all(dest\.join(\"\.perseus/dist\"))\.unwrap();\)|// \1|; \
  s|PERSEUS_VERSION|path = \\\\\"/perseus/packages/perseus\\\\\"|g; \
  s|PERSEUS_ACTIX_WEB_VERSION|path = \\\\\"/perseus/packages/perseus-actix-web\\\\\"|g; \
  s|PERSEUS_WARP_VERSION|path = \\\\\"/perseus/packages/perseus-warp\\\\\"|g;" \
  ./packages/perseus-cli/build.rs \
  && sed -i "\
  s|^\(sycamore = \"\)\([0-9\.beta-]\{3,\}\)\(.*\)$|\1=${SYCAMORE_VERSION}\3|g; \
  s|^\(sycamore = { version = \"\)\([0-9\.beta-]\{3,\}\)\(.*\)$|\1=${SYCAMORE_VERSION}\3|g; \
  s|\(\[dependencies\]\)$|\1\nwee_alloc = \"=${WEE_ALLOC_VERSION}\"|; \
  " ./examples/${EXAMPLE_CATEGORY}/${EXAMPLE_NAME}/Cargo.toml \
  && sed -i "1i \
  #[global_allocator]\n\
  static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;\n\
  " ./examples/${EXAMPLE_CATEGORY}/${EXAMPLE_NAME}/src/lib.rs \
  && sed -i "\
  s|^\(sycamore = \"\)\([0-9\.beta-]\{3,\}\)\(.*\)$|\1=${SYCAMORE_VERSION}\3|g; \
  s|^\(sycamore = { version = \"\)\([0-9\.beta-]\{3,\}\)\(.*\)$|\1=${SYCAMORE_VERSION}\3|g; \
  " ./examples/core/basic/Cargo.toml \
  && sed -i "\
  s|^\(sycamore = \"\)\([0-9\.beta-]\{3,\}\)\(.*\)$|\1=${SYCAMORE_VERSION}\3|g; \
  s|^\(sycamore = { version = \"\)\([0-9\.beta-]\{3,\}\)\(.*\)$|\1=${SYCAMORE_VERSION}\3|g; \
  s|^\(sycamore-router = \"\)\([0-9\.beta-]\{3,\}\)\(.*\)$|\1=${SYCAMORE_VERSION}\3|g; \
  s|^\(sycamore-router = { version = \"\)\([0-9\.beta-]\{3,\}\)\(.*\)$|\1=${SYCAMORE_VERSION}\3|g; \
  " ./examples/core/basic/.perseus/Cargo.toml \
  && printf '%s\n' \
  '' '' \
  '[profile.release]' \
  'codegen-units = 1' \
  'opt-level = "s"' \
  'lto = true' >> ./examples/core/basic/.perseus/Cargo.toml

# Create a build stage for `perseus-cli` that we can run in parallel.
FROM framework as perseus-cli

# Work from the root of the package.
WORKDIR /perseus/packages/perseus-cli

# Compile the release binary target of package `perseus-cli`.
RUN cargo build --bin perseus --release

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
WORKDIR /perseus/examples/demos/fetching

# Execute all necessary commands for deploying our app.
RUN . /etc/profile && . /usr/local/cargo/env \
  && perseus clean \
  && perseus prep \
  && perseus eject \
  && printf '%s\n' \
  '#!/bin/sh' \
  'parse_file () {' \
  '  local file_path=./.perseus/src/lib.rs' \
  '  local line_num=$( grep -ne "clippy" $file_path | grep -Eo "^[^:]+" )' \
  '  if [ ! -z "${line_num}" ] && [ $line_num -ne 1 ]' \
  '  then' \
  '    awk -i inplace \\ ' \
  '    -v line_num=$line_num \\ ' \
  '    -v inner_attr=$( sed "${line_num}q;d" $file_path | cut -d " " -f 1 ) \\ ' \
  '    "NR==1 { print inner_attr } NR!=line_num { print }" $file_path' \
  '  fi' \
  '}' \
  'parse_file' > ./parse_file.sh \
  && chmod +x ./parse_file.sh && . ./parse_file.sh && rm -f ./parse_file.sh \
  && export PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig \
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
COPY --from=builder /perseus/examples/demos/fetching/pkg /app/

# Bind the server to `localhost`.
ENV PERSEUS_HOST=0.0.0.0

# Bind the container to the default port of 8080.
ENV PERSEUS_PORT=8080

# Configure the container to automatically serve the deployed app while running.
CMD ["./server"]
```

</details>

<details>
<summary>Test example for deploying a specific branch from the Perseus repository</summary>

```dockerfile
# Pull base image.
FROM rust:1.57-slim AS base

# Define optional command-line build arguments we can pass to docker.
ARG EXAMPLE_CATEGORY \
  EXAMPLE_NAME \
  PERSEUS_BRANCH \
  BONNIE_VERSION \
  BINARYEN_VERSION \
  ESBUILD_VERSION \
  ESBUILD_TARGET \
  WASM_BINDGEN_CLI_VERSION \
  WASM_PACK_VERSION \
  WASM_TARGET \
  RUST_RELEASE_CHANNEL \
  CARGO_NET_GIT_FETCH_WITH_CLI

# Export environment variables.
ENV EXAMPLE_CATEGORY=${EXAMPLE_CATEGORY:-demos} \
  EXAMPLE_NAME=${EXAMPLE_NAME:-fetching} \
  PERSEUS_BRANCH=${PERSEUS_BRANCH:-main} \
  BONNIE_VERSION=${BONNIE_VERSION:-0.3.2} \
  BINARYEN_VERSION=${BINARYEN_VERSION:-104} \
  ESBUILD_VERSION=${ESBUILD_VERSION:-0.14.7} \
  ESBUILD_TARGET=${ESBUILD_TARGET:-es6} \
  WASM_BINDGEN_CLI_VERSION=${WASM_BINDGEN_CLI_VERSION:-0.2.79} \
  WASM_PACK_VERSION=${WASM_PACK_VERSION:-0.10.3} \
  WASM_TARGET=${WASM_TARGET:-wasm32-unknown-unknown} \
  RUST_RELEASE_CHANNEL=${RUST_RELEASE_CHANNEL:-stable} \
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
  npm \
  openssl \
  pkg-config \
  tree \
  && rustup install $RUST_RELEASE_CHANNEL \
  && rustup default $RUST_RELEASE_CHANNEL \
  && rustup target add $WASM_TARGET

# Create a build stage for `bonnie` we can run in parallel.
FROM base AS bonnie

# Work from the chosen install path for `bonnie`.
WORKDIR /bonnie

# Install crate `bonnie` into the work path.
RUN cargo install bonnie --version $BONNIE_VERSION \
  && mv /usr/local/cargo/bin/bonnie .

# Create a build stage for `binaryen` we can run in parallel.
FROM base AS binaryen

# Work from the chosen install path for `binaryen`.
WORKDIR /binaryen

# Download, extract, and remove compressed tar of `binaryen`.
RUN curl --progress-bar -Lo binaryen-${BINARYEN_VERSION}.tar.gz \
  https://github.com/WebAssembly/binaryen/releases/download/version_${BINARYEN_VERSION}/binaryen-version_${BINARYEN_VERSION}-x86_64-linux.tar.gz \
  && tar --strip-components=1 -xzf binaryen-${BINARYEN_VERSION}.tar.gz \
  && rm -f binaryen-${BINARYEN_VERSION}.tar.gz

# Create a build stage for `esbuild` we can run in parallel.
FROM base AS esbuild

# Work from the chosen install path for `esbuild`.
WORKDIR /esbuild

# Download, extract, and remove compressed tar of `esbuild`.
RUN curl --progress-bar -Lo esbuild-${ESBUILD_VERSION}.tar.gz \
  https://registry.npmjs.org/esbuild-linux-64/-/esbuild-linux-64-${ESBUILD_VERSION}.tgz \
  && tar --strip-components=1 -xzf esbuild-${ESBUILD_VERSION}.tar.gz \
  && rm -f esbuild-${ESBUILD_VERSION}.tar.gz

# Create a build stage for `rust-script` we can run in parallel.
FROM base AS rust-script

# Work from the chosen install path for `rust-script`.
WORKDIR /rust-script

# Install crate `rust-script` into the work path.
RUN cargo install rust-script \
  && mv /usr/local/cargo/bin/rust-script .

# Create a build stage for `wasm-bindgen-cli` we can run in parallel.
FROM base AS wasm-bindgen-cli

# Work from the chosen install path for `wasm-bindgen-cli`.
WORKDIR /wasm-bindgen

# Install crate `wasm-bindgen-cli` into the work path.
RUN cargo install wasm-bindgen-cli --version $WASM_BINDGEN_CLI_VERSION \
  && mv /usr/local/cargo/bin/wasm* .

# Create a build stage for `wasm-pack` we can run in parallel.
FROM base AS wasm-pack

# Work from the chosen install path for `wasm-pack`.
WORKDIR /wasm-pack

# Install crate `wasm-pack` into the work path.
RUN cargo install wasm-pack --version $WASM_PACK_VERSION \
  && mv /usr/local/cargo/bin/wasm-pack .

# Create a build stage for the codebase of the `perseus` framework we can run in parallel.
FROM base AS framework

# Copy bonnie to satisfy implementation.
COPY --from=bonnie /bonnie/bonnie /usr/bin/

# Work from the root of the codebase.
WORKDIR /perseus

# Compile and install `perseus` from the current state of a branch in the official repo.
RUN curl --progress-bar -L \
  https://codeload.github.com/arctic-hen7/perseus/tar.gz/${PERSEUS_BRANCH} \
  | tar -xz --strip-components=1 \
  && sed -i "\
  s|\(cargo build\)|\1 --bin perseus|; \
  s|\`|'|g" ./bonnie.toml \
  && bonnie setup

# Create a build stage for building our app.
FROM framework AS builder

# Copy the tools we previously prepared in parallel.
COPY --from=bonnie /bonnie/bonnie /usr/bin/
COPY --from=binaryen /binaryen/bin/ /usr/bin/
COPY --from=binaryen /binaryen/include/ /usr/include/
COPY --from=binaryen /binaryen/lib/ /usr/lib/
COPY --from=esbuild /esbuild/bin/esbuild /usr/bin/
COPY --from=rust-script /rust-script/rust-script /usr/bin/
COPY --from=wasm-pack /wasm-pack/wasm-pack /usr/bin/
COPY --from=wasm-bindgen-cli /wasm-bindgen/ /usr/bin/

# Work from the root of the codebase.
WORKDIR /perseus

# Execute all necessary commands for deploying our app.
RUN ln -s /perseus/target/debug/perseus /usr/bin/perseus \
  && . /etc/profile && . /usr/local/cargo/env \
  && curl --progress-bar -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.1/install.sh \
  | bash \
  && . ${HOME}/.bashrc \
  && nvm install node \
  && npm update -g \
  && . /etc/profile && . ${HOME}/.bashrc && . /usr/local/cargo/env \
  && npm i -g browser-sync concurrently serve tailwindcss \
  && export PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig \
  && bonnie dev example $EXAMPLE_CATEGORY $EXAMPLE_NAME deploy \
  && esbuild ./examples/${EXAMPLE_CATEGORY}/${EXAMPLE_NAME}/pkg/dist/pkg/perseus_engine.js \
  --minify \
  --target=${ESBUILD_TARGET} \
  --outfile=./pkg/dist/pkg/perseus_engine.js \
  --allow-overwrite \
  && wasm-opt \
  -Os ./examples/${EXAMPLE_CATEGORY}/${EXAMPLE_NAME}/pkg/dist/pkg/perseus_engine_bg.wasm \
  -o ./examples/${EXAMPLE_CATEGORY}/${EXAMPLE_NAME}/pkg/dist/pkg/perseus_engine_bg.wasm

# Prepare the final image where the app will be deployed.
FROM debian:stable-slim

# Work from a chosen install path for the deployed app.
WORKDIR /app

# Copy the deployed app into its chosen install path.
COPY --from=builder /perseus/examples/demos/fetching/pkg /app/

# Bind the container to `localhost`.
ENV PERSEUS_HOST=0.0.0.0

# Bind the container to the default port of `8080`.
ENV PERSEUS_PORT=8080

# Configure the container to automatically serve the deployed app while running.
CMD ["./server"]
```

</details>
