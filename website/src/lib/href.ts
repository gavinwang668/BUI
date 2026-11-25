// No Node or astro:content imports here: React islands (the /charts pages) use it in the browser too.

/** Prefix a site-relative path with the configured base (works under a sub-path deploy). */
export function href(path = ""): string {
  const base = import.meta.env.BASE_URL.replace(/\/$/, "");
  if (/^(https?:|mailto:|#)/.test(path)) return path;
  return `${base}/${path.replace(/^\//, "")}`;
}
