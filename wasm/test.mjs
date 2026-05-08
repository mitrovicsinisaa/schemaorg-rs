/**
 * Node.js integration test for @schemaorg-rs/wasm.
 *
 * Run after building:
 *   ./scripts/build-wasm.sh node
 *   node wasm/test.mjs
 */

import { init, extract, validateHtml, schemaVersion } from './index.js';
import { strict as assert } from 'node:assert';

// Force Node.js target
await init('nodejs');

console.log('── Schema Version ──');
const version = await schemaVersion();
console.log(`  Version: ${version}`);
assert.ok(version.length > 0, 'Version should not be empty');

console.log('\n── Extract (JSON-LD) ──');
const html = `<html><head>
  <script type="application/ld+json">{
    "@context": "https://schema.org",
    "@type": "Product",
    "name": "Widget",
    "image": "https://example.com/widget.jpg",
    "description": "A great widget"
  }</script>
</head></html>`;

const extractResult = await extract(html);
assert.ok(!extractResult.error, `Extract should not error: ${extractResult.error}`);
assert.ok(extractResult.nodes.length > 0, 'Should extract at least one node');
assert.equal(extractResult.nodes[0].types[0], 'Product');
console.log(`  Nodes: ${extractResult.nodes.length}`);
console.log(`  Type: ${extractResult.nodes[0].types[0]}`);

console.log('\n── Validate (Google Profile) ──');
const validateResult = await validateHtml(html, 'google');
assert.ok(!validateResult.error, `Validate should not error: ${validateResult.error}`);
assert.ok(validateResult.extraction.nodes.length > 0, 'Should have extraction results');
assert.ok(validateResult.profile, 'Should have profile results');
assert.ok(
  ['Eligible', 'WarningsOnly', 'NotEligible', 'Restricted'].includes(validateResult.profile.eligibility),
  `Eligibility should be valid: ${validateResult.profile.eligibility}`
);
console.log(`  Eligibility: ${validateResult.profile.eligibility}`);
console.log(`  Type results: ${validateResult.profile.type_results.length}`);
console.log(`  Diagnostics: ${validateResult.profile.diagnostics.length}`);

console.log('\n── Validate (Baseline Profile) ──');
const baselineResult = await validateHtml(html, 'baseline');
assert.ok(!baselineResult.error, 'Baseline should not error');
console.log(`  Eligibility: ${baselineResult.profile.eligibility}`);

console.log('\n── Extract (Empty HTML) ──');
const emptyResult = await extract('<html></html>');
assert.ok(!emptyResult.error, 'Empty HTML should not error');
assert.equal(emptyResult.nodes.length, 0, 'Empty HTML should have no nodes');
console.log(`  Nodes: ${emptyResult.nodes.length}`);

console.log('\n── Extract (Microdata) ──');
const microdataHtml = `<html><body>
  <div itemscope itemtype="https://schema.org/Article">
    <h1 itemprop="headline">Test Article</h1>
  </div>
</body></html>`;
const microdataResult = await extract(microdataHtml);
assert.ok(!microdataResult.error, 'Microdata should not error');
assert.equal(microdataResult.nodes[0].types[0], 'Article');
console.log(`  Type: ${microdataResult.nodes[0].types[0]}`);

console.log('\n══════════════════════════════════');
console.log('  ✅ All integration tests passed!');
console.log('══════════════════════════════════\n');
