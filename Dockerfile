FROM mcr.microsoft.com/devcontainers/base:ubuntu AS builder
RUN apt-get update && apt-get install -y build-essential libssl-dev
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

FROM builder AS build
WORKDIR /app
COPY . .
RUN cargo build --release

FROM node:25.2.1-trixie AS node
WORKDIR /extension
COPY ./vscode-extension .
RUN npm install
RUN mkdir build-target
RUN npm run build

FROM mcr.microsoft.com/devcontainers/base:ubuntu AS devcontainer
COPY --from=build /app/target/release/reSsg /bin/reSsg
COPY --from=node /extension/build-target/extension.vsix /home/vscode/ressg.vsix
