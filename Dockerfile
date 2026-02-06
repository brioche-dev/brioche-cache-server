FROM ubuntu:24.04
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
ARG TARGETARCH
ARG TARGETOS
COPY ./artifacts/$TARGETARCH-$TARGETOS/brioche-cache-server /usr/local/bin/brioche-cache-server
RUN chmod +x /usr/local/bin/brioche-cache-server
EXPOSE 3000/tcp
EXPOSE 3001/tcp
CMD ["brioche-cache-server"]
