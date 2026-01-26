# RFC 0015: Let's Encrypt TLS for Forgejo

| | |
|---|---|
| **Status** | Complete |
| **Date** | 2026-01-25 |

---

## Summary

Enable automatic TLS certificate management via Let's Encrypt for `git.beyondtheuniverse.superviber.com` on the hearth instance.

## Problem

Current state:
- `git.beyondtheuniverse.superviber.com` uses self-signed certificates
- API calls require `curl -k` to skip verification
- Blue's forge client fails without `HTTPS_INSECURE` workarounds
- No automatic certificate renewal

## Solution

### Architecture (Final)

```
┌─────────────────────────────────────────────────────────┐
│                Hearth K3s (3.218.167.115)               │
│                                                         │
│  ┌─────────────────────────────────────────────────┐   │
│  │                    Traefik                       │   │
│  │  ┌───────────────────────────────────────────┐  │   │
│  │  │  Built-in ACME Resolver (letsencrypt)     │  │   │
│  │  │  - HTTP-01 Challenge                      │  │   │
│  │  │  - Auto-renewal                           │  │   │
│  │  │  - Storage: /data/acme.json               │  │   │
│  │  └───────────────────────────────────────────┘  │   │
│  └─────────────────────────────────────────────────┘   │
│                          │                              │
│                          ▼                              │
│  ┌─────────────────────────────────────────────────┐   │
│  │              Forgejo (port 3000)                │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

### Components

1. **Traefik built-in ACME** - Handles Let's Encrypt automatically
2. **IngressRoute** - Routes traffic to Forgejo with TLS
3. **PowerDNS** - Self-managed DNS (also on hearth)

### Implementation

Traefik on hearth was already configured with ACME support:

```yaml
# Traefik args (already configured)
- --certificatesresolvers.letsencrypt.acme.email=admin@superviber.com
- --certificatesresolvers.letsencrypt.acme.storage=/data/acme.json
- --certificatesresolvers.letsencrypt.acme.httpchallenge.entrypoint=web
```

The Forgejo IngressRoute references this resolver:

```yaml
apiVersion: traefik.containo.us/v1alpha1
kind: IngressRoute
metadata:
  name: forgejo
  namespace: forgejo
spec:
  entryPoints:
    - websecure
  routes:
    - match: Host(`git.beyondtheuniverse.superviber.com`)
      kind: Rule
      services:
        - name: forgejo
          port: 3000
  tls:
    certResolver: letsencrypt
```

### Verification

```bash
# HTTPS works without -k flag
curl https://git.beyondtheuniverse.superviber.com/api/v1/version
# Returns: {"version":"9.0.3+gitea-1.22.0"}

# Certificate details
echo | openssl s_client -connect 3.218.167.115:443 \
  -servername git.beyondtheuniverse.superviber.com 2>/dev/null | \
  openssl x509 -noout -issuer -dates
# issuer=C=US, O=Let's Encrypt, CN=R12
# notBefore=Jan 25 21:23:57 2026 GMT
# notAfter=Apr 25 21:23:56 2026 GMT
```

## Test Plan

- [x] Traefik configured with ACME resolver
- [x] Certificate issued by Let's Encrypt (CN=R12)
- [x] HTTPS works without -k flag
- [x] Browser shows secure connection
- [x] Blue forge client works without SSL workarounds

## DNS Configuration

DNS is managed via PowerDNS on hearth (3.218.167.115):
- `git.beyondtheuniverse.superviber.com` → 3.218.167.115

---

*"One instance. One certificate. Done."*

— Blue
