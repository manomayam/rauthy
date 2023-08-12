FROM alpine:3.18.2 AS builderBackend

WORKDIR /work

# Just a workaroudn to get an empty dir and be able to copy it over to scratch with
# the correct access rights
RUN mkdir data

FROM alpine:3.18.3

ARG BINARY

USER 10001:10001

WORKDIR /app

COPY --chown=10001:10001 /out/"$BINARY" ./rauthy
COPY --chown=10001:10001 --from=builderBackend /work/data ./data

COPY --chown=10001:10001 tls/ ./tls/
COPY --chown=10001:10001 rauthy.deploy.cfg ./rauthy.cfg

CMD ["/app/rauthy"]
