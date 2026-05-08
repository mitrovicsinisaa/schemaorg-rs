//! Build script: generates Schema.org vocabulary lookup code from the vendored
//! `schemaorg-all-https.jsonld` file.
//!
//! This runs at compile time and produces `generated.rs` in `$OUT_DIR`,
//! which is included by `src/vocabulary/mod.rs`.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::env;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::path::Path;

use serde_json::Value;

/// Schema.org URL prefixes to strip from @id values.
const SCHEMA_PREFIXES: &[&str] = &["https://schema.org/", "http://schema.org/", "schema:"];

/// DataType names that are value constraints, not real types.
const DATA_TYPES: &[&str] = &[
    "Text",
    "Number",
    "Boolean",
    "Date",
    "DateTime",
    "Time",
    "URL",
    "Integer",
    "Float",
    "CssSelectorType",
    "PronounceableText",
    "XPathType",
];

fn main() {
    println!("cargo:rerun-if-changed=schema-data/schemaorg-all-https.jsonld");
    println!("cargo:rerun-if-changed=schema-data/SCHEMA_VERSION");

    let vocab_path = Path::new("schema-data/schemaorg-all-https.jsonld");
    if !vocab_path.exists() {
        // If the vocabulary file doesn't exist, skip code generation.
        // This allows `cargo build --no-default-features` to work without the file.
        return;
    }

    let vocab_json: Value = serde_json::from_str(
        &fs::read_to_string(vocab_path)
            .expect("failed to read schema-data/schemaorg-all-https.jsonld"),
    )
    .expect("failed to parse schema-data/schemaorg-all-https.jsonld as JSON");

    let graph = vocab_json["@graph"]
        .as_array()
        .expect("@graph must be an array in schemaorg-all-https.jsonld");

    let version = fs::read_to_string("schema-data/SCHEMA_VERSION")
        .unwrap_or_else(|_| "unknown".into())
        .trim()
        .to_string();

    // Phase 1: classify all entries
    let mut raw_types: BTreeMap<String, RawType> = BTreeMap::new();
    let mut raw_properties: BTreeMap<String, RawProperty> = BTreeMap::new();
    let mut enum_type_names: HashSet<String> = HashSet::new();
    let mut raw_enum_members: Vec<RawEnumMember> = Vec::new();

    // First pass: identify all classes and find which are enumerations
    for entry in graph {
        let entry_type = get_type(entry);
        let id = get_id(entry);
        if id.is_empty() {
            continue;
        }
        let name = strip_prefix(&id);
        if name.is_empty() {
            continue;
        }

        match entry_type.as_str() {
            "rdfs:Class" => {
                let parents = get_parents(entry);
                let is_enum = is_enumeration(&parents, &name);
                let is_pending = is_part_of(entry, "pending.schema.org");
                let is_attic = is_part_of(entry, "attic.schema.org");
                let comment = get_comment(entry);

                if is_enum {
                    enum_type_names.insert(name.clone());
                }

                // Skip DataTypes  --  they're not real types
                if DATA_TYPES.contains(&name.as_str()) {
                    continue;
                }

                raw_types.insert(
                    name,
                    RawType {
                        parent_names: parents,
                        own_properties: BTreeSet::new(),
                        comment,
                        is_pending,
                        is_attic,
                    },
                );
            }
            "rdf:Property" => {
                let domain = get_includes(entry, "schema:domainIncludes");
                let range = get_includes(entry, "schema:rangeIncludes");
                let (is_superseded, superseded_by) = get_superseded(entry);
                let is_pending = is_part_of(entry, "pending.schema.org");

                raw_properties.insert(
                    name,
                    RawProperty {
                        domain_types: domain,
                        expected_types: range,
                        is_superseded,
                        superseded_by,
                        is_pending,
                    },
                );
            }
            _ => {
                // Might be an enum member (e.g., @type: "schema:ItemAvailability")
                let type_local = strip_prefix(&entry_type);
                if !type_local.is_empty()
                    && type_local != "rdfs:Class"
                    && type_local != "rdf:Property"
                {
                    raw_enum_members.push(RawEnumMember {
                        name,
                        enum_type: type_local,
                    });
                }
            }
        }
    }

    // Phase 2: assign properties to types based on domainIncludes
    for (prop_name, prop) in &raw_properties {
        for domain_type in &prop.domain_types {
            if let Some(t) = raw_types.get_mut(domain_type) {
                t.own_properties.insert(prop_name.clone());
            }
        }
    }

    // Phase 3: resolve inheritance  --  compute all_properties for each type
    let all_properties = resolve_all_properties(&raw_types);

    // Phase 4: filter enum members to only those whose enum_type is known
    let valid_enum_members: Vec<&RawEnumMember> = raw_enum_members
        .iter()
        .filter(|m| enum_type_names.contains(&m.enum_type) || raw_types.contains_key(&m.enum_type))
        .collect();

    // Phase 5: generate code
    let generated = generate_code(
        &raw_types,
        &all_properties,
        &raw_properties,
        &valid_enum_members,
        &version,
    );

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let dest = Path::new(&out_dir).join("generated.rs");
    fs::write(&dest, generated).expect("failed to write generated.rs");
}

