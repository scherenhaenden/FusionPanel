ARG UBUNTU_BASE=ubuntu:24.04@sha256:77eb101c607d6e3d4f5ad0c8a02c2136c689b98aba1f359e6502ba517e5c57ec
FROM ${UBUNTU_BASE}

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update \
    && apt-get install --no-install-recommends -y busybox-static \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /srv/www

COPY index.html /srv/www/index.html
RUN chmod 0444 /srv/www/index.html

EXPOSE 8080
STOPSIGNAL SIGKILL

CMD ["busybox", "httpd", "-f", "-p", "0.0.0.0:8080", "-h", "/srv/www"]
