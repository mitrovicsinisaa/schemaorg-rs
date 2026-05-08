/**
 * @schemaorg-rs/wasm — Schema.org structured data parser & validator.
 *
 * Lazy-loads the WASM binary on first use. All functions return
 * parsed JavaScript objects (the Rust side returns JSON strings,
 * this wrapper calls JSON.parse automatically).
 *
 * Usage (browser ESM):
 *   import { extract, validateHtml, schemaVersion } from '@schemaorg-rs/wasm';
 *   const result = await extract('<html>...</html>');
 *
 * Usage (Node.js):
 *   import { extract, validateHtml, schemaVersion } from '@schemaorg-rs/wasm';
 *   const result = await extract('<html>...</html>');
 */

let wasmModule = null;

/**
 * Initializes the WASM module. Called automatically on first use.
 * Can be called explicitly for eager loading.
 *
 * @param {string} [target='web'] - 'web' for browser ESM, 'nodejs' for Node.js
 * @returns {Promise<void>}
 */
export async function init(target = 'web') {
  if (wasmModule) return;

  if (target === 'nodejs') {
    const mod = await import(`./pkg/nodejs/schemaorg_rs.js`);
    wasmModule = mod;
  } else {
    const mod = await import(`./pkg/web/schemaorg_rs.js`);
    await mod.default();
    wasmModule = mod;
  }
}

/**
 * Detects the runtime environment and initializes appropriately.
 */
async function ensureInit() {
  if (wasmModule) return;

  const isNode =
    typeof globalThis.process !== 'undefined' &&
    typeof globalThis.process.versions?.node !== 'undefined';

  await init(isNode ? 'nodejs' : 'web');
}

/**
 * Extracts structured data from HTML.
 *
 * @param {string} html - The HTML document to parse
 * @returns {Promise<ExtractResult>} Parsed structured data
 */
export async function extract(html) {
  await ensureInit();
  const json = wasmModule.extract(html);
  return JSON.parse(json);
}

/**
 * Full validation pipeline: extract → validate → profile evaluate.
 *
 * @param {string} html - The HTML document to analyze
 * @param {string} [profile='google'] - Profile name ('google' or 'baseline')
 * @returns {Promise<ValidateResult>} Combined validation result
 */
export async function validateHtml(html, profile = 'google') {
  await ensureInit();
  const json = wasmModule.validate_html(html, profile);
  return JSON.parse(json);
}

/**
 * Returns the Schema.org vocabulary version used by this build.
 *
 * @returns {Promise<string>} Version string (e.g., "28.0")
 */
export async function schemaVersion() {
  await ensureInit();
  return wasmModule.schema_version();
}
