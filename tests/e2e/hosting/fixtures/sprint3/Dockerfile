ARG UBUNTU_BASE=ubuntu:24.04@sha256:77eb101c607d6e3d4f5ad0c8a02c2136c689b98aba1f359e6502ba517e5c57ec
FROM ${UBUNTU_BASE}

ARG NGINX_PACKAGE_VERSION=1.24.0-2ubuntu7.19
ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update \
    && apt-get install --no-install-recommends -y \
      "nginx=${NGINX_PACKAGE_VERSION}" \
      ca-certificates \
      libgcc-s1 \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /etc/nginx/fusionpanel

COPY start-node.sh /usr/local/bin/start-sprint3-node.sh
RUN chmod 0555 /usr/local/bin/start-sprint3-node.sh

ENTRYPOINT ["/usr/local/bin/start-sprint3-node.sh"]
