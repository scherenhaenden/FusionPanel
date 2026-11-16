FROM ubuntu:24.04@sha256:77eb101c607d6e3d4f5ad0c8a02c2136c689b98aba1f359e6502ba517e5c57ec

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update \
    && apt-get install --no-install-recommends -y ca-certificates libgcc-s1 \
    && rm -rf /var/lib/apt/lists/*

COPY seed-admin.sh /usr/local/bin/seed-admin.sh
RUN chmod 0555 /usr/local/bin/seed-admin.sh

ENTRYPOINT ["/usr/local/bin/seed-admin.sh"]
