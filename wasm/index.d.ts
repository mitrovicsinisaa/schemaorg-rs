/**
 * TypeScript definitions for @schemaorg-rs/wasm
 */

// ── Core Types ──────────────────────────────────────────────────────

export interface SchemaNode {
  types: string[];
  properties: Record<string, SchemaValue[]>;
  id?: string;
  source_format: 'JsonLd' | 'Microdata' | 'RdfaLite';
  source_location?: SourceLocation;
}

export type SchemaValue =
  | { Text: string }
  | { Url: string }
  | { Number: number }
  | { Boolean: boolean }
  | { Date: string }
  | { Time: string }
  | { DateTime: string }
  | { Nested: SchemaNode };

export interface SourceLocation {
  line: number;
  column: number;
}

// ── Extraction ──────────────────────────────────────────────────────

export interface ExtractResult {
  nodes: SchemaNode[];
  warnings: ExtractionWarning[];
  error?: string;
}

export interface ExtractionWarning {
  code: string;
  message: string;
  source_location?: SourceLocation;
}

// ── Validation ──────────────────────────────────────────────────────

export interface ValidationDiagnostic {
  path: string;
  severity: 'Error' | 'Warning' | 'Info';
  code: string;
  message: string;
  source_location?: SourceLocation;
}

// ── Profiles ────────────────────────────────────────────────────────

export type Eligibility = 'Eligible' | 'WarningsOnly' | 'NotEligible' | 'Restricted';

export interface TypeEligibility {
  schema_type: string;
  eligible: boolean;
  required_missing: string[];
  recommended_missing: string[];
  field_diagnostics: ValidationDiagnostic[];
}

export interface ProfileResult {
  eligibility: Eligibility;
  type_results: TypeEligibility[];
  diagnostics: ValidationDiagnostic[];
  note?: string;
}

// ── Combined Result ─────────────────────────────────────────────────

export interface ValidateResult {
  extraction: ExtractResult;
  validation: {
    diagnostics: ValidationDiagnostic[];
    has_errors: boolean;
  };
  profile: ProfileResult;
  error?: string;
}

// ── Functions ───────────────────────────────────────────────────────

/**
 * Initialize the WASM module.
 * @param target - 'web' for browser ESM, 'nodejs' for Node.js
 */
export function init(target?: 'web' | 'nodejs'): Promise<void>;

/**
 * Extract structured data from HTML.
 */
export function extract(html: string): Promise<ExtractResult>;

/**
 * Full validation pipeline: extract → validate → profile evaluate.
 */
export function validateHtml(html: string, profile?: string): Promise<ValidateResult>;

/**
 * Returns the Schema.org vocabulary version.
 */
export function schemaVersion(): Promise<string>;
