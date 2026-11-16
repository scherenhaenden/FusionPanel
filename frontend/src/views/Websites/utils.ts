export const isValidFQDN = (domain: string): boolean => {
  const fqdnRegex = /^(?=.{1,253}$)(?:(?!-)[A-Za-z0-9-]{1,63}(?<!-)\.)+[A-Za-z]{2,63}$/;
  return fqdnRegex.test(domain.trim());
};

export const validateDocumentRoot = (documentRoot: string, domain: string): string | null => {
  if (!documentRoot) return 'Document root is required.';
  if (documentRoot !== documentRoot.trim()) return 'Document root cannot start or end with whitespace.';
  if (/[\\\s\u0000-\u001f\u007f{};$"'`]/.test(documentRoot)) {
    return 'Document root contains unsupported characters.';
  }
  if (!documentRoot.startsWith('/')) return 'Document root must be an absolute path.';
  if (documentRoot.includes('//')) return 'Document root must not contain empty path segments.';

  const segments = documentRoot.split('/');
  if (segments.some((segment) => segment === '.' || segment === '..')) {
    return 'Document root must not contain traversal segments.';
  }

  const managedRoot = `/srv/fusionpanel/sites/${domain.toLowerCase()}`;
  if (documentRoot !== managedRoot && !documentRoot.startsWith(`${managedRoot}/`)) {
    return `Document root must stay under ${managedRoot}.`;
  }

  return null;
};

export const validateUpstreamUrl = (upstreamUrl: string): string | null => {
  if (!upstreamUrl) return 'Upstream URL is required.';
  if (upstreamUrl !== upstreamUrl.trim() || /[\s\u0000-\u001f\u007f{};$"'`\\]/.test(upstreamUrl)) {
    return 'Upstream URL contains unsupported characters.';
  }

  let parsed: URL;
  try {
    parsed = new URL(upstreamUrl);
  } catch {
    return 'Enter an absolute HTTP or HTTPS URL.';
  }

  if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
    return 'Upstream URL must use HTTP or HTTPS.';
  }
  const authorityStart = upstreamUrl.indexOf('://') + 3;
  const authority = upstreamUrl.slice(authorityStart).split(/[/?#]/, 1)[0];
  if (!authority || !parsed.hostname || parsed.username || parsed.password || authority.includes('@')) {
    return 'Upstream URL must include a host and cannot include credentials.';
  }
  if (parsed.port === '0') return 'Upstream URL port must be between 1 and 65535.';
  if (upstreamUrl.includes('?') || upstreamUrl.includes('#')) {
    return 'Upstream URL cannot include a query string or fragment.';
  }

  return null;
};
