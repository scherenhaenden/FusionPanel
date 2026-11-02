export const isValidFQDN = (domain: string): boolean => {
  // Regex to validate a Fully Qualified Domain Name (FQDN)
  // It ensures the domain ends with a TLD and consists of valid alphanumeric characters and hyphens.
  const fqdnRegex = /^(?=.{1,253}$)(?:(?!-)[A-Za-z0-9-]{1,63}(?<!-)\.)+[A-Za-z]{2,63}$/;
  return fqdnRegex.test(domain);
};
