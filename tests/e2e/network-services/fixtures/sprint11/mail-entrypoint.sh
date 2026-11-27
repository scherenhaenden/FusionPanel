#!/usr/bin/env bash
set -euo pipefail

MAIL_ROOT="${FUSIONPANEL_MAIL_ROOT:-/var/lib/fusionpanel/mail}"

# Ensure vmail user/group (uid/gid 5000 matches agent mail_driver.rs dovecot/users output)
if ! getent group vmail >/dev/null 2>&1; then
  groupadd -g 5000 vmail
fi
if ! id -u vmail >/dev/null 2>&1; then
  useradd -u 5000 -g 5000 -s /usr/sbin/nologin -d "$MAIL_ROOT/vmail" -M vmail
fi

mkdir -p \
  "$MAIL_ROOT/postfix" \
  "$MAIL_ROOT/dovecot" \
  "$MAIL_ROOT/dkim/keys" \
  "$MAIL_ROOT/vmail" \
  "$MAIL_ROOT/state" \
  /etc/ssl/mail \
  /var/spool/postfix/private

chown -R vmail:vmail "$MAIL_ROOT/vmail"
chmod 0755 "$MAIL_ROOT" "$MAIL_ROOT/postfix" "$MAIL_ROOT/dovecot" "$MAIL_ROOT/dkim" "$MAIL_ROOT/dkim/keys"

touch \
  "$MAIL_ROOT/postfix/virtual_mailbox_domains" \
  "$MAIL_ROOT/postfix/virtual_mailbox_maps" \
  "$MAIL_ROOT/postfix/virtual_alias_maps" \
  "$MAIL_ROOT/dovecot/users" \
  "$MAIL_ROOT/dkim/KeyTable" \
  "$MAIL_ROOT/dkim/SigningTable"

postmap "$MAIL_ROOT/postfix/virtual_mailbox_domains"
postmap "$MAIL_ROOT/postfix/virtual_mailbox_maps"
postmap "$MAIL_ROOT/postfix/virtual_alias_maps"

# Generate fixture TLS certificate for SMTP STARTTLS and IMAP STARTTLS
if [ ! -f /etc/ssl/mail/cert.pem ] || [ ! -f /etc/ssl/mail/key.pem ]; then
  openssl req -x509 -newkey rsa:2048 -nodes -days 7 \
    -keyout /etc/ssl/mail/key.pem \
    -out /etc/ssl/mail/cert.pem \
    -subj "/CN=mail.mail-e2e.local" \
    -addext "subjectAltName=DNS:mail.mail-e2e.local,DNS:localhost,IP:127.0.0.1" >/dev/null 2>&1
  chmod 0600 /etc/ssl/mail/key.pem
  chmod 0644 /etc/ssl/mail/cert.pem
fi

# Configure Postfix
postconf -e "myhostname = mail.mail-e2e.local"
postconf -e "mydomain = mail-e2e.local"
postconf -e "myorigin = \$myhostname"
postconf -e "mydestination = localhost"
postconf -e "inet_interfaces = all"
postconf -e "inet_protocols = ipv4"
postconf -e "mynetworks = 127.0.0.0/8"
postconf -e "virtual_mailbox_domains = hash:$MAIL_ROOT/postfix/virtual_mailbox_domains"
postconf -e "virtual_mailbox_maps = hash:$MAIL_ROOT/postfix/virtual_mailbox_maps"
postconf -e "virtual_alias_maps = hash:$MAIL_ROOT/postfix/virtual_alias_maps"
postconf -e "virtual_transport = lmtp:unix:private/dovecot-lmtp"
postconf -e "smtpd_sasl_type = dovecot"
postconf -e "smtpd_sasl_path = private/auth"
postconf -e "smtpd_sasl_auth_enable = yes"
postconf -e "smtpd_sasl_authenticated_header = yes"
postconf -e "smtpd_tls_cert_file = /etc/ssl/mail/cert.pem"
postconf -e "smtpd_tls_key_file = /etc/ssl/mail/key.pem"
postconf -e "smtpd_tls_security_level = may"
postconf -e "smtpd_tls_auth_only = no"
postconf -e "smtpd_relay_restrictions = permit_mynetworks, permit_sasl_authenticated, reject_unauth_destination"
postconf -e "smtpd_recipient_restrictions = permit_sasl_authenticated, reject_unauth_destination"
postconf -e "milter_protocol = 6"
postconf -e "milter_default_action = tempfail"
postconf -e "smtpd_milters = inet:127.0.0.1:8891"
postconf -e "non_smtpd_milters = inet:127.0.0.1:8891"
postconf -e "maillog_file = /var/log/mail.log"

