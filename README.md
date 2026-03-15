# schemaorg-rs

**Rust library for parsing and validating Schema.org structured data.**

Validates JSON-LD, Microdata, and RDFa markup embedded in HTML against the
official Schema.org vocabulary and real-world deployment profiles (Google Rich
Results, Bing, and more).

---

## The problem

Schema.org structured data is embedded in hundreds of millions of web pages.
When it's broken — a missing `name` on a `Product`, a wrong value type on
`offers.price` — search engines silently ignore it. No rich results. No AI
citations. No visibility.

The only validators that understand Schema.org semantically are:

- **Google Rich Results Test** — closed-source, no API, sends your URLs to Google
- **validator.schema.org** — hosted on Google infrastructure, not self-hostable

In the Rust ecosystem, nothing exists. `json-ld` handles format transformation.
`jsonschema` handles generic JSON Schema. Neither knows what a `Product` requires
to qualify for rich results.

`schemaorg-rs` fills this gap.

---

## What it does

```rust
use schemaorg_rs::{validate, Profile};

let html = r#"
  <script type="application/ld+json">
  {
    "@context": "https://schema.org",
    "@type": "Product",
    "name": "Fjord T-Shirt",
    "offers": {
      "@type": "Offer",
      "price": "29.99",
      "priceCurrency": "EUR"
    }
  }
  </script>
"#;

let result = validate(html, Profile::GoogleRichResults)?;

println!("Eligible: {}", result.rich_result_eligible);
// Eligible: true

for warning in &result.warnings {
    println!("Warning: {}", warning);
}
// Warning: Product.image — recommended for rich results, missing
```

---

## Features

- **Multi-format extraction** — JSON-LD, Microdata, RDFa from raw HTML
- **Schema.org vocabulary validation** — types, properties, value types, enums;
  rules auto-generated from official schema.org machine-readable definitions
- **Deployment profiles** — pluggable rule sets for Google Rich Results, Bing,
  and a generic schema.org baseline
- **Rich result eligibility verdict** — `eligible`, `not_eligible`, or
  `warnings_only` per Google's public documentation
- **WASM build** — use from Node.js, browser tooling, or serverless environments
- **CLI tool** — `schemaorg-validate --url https://example.com --profile google`
  with SARIF output for CI/CD integration

---

## Supported Rich Results profiles

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

## Installation

```toml
[dependencies]
schemaorg-rs = "0.1"
```

Requires Rust 1.75+.

For Node.js (WASM):
```sh
npm install schemaorg-rs
```

---

## CLI

```sh
cargo install schemaorg-validate

# Validate a URL
schemaorg-validate --url https://example.com --profile google

# Validate a local file
schemaorg-validate --file product.html --profile google --output sarif

# Use in GitHub Actions
schemaorg-validate --url $URL --output sarif > results.sarif
```

---

## Roadmap

### Milestone 1 — HTML extraction engine
JSON-LD, Microdata, and RDFa parsers. Unified `StructuredDataGraph`
representation. W3C test suite compliance.

### Milestone 2 — Schema.org vocabulary validation
Code-generation pipeline from official schema.org definitions. Type,
property, value type, and enum validation. Structured error output.

### Milestone 3 — Rich Results profiles + WASM
Google Rich Results profiles for 7 schema types. Rich result eligibility
verdict. WASM build and npm package.

### Milestone 4 — CLI + docs + ecosystem
`schemaorg-validate` CLI with SARIF output. Full docs.rs documentation.
GitHub Actions marketplace integration. Shopware/TYPO3 proof-of-concept.

---

## Why Rust?

- Embeddable as a native dependency in other Rust projects
- Compiles to WASM for use in any JavaScript environment
- FFI bindings for Python, Ruby, PHP — anywhere Schema.org validation is needed
- Fast enough to validate thousands of pages per second in a crawler

---

## Contributing

This project is in early development. Issues and discussions welcome.

Once the first milestone is complete, a contributing guide will be published.
All contributions must be licensed under MIT.

---

## License

MIT — see [LICENSE](LICENSE)

---

