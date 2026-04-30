// No gql or graphql tagged templates — rover processes this file but finds nothing to extract.

export function formatPrice(cents: number): string {
  return `$${(cents / 100).toFixed(2)}`;
}

export const PLAIN_STRING = `this is not graphql`;
