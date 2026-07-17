//! Parser — extracts public function docs from a Soroban contract source file
//! using `syn`. Collects `#[contractimpl]` impl blocks and reads `///` doc
//! comments, function signatures, and parameter lists.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use syn::{
    parse_file, Attribute, FnArg, ImplItem, Item, Pat, ReturnType, Type,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamDoc {
    pub name: String,
    pub ty: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDoc {
    pub name: String,
    pub doc: String,
    pub params: Vec<ParamDoc>,
    pub returns: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorVariant {
    pub name: String,
    pub code: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractDoc {
    pub contract: String,
    pub functions: Vec<FunctionDoc>,
    pub errors: Vec<ErrorVariant>,
}

/// Parse `source` and return a `ContractDoc`.
pub fn parse_contract(source: &str, contract_name: &str) -> Result<ContractDoc> {
    let ast = parse_file(source)?;

    let mut functions = Vec::new();
    let mut errors = Vec::new();

    for item in &ast.items {
        match item {
            // Collect error enum variants from `#[contracterror]` enums
            Item::Enum(e) if has_attr(&e.attrs, "contracterror") => {
                for variant in &e.variants {
                    let code = variant
                        .discriminant
                        .as_ref()
                        .and_then(|(_, expr)| {
                            if let syn::Expr::Lit(syn::ExprLit {
                                lit: syn::Lit::Int(i), ..
                            }) = expr
                            {
                                i.base10_parse::<u32>().ok()
                            } else {
                                None
                            }
                        })
                        .unwrap_or(0);
                    errors.push(ErrorVariant {
                        name: variant.ident.to_string(),
                        code,
                    });
                }
            }

            // Collect public functions from `#[contractimpl]` impl blocks
            Item::Impl(impl_block) if has_attr(&impl_block.attrs, "contractimpl") => {
                for impl_item in &impl_block.items {
                    if let ImplItem::Fn(method) = impl_item {
                        // Only document `pub` functions
                        if !matches!(method.vis, syn::Visibility::Public(_)) {
                            continue;
                        }
                        let name = method.sig.ident.to_string();
                        let doc = extract_doc(&method.attrs);

                        // Parameters — skip `env: Env` (Soroban boilerplate)
                        let params = method
                            .sig
                            .inputs
                            .iter()
                            .filter_map(|arg| {
                                if let FnArg::Typed(pat_type) = arg {
                                    let param_name = if let Pat::Ident(pi) = &*pat_type.pat {
                                        pi.ident.to_string()
                                    } else {
                                        "_".to_string()
                                    };
                                    if param_name == "env" {
                                        return None;
                                    }
                                    let ty = type_to_string(&pat_type.ty);
                                    Some(ParamDoc { name: param_name, ty })
                                } else {
                                    None
                                }
                            })
                            .collect();

                        let returns = match &method.sig.output {
                            ReturnType::Default => "()".to_string(),
                            ReturnType::Type(_, ty) => type_to_string(ty),
                        };

                        functions.push(FunctionDoc { name, doc, params, returns });
                    }
                }
            }
            _ => {}
        }
    }

    Ok(ContractDoc {
        contract: contract_name.to_string(),
        functions,
        errors,
    })
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn has_attr(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().any(|a| a.path().is_ident(name))
}

fn extract_doc(attrs: &[Attribute]) -> String {
    attrs
        .iter()
        .filter_map(|a| {
            if !a.path().is_ident("doc") {
                return None;
            }
            if let syn::Meta::NameValue(nv) = &a.meta {
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(s), ..
                }) = &nv.value
                {
                    return Some(s.value().trim().to_string());
                }
            }
            None
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn type_to_string(ty: &Type) -> String {
    // Use quote to render the type back to a token stream, then clean it up.
    let ts = quote::quote!(#ty).to_string();
    // quote adds spaces around punctuation — collapse them for readability
    ts.replace(" < ", "<")
        .replace(" > ", ">")
        .replace(" , ", ", ")
        .replace("< ", "<")
        .replace(" >", ">")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
        use soroban_sdk::{Env, Address};

        #[contracterror]
        pub enum MyError {
            BadAmount = 1,
            Unauthorized = 2,
        }

        pub struct MyContract;

        #[contractimpl]
        impl MyContract {
            /// Initialize the contract.
            pub fn init(env: Env, admin: Address) {}

            /// Send a tip to creator.
            ///
            /// Emits a tip event.
            pub fn tip(env: Env, sender: Address, creator: Address, amount: i128) {}

            /// Get total tips.
            pub fn get_total(env: Env, creator: Address) -> i128 { 0 }

            // private — should be excluded
            fn internal(env: Env) {}
        }
    "#;

    #[test]
    fn parses_public_functions() {
        let doc = parse_contract(SAMPLE, "MyContract").unwrap();
        let names: Vec<_> = doc.functions.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"init"));
        assert!(names.contains(&"tip"));
        assert!(names.contains(&"get_total"));
        // private fn excluded
        assert!(!names.contains(&"internal"));
    }

    #[test]
    fn skips_env_parameter() {
        let doc = parse_contract(SAMPLE, "MyContract").unwrap();
        let tip = doc.functions.iter().find(|f| f.name == "tip").unwrap();
        assert!(tip.params.iter().all(|p| p.name != "env"));
        assert_eq!(tip.params.len(), 3); // sender, creator, amount
    }

    #[test]
    fn extracts_doc_comments() {
        let doc = parse_contract(SAMPLE, "MyContract").unwrap();
        let tip = doc.functions.iter().find(|f| f.name == "tip").unwrap();
        assert!(tip.doc.contains("Send a tip"));
        assert!(tip.doc.contains("Emits a tip event"));
    }

    #[test]
    fn captures_return_type() {
        let doc = parse_contract(SAMPLE, "MyContract").unwrap();
        let get = doc.functions.iter().find(|f| f.name == "get_total").unwrap();
        assert_eq!(get.returns, "i128");
    }

    #[test]
    fn parses_error_variants() {
        let doc = parse_contract(SAMPLE, "MyContract").unwrap();
        assert_eq!(doc.errors.len(), 2);
        let bad = doc.errors.iter().find(|e| e.name == "BadAmount").unwrap();
        assert_eq!(bad.code, 1);
        let unauth = doc.errors.iter().find(|e| e.name == "Unauthorized").unwrap();
        assert_eq!(unauth.code, 2);
    }
}
