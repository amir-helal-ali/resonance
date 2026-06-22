// SSR is enabled by default. We disable it for routes that use Web Crypto,
// because the crypto APIs are browser-only and the keys must never leave
// the client. (Per-route `+page.ts` files can also disable SSR individually.)
export const ssr = false;
export const prerender = false;
