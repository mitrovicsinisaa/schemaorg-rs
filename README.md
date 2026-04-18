# schemaorg-rs

**Rust library for extracting and validating Schema.org structured data.**

Extract JSON-LD, Microdata, and RDFa markup from HTML into a unified data model.
Future milestones will add vocabulary validation against the official Schema.org
definitions and Google Rich Results deployment profiles.

---

## Quick start

```rust
use schemaorg_rs::extract_all;

let html = r#"<html><head>
<script type="application/ld+json">{
  "@context": "https://schema.org",
  "@type": "Product",
  "name": "Widget",
  "offers": {
    "@type": "Offer",
    "price": 29.99,
    "priceCurrency": "EUR"
  }
}</script>
</head></html>"#;

let graph = extract_all(html).unwrap();
assert_eq!(graph.nodes[0].types, vec!["Product"]);
```

---

## Current status (Milestone 1)

The extraction engine is complete and audited. It parses all three Schema.org
embedding formats from raw HTML into a shared `StructuredDataGraph`:

- **JSON-LD** -- `<script type="application/ld+json">` tags, including `@graph`
  arrays, `@id` cross-reference resolution, and nested objects
- **Microdata** -- `itemscope`/`itemprop` attributes, including `itemref`,
  `itemid`, and value extraction by element type
- **RDFa Lite 1.1** -- `vocab`/`typeof`/`property` attributes, including
  `prefix` namespace mappings and `resource` identifiers

All three extractors produce the same `SchemaNode` / `SchemaValue` data model,
so downstream code does not need to care which format the data came from.

### What works today

| Feature | Status |
|---------|--------|
| JSON-LD extraction | Done |
| Microdata extraction | Done |
| RDFa Lite extraction | Done |
| Unified `extract_all()` | Done |
| `@id` cross-reference resolution | Done |
| Source location tracking (line/column/byte) | Done (JSON-LD) |
| Depth-limited recursion (DoS protection) | Done |
| Feature-gated compilation | Done |
| 81 tests (unit + integration + doc-tests) | Passing |

---

## Installation

```toml
[dependencies]
schemaorg-rs = "0.1"
```

Requires Rust 1.75+.

### Feature flags

| Flag | Default | Description |
|------|---------|-------------|
| `extraction` | Yes | HTML parsing and structured data extraction |
| `validation` | No | Schema.org vocabulary validation (M2) |
| `profiles` | No | Google Rich Results profiles (M3) |
| `wasm` | No | WASM/`wasm-bindgen` support (M3) |
| `cli` | No | CLI binary (M4) |
| `full` | No | `extraction` + `validation` + `profiles` |

To use only the core types without HTML parsing:

```toml
[dependencies]
schemaorg-rs = { version = "0.1", default-features = false }
```

---

## Usage

### Extract all formats at once

```rust
use schemaorg_rs::{extract_all, SourceFormat};

let graph = extract_all(html)?;

for node in &graph.nodes {
    println!("{:?}: {:?}", node.source_format, node.types);
}

for warning in &graph.warnings {
    println!("Warning [{}]: {}", warning.code, warning.message);
}
```

### Use a specific extractor

```rust
use schemaorg_rs::{Extractor, JsonLdExtractor};

let output = JsonLdExtractor.extract(html)?;
assert_eq!(output.nodes[0].types, vec!["Product"]);
```

### Pre-parse HTML for multiple extractors

```rust
use schemaorg_rs::{Html, JsonLdExtractor, MicrodataExtractor};

let document = Html::parse_document(html);

let jsonld = JsonLdExtractor.extract_from_document(&document, html)?;
let microdata = MicrodataExtractor.extract_from_document(&document)?;
```

---

## The problem

Schema.org structured data is embedded in hundreds of millions of web pages.
When it's broken -- a missing `name` on a `Product`, a wrong value type on
`offers.price` -- search engines silently ignore it. No rich results. No AI
citations. No visibility.

The only validators that understand Schema.org semantically are:

- **Google Rich Results Test** -- closed-source, no API, sends your URLs to Google
- **validator.schema.org** -- hosted on Google infrastructure, not self-hostable

In the Rust ecosystem, nothing exists. `json-ld` handles format transformation.
`jsonschema` handles generic JSON Schema. Neither knows what a `Product` requires
to qualify for rich results.

`schemaorg-rs` fills this gap.

---

## Roadmap

### Milestone 1 -- HTML extraction engine (done)

JSON-LD, Microdata, and RDFa parsers. Unified `StructuredDataGraph`
representation. Full test coverage. Audited for memory safety and performance.

### Milestone 2 -- Schema.org vocabulary validation

Code-generation pipeline from official schema.org definitions. Type,
property, value type, and enum validation. Structured error output.

### Milestone 3 -- Rich Results profiles + WASM

Google Rich Results profiles for 7 schema types. Rich result eligibility
verdict. WASM build and npm package.

### Milestone 4 -- CLI + docs + ecosystem

`schemaorg-validate` CLI with SARIF output. Full docs.rs documentation.
GitHub Actions marketplace integration.

### Target Rich Results profiles (M3)

| Type            | Status      |
|-----------------|-------------|
| Product         | Milestone 3 |
| Article         | Milestone 3 |
| FAQPage         | Milestone 3 |
| BreadcrumbList  | Milestone 3 |
| LocalBusiness   | Milestone 3 |
| Event           | Milestone 3 |
| Recipe          | Milestone 3 |

---

## Why Rust?

- Embeddable as a native dependency in other Rust projects
- Compiles to WASM for use in any JavaScript environment
- FFI bindings for Python, Ruby, PHP -- anywhere Schema.org validation is needed
- Fast enough to validate thousands of pages per second in a crawler

---

## Contributing

This project is in early development. Issues and discussions welcome.

Once Milestone 2 is complete, a contributing guide will be published.
All contributions must be licensed under MIT.

---

## License

MIT -- see [LICENSE](LICENSE)