// Data structures
struct RawType {
    parent_names: Vec<String>,
    own_properties: BTreeSet<String>,
    comment: String,
    is_pending: bool,
    is_attic: bool,
}

struct RawProperty {
    domain_types: Vec<String>,
    expected_types: Vec<String>,
    is_superseded: bool,
    superseded_by: Option<String>,
    is_pending: bool,
}

struct RawEnumMember {
    name: String,
    enum_type: String,
}

// JSON helpers
fn get_type(entry: &Value) -> String {
    match &entry["@type"] {
        Value::String(s) => s.clone(),
        Value::Array(arr) => arr
            .first()
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    }
}

fn get_id(entry: &Value) -> String {
    entry["@id"].as_str().unwrap_or("").to_string()
}

fn get_comment(entry: &Value) -> String {
    entry["rdfs:comment"].as_str().unwrap_or("").to_string()
}

fn get_parents(entry: &Value) -> Vec<String> {
    get_id_list(entry, "rdfs:subClassOf")
}

fn get_includes(entry: &Value, key: &str) -> Vec<String> {
    get_id_list(entry, key)
}

/// Extracts a list of @id values from a field that can be a single object or array.
fn get_id_list(entry: &Value, key: &str) -> Vec<String> {
    match &entry[key] {
        Value::Object(obj) => {
            if let Some(Value::String(id)) = obj.get("@id") {
                vec![strip_prefix(id)]
            } else {
                vec![]
            }
        }
        Value::Array(arr) => arr
            .iter()
            .filter_map(|v| v["@id"].as_str())
            .map(strip_prefix)
            .collect(),
        _ => vec![],
    }
}

fn get_superseded(entry: &Value) -> (bool, Option<String>) {
    match &entry["schema:supersededBy"] {
        Value::Object(obj) => {
            let by = obj.get("@id").and_then(|v| v.as_str()).map(strip_prefix);
            (true, by)
        }
        Value::Null => (false, None),
        _ => (false, None),
    }
}

fn is_part_of(entry: &Value, substring: &str) -> bool {
    match &entry["schema:isPartOf"] {
        Value::Object(obj) => obj
            .get("@id")
            .and_then(|v| v.as_str())
            .is_some_and(|s| s.contains(substring)),
        _ => false,
    }
}

fn is_enumeration(parents: &[String], _name: &str) -> bool {
    parents
        .iter()
        .any(|p| p == "Enumeration" || p.ends_with("Enumeration"))
}

fn strip_prefix(s: &str) -> String {
    for prefix in SCHEMA_PREFIXES {
        if let Some(stripped) = s.strip_prefix(prefix) {
            return stripped.to_string();
        }
    }
    s.to_string()
}

// Inheritance resolution
/// Resolves the full set of properties for each type (own + inherited).
/// Returns a map of type_name -> sorted list of all property names.
fn resolve_all_properties(raw_types: &BTreeMap<String, RawType>) -> HashMap<String, Vec<String>> {
    let mut cache: HashMap<String, Vec<String>> = HashMap::new();

    for type_name in raw_types.keys() {
        if !cache.contains_key(type_name) {
            let mut visited = HashSet::new();
            let props = collect_all_props(type_name, raw_types, &mut cache, &mut visited);
            cache.insert(type_name.clone(), props);
        }
    }

    cache
}

