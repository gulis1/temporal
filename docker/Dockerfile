FROM rust:slim-buster

COPY . /controller_prueba
COPY ./docker/config /root/.kube/config
WORKDIR /controller_prueba

RUN apt-get update -y && \
    apt install -y pkg-config openssl libssl-dev && \
    cargo build --release

ENTRYPOINT ["cargo", "run", "--release"]
#ENTRYPOINT ["bash"]
