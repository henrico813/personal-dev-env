// @ts-check

/**
 * @typedef {object} SlugOptions
 * @property {number} [maxLength] Longest slug to return. Defaults to 64.
 */

/**
 * Turns a title into a lowercase, URL-safe slug. It removes accents and
 * turns each run of characters outside a-z and 0-9, such as ß, into one
 * hyphen. A slug longer than `maxLength` is cut at the last hyphen at
 * or before that length, or at `maxLength` when there is no such
 * hyphen.
 *
 * @param {string} title
 * @param {SlugOptions} [options]
 * @returns {string}
 */
export function toSlug(title, { maxLength = 64 } = {}) {
  const slug = title
    // NFKD splits an accented letter into a base letter and a mark.
    .normalize("NFKD")
    .replace(/\p{M}/gu, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
  if (slug.length <= maxLength) {
    return slug;
  }
  const lastHyphen = slug.lastIndexOf("-", maxLength);
  return lastHyphen > 0 ? slug.slice(0, lastHyphen) : slug.slice(0, maxLength);
}
