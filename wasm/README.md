# @schemaorg-rs/wasm

Parse and validate Schema.org structured data (JSON-LD, Microdata, RDFa) — powered by Rust/WASM.

## Installation

```bash
npm install @schemaorg-rs/wasm
```

## Quick Start

### Browser (ESM)

```html
<script type="module">
  import { extract, validateHtml } from '@schemaorg-rs/wasm';

  const result = await validateHtml(document.documentElement.outerHTML, 'google');
  console.log(result.profile.eligibility);
</script>
```

### Node.js

```js
import { extract, validateHtml, schemaVersion } from '@schemaorg-rs/wasm';

const html = `<script type="application/ld+json">{
  "@context": "https://schema.org",
  "@type": "Product",
  "name": "Widget"
}</script>`;

// Extract structured data
const nodes = await extract(html);
console.log(nodes);

// Full validation pipeline
const result = await validateHtml(html, 'google');
console.log(result.profile.eligibility); // "Eligible" | "WarningsOnly" | "NotEligible" | "Restricted"

// Schema.org version
const version = await schemaVersion();
console.log(`Using Schema.org v${version}`);
```

## API

### `extract(html: string): Promise<ExtractResult>`

Parses HTML and extracts all structured data nodes (JSON-LD, Microdata, RDFa).

Returns:
```ts
{
  nodes: SchemaNode[];   // extracted structured data
  warnings: Warning[];   // non-fatal extraction warnings
}
```

### `validateHtml(html: string, profile?: string): Promise<ValidateResult>`

Full pipeline: extract → Schema.org vocabulary validation → profile evaluation.

**Profiles:**
- `"google"` (default) — Google Rich Results requirements
- `"baseline"` — generic Schema.org best practices

Returns:
```ts
{
  extraction: { nodes, warnings },
  validation: { diagnostics, has_errors },
  profile: {
    eligibility: "Eligible" | "WarningsOnly" | "NotEligible" | "Restricted",
    type_results: TypeEligibility[],
    diagnostics: ValidationDiagnostic[]
  }
}
```

### `schemaVersion(): Promise<string>`

Returns the Schema.org vocabulary version compiled into this build.

## Supported Google Rich Results Profiles

| Type | Required Fields | Key Nested Validations |
|---|---|---|
| Product | name | Offer (price, currency), AggregateRating, Review |
| Article | headline, image, datePublished, author | Author (name), Publisher (name) |
| FAQPage | mainEntity | Question (name, acceptedAnswer), Answer (text) |
| BreadcrumbList | itemListElement | ListItem (position, name, item) |
| LocalBusiness | name, address | PostalAddress (street, locality, postal, country) |
| Event | name, startDate, location | Place (name, address), VirtualLocation (url) |
| Recipe | name, image | HowToStep (text) |

## Building from Source

```bash
# Prerequisites
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
brew install binaryen

# Build both targets
./scripts/build-wasm.sh

# Build specific target
./scripts/build-wasm.sh web    # browser ESM
./scripts/build-wasm.sh node   # Node.js
```

## License

MIT