/// Recursively collects all properties for a type including inherited ones.
/// Uses a visited set for cycle detection.
fn collect_all_props(
    type_name: &str,
    raw_types: &BTreeMap<String, RawType>,
    cache: &mut HashMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
) -> Vec<String> {
    if let Some(cached) = cache.get(type_name) {
        return cached.clone();
    }

    if !visited.insert(type_name.to_string()) {
        // Cycle detected  --  return empty to break the cycle
        eprintln!("cargo:warning=Cycle detected in Schema.org type hierarchy at: {type_name}");
        return Vec::new();
    }

    let Some(raw) = raw_types.get(type_name) else {
        return Vec::new();
    };

    let mut all: BTreeSet<String> = raw.own_properties.clone();

    for parent in &raw.parent_names {
        let parent_props = collect_all_props(parent, raw_types, cache, visited);
        all.extend(parent_props);
    }

    let result: Vec<String> = all.into_iter().collect(); // BTreeSet is already sorted
    cache.insert(type_name.to_string(), result.clone());
    result
}

// Code generation
fn generate_code(
    types: &BTreeMap<String, RawType>,
    all_properties: &HashMap<String, Vec<String>>,
    properties: &BTreeMap<String, RawProperty>,
    enum_members: &[&RawEnumMember],
    version: &str,
) -> String {
    let mut code = String::with_capacity(2 * 1024 * 1024); // ~2MB pre-allocation
    writeln!(
        code,
        "// AUTO-GENERATED by build.rs from schemaorg-all-https.jsonld v{version}"
    )
    .unwrap();
    writeln!(
        code,
        "// Do not edit manually. Re-run `cargo build` to regenerate."
    )
    .unwrap();
    writeln!(code).unwrap();

    // Type definitions
    for (name, raw) in types {
        let var_name = to_const_name("TYPE", name);
        let parent_list = static_str_slice(
            &raw.parent_names
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>(),
        );
        let own_props = static_str_slice(
            &raw.own_properties
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>(),
        );
        let all_props_vec = all_properties.get(name).cloned().unwrap_or_default();
        let all_props =
            static_str_slice(&all_props_vec.iter().map(|s| s.as_str()).collect::<Vec<_>>());
        let comment = escape_string(&raw.comment);

        writeln!(code, "static {var_name}: TypeDef = TypeDef {{").unwrap();
        writeln!(code, "    name: \"{name}\",").unwrap();
        writeln!(code, "    parent_types: {parent_list},").unwrap();
        writeln!(code, "    own_properties: {own_props},").unwrap();
        writeln!(code, "    all_properties: {all_props},").unwrap();
        writeln!(code, "    comment: \"{comment}\",").unwrap();
        writeln!(code, "    is_pending: {},", raw.is_pending).unwrap();
        writeln!(code, "    is_attic: {},", raw.is_attic).unwrap();
        writeln!(code, "}};").unwrap();
        writeln!(code).unwrap();
    }

    // Property definitions
    for (name, raw) in properties {
        let var_name = to_const_name("PROP", name);
        let expected = static_str_slice(
            &raw.expected_types
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>(),
        );
        let domain = static_str_slice(
            &raw.domain_types
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>(),
        );
        let superseded_str = match &raw.superseded_by {
            Some(s) => format!("Some(\"{s}\")"),
            None => "None".to_string(),
        };

        writeln!(code, "static {var_name}: PropertyDef = PropertyDef {{").unwrap();
        writeln!(code, "    name: \"{name}\",").unwrap();
        writeln!(code, "    expected_types: {expected},").unwrap();
        writeln!(code, "    domain_types: {domain},").unwrap();
        writeln!(code, "    is_superseded: {},", raw.is_superseded).unwrap();
        writeln!(code, "    superseded_by: {superseded_str},").unwrap();
        writeln!(code, "    is_pending: {},", raw.is_pending).unwrap();
        writeln!(code, "}};").unwrap();
        writeln!(code).unwrap();
    }

    // Enum member definitions
    for member in enum_members {
        let var_name = to_const_name("ENUM", &member.name);
        writeln!(code, "static {var_name}: EnumMemberDef = EnumMemberDef {{").unwrap();
        writeln!(code, "    name: \"{}\",", member.name).unwrap();
        writeln!(code, "    enum_type: \"{}\",", member.enum_type).unwrap();
        writeln!(code, "}};").unwrap();
        writeln!(code).unwrap();
    }

    // lookup_type()
    writeln!(code, "/// Looks up a Schema.org type by name.").unwrap();
    writeln!(
        code,
        "pub fn lookup_type(name: &str) -> Option<&'static TypeDef> {{"
    )
    .unwrap();
    writeln!(code, "    match name {{").unwrap();
    for name in types.keys() {
        let var_name = to_const_name("TYPE", name);
        writeln!(code, "        \"{name}\" => Some(&{var_name}),").unwrap();
    }
    writeln!(code, "        _ => None,").unwrap();
    writeln!(code, "    }}").unwrap();
    writeln!(code, "}}").unwrap();
    writeln!(code).unwrap();

    // lookup_property()
    writeln!(code, "/// Looks up a Schema.org property by name.").unwrap();
    writeln!(
        code,
        "pub fn lookup_property(name: &str) -> Option<&'static PropertyDef> {{"
    )
    .unwrap();
    writeln!(code, "    match name {{").unwrap();
    for name in properties.keys() {
        let var_name = to_const_name("PROP", name);
        writeln!(code, "        \"{name}\" => Some(&{var_name}),").unwrap();
    }
    writeln!(code, "        _ => None,").unwrap();
    writeln!(code, "    }}").unwrap();
    writeln!(code, "}}").unwrap();
    writeln!(code).unwrap();

    // lookup_enum_member()
    writeln!(
        code,
        "/// Looks up a Schema.org enumeration member by name."
    )
    .unwrap();
    writeln!(
        code,
        "pub fn lookup_enum_member(name: &str) -> Option<&'static EnumMemberDef> {{"
    )
    .unwrap();
    writeln!(code, "    match name {{").unwrap();
    for member in enum_members {
        let var_name = to_const_name("ENUM", &member.name);
        writeln!(code, "        \"{}\" => Some(&{var_name}),", member.name).unwrap();
    }
    writeln!(code, "        _ => None,").unwrap();
    writeln!(code, "    }}").unwrap();
    writeln!(code, "}}").unwrap();
    writeln!(code).unwrap();

    // schema_version()
    writeln!(code, "/// Returns the vendored Schema.org version.").unwrap();
    writeln!(
        code,
        "pub fn schema_version() -> &'static str {{ \"{version}\" }}"
    )
    .unwrap();
    writeln!(code).unwrap();

    // ALL_TYPE_NAMES / ALL_PROPERTY_NAMES
    let type_names_str = static_str_slice(&types.keys().map(|s| s.as_str()).collect::<Vec<_>>());
    writeln!(
        code,
        "/// All known Schema.org type names, sorted alphabetically."
    )
    .unwrap();
    writeln!(
        code,
        "pub(crate) static ALL_TYPE_NAMES: &[&str] = {type_names_str};"
    )
    .unwrap();
    writeln!(code).unwrap();

    let prop_names_str =
        static_str_slice(&properties.keys().map(|s| s.as_str()).collect::<Vec<_>>());
    writeln!(
        code,
        "/// All known Schema.org property names, sorted alphabetically."
    )
    .unwrap();
    writeln!(
        code,
        "pub(crate) static ALL_PROPERTY_NAMES: &[&str] = {prop_names_str};"
    )
    .unwrap();
    writeln!(code).unwrap();

    code
}

// String helpers
/// Converts a name like "Product" to a Rust const name like "TYPE_PRODUCT".
fn to_const_name(prefix: &str, name: &str) -> String {
    let mut result = String::with_capacity(prefix.len() + name.len() + 10);
    result.push_str(prefix);
    result.push('_');

    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            // Insert underscore before uppercase letters (camelCase -> CAMEL_CASE)
            // but not at the start
            let prev = name.chars().nth(i - 1).unwrap_or('_');
            if prev.is_lowercase() || prev.is_ascii_digit() {
                result.push('_');
            }
        }
        result.push(ch.to_ascii_uppercase());
    }

    // Replace any non-alphanumeric chars with underscores
    result
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Generates a `&[&str]` slice literal.
fn static_str_slice(items: &[&str]) -> String {
    if items.is_empty() {
        return "&[]".to_string();
    }
    let mut s = String::from("&[");
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        write!(s, "\"{}\"", escape_string(item)).unwrap();
    }
    s.push(']');
    s
}

/// Escapes a string for use in a Rust string literal.
fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