postconf -M submission/inet="submission inet n - n - - smtpd"
postconf -P "submission/inet/syslog_name=postfix/submission"
postconf -P "submission/inet/smtpd_tls_security_level=encrypt"
postconf -P "submission/inet/smtpd_sasl_auth_enable=yes"
postconf -P "submission/inet/smtpd_tls_auth_only=yes"
postconf -P "submission/inet/smtpd_client_restrictions=permit_sasl_authenticated,reject"
postconf -P "submission/inet/smtpd_relay_restrictions=permit_sasl_authenticated,reject"

# Configure Dovecot
cat > /etc/dovecot/conf.d/99-fusionpanel.conf <<EOF
protocols = imap lmtp
listen = *
ssl = yes
ssl_cert = </etc/ssl/mail/cert.pem
ssl_key = </etc/ssl/mail/key.pem
disable_plaintext_auth = yes
auth_mechanisms = plain login
mail_uid = 5000
mail_gid = 5000
mail_location = maildir:$MAIL_ROOT/vmail/%d/%n
log_path = /var/log/dovecot.log

passdb {
  driver = passwd-file
  args = scheme=ARGON2ID $MAIL_ROOT/dovecot/users
}

userdb {
  driver = passwd-file
  args = $MAIL_ROOT/dovecot/users
}

service lmtp {
  unix_listener /var/spool/postfix/private/dovecot-lmtp {
    mode = 0600
    user = postfix
    group = postfix
  }
}

service auth {
  unix_listener /var/spool/postfix/private/auth {
    mode = 0660
    user = postfix
    group = postfix
  }
}
EOF

# Configure OpenDKIM
cat > /etc/opendkim.conf <<EOF
Syslog                  yes
LogWhy                  yes
UMask                   007
Mode                    s
Canonicalization        relaxed/relaxed
KeyTable                file:$MAIL_ROOT/dkim/KeyTable
SigningTable            refile:$MAIL_ROOT/dkim/SigningTable
InternalHosts           0.0.0.0/0
Socket                  inet:8891@127.0.0.1
RequireSafeKeys         false
PidFile                 /run/opendkim/opendkim.pid
EOF

mkdir -p /run/opendkim
chown opendkim:opendkim /run/opendkim

start_opendkim() {
  if [ -f /run/opendkim/opendkim.pid ]; then
    local pid
    pid=$(cat /run/opendkim/opendkim.pid 2>/dev/null || true)
    if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
      kill "$pid" || true
      sleep 0.2
    fi
  fi
  /usr/sbin/opendkim -x /etc/opendkim.conf
}

start_opendkim
/usr/sbin/dovecot
/usr/sbin/postfix start

exec python3 - <<'PY'
import http.server
import os
import subprocess
import urllib.parse

MAIL_ROOT = os.environ.get("FUSIONPANEL_MAIL_ROOT", "/var/lib/fusionpanel/mail")
FAIL_SENTINEL = os.path.join(MAIL_ROOT, "state", "fail-next-validate")

def run(cmd):
    res = subprocess.run(cmd, capture_output=True, text=True)
    if res.returncode != 0:
        raise RuntimeError(f"{' '.join(cmd)} failed: {res.stderr or res.stdout}")

class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        parsed = urllib.parse.urlparse(self.path)
        params = urllib.parse.parse_qs(parsed.query)
        try:
            if parsed.path == "/validate":
                if os.path.exists(FAIL_SENTINEL):
                    os.remove(FAIL_SENTINEL)
                    raise RuntimeError("staged mail config validation failure triggered")
                staging = params.get("staging", [""])[0]
                if staging:
                    for name in ("virtual_mailbox_domains", "virtual_mailbox_maps", "virtual_alias_maps"):
                        p = os.path.join(staging, "postfix", name)
                        if os.path.exists(p):
                            run(["postmap", p])
                run(["postconf", "-n"])
                run(["doveconf", "-n"])
            elif parsed.path == "/reload":
                for name in ("virtual_mailbox_domains", "virtual_mailbox_maps", "virtual_alias_maps"):
                    p = os.path.join(MAIL_ROOT, "postfix", name)
                    if os.path.exists(p):
                        run(["postmap", p])
                run(["chown", "-R", "vmail:vmail", os.path.join(MAIL_ROOT, "vmail")])
                run(["postfix", "reload"])
                run(["doveadm", "reload"])
                run(["bash", "-c", "pkill -x opendkim || true; /usr/sbin/opendkim -x /etc/opendkim.conf"])
            else:
                self.send_response(404)
                self.end_headers()
                return
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b"ok\n")
        except Exception as exc:
            self.send_response(500)
            self.end_headers()
            self.wfile.write(f"{exc}\n".encode("utf-8"))

    def log_message(self, format, *args):
        pass

http.server.HTTPServer(("0.0.0.0", 9025), Handler).serve_forever()
PY
