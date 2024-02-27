FROM alpine:3.19 as build

RUN apk add musl musl-dev openssl-dev cargo

COPY . /controller-prueba
WORKDIR /controller-prueba

RUN cargo build --release
COPY ./target/release/controller-prueba /controller

FROM alpine:3.19 
RUN apk add libgcc
COPY ./docker/config /root/.kube/config
COPY --from=build /controller-prueba/target/release/controller-prueba /controller

ENTRYPOINT ["/controller"]
