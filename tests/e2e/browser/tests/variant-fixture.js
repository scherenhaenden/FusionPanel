function requireLoopbackUrl(name) {
  const raw = process.env[name];
  if (!raw) {
    throw new Error(`BLOCKED: ${name} must name the disposable loopback fixture`);
  }

  let url;
  try {
    url = new URL(raw);
  } catch {
    throw new Error(`BLOCKED: ${name} is not a valid URL`);
  }

  if (
    url.protocol !== 'http:' ||
    !['127.0.0.1', 'localhost'].includes(url.hostname) ||
    !url.port ||
    url.username ||
    url.password ||
    (url.pathname !== '' && url.pathname !== '/') ||
    url.search ||
    url.hash
  ) {
    throw new Error(`BLOCKED: ${name} must be an explicit HTTP loopback URL with a port`);
  }

  return url.origin;
}

export function requireVariantFixture() {
  if (process.env.FUSIONPANEL_E2E_DISPOSABLE !== '1') {
    throw new Error('BLOCKED: set FUSIONPANEL_E2E_DISPOSABLE=1 only for an owned disposable target');
  }

  const frontendUrl = requireLoopbackUrl('FUSIONPANEL_FRONTEND_URL');
  // The frontend can proxy API writes elsewhere; require an explicitly local API target too.
  requireLoopbackUrl('FUSIONPANEL_API_URL');
  const siteId = Number(process.env.FUSIONPANEL_E2E_VARIANT_SITE_ID);
  if (!Number.isSafeInteger(siteId) || siteId <= 0) {
    throw new Error('BLOCKED: FUSIONPANEL_E2E_VARIANT_SITE_ID must identify a disposable test site');
  }

  return { frontendUrl, siteId };
}
