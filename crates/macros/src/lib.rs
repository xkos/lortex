//! lortex-macros: Lortex 框架过程宏
//!
//! 提供 `#[tool]` 属性宏，可以从普通 async 函数自动生成实现 Tool trait 的结构体。

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, FnArg, ItemFn, Pat};

/// Attribute macro to define a tool from an async function.
///
/// # Example
///
/// ```rust,ignore
/// #[tool(
///     name = "read_file",
///     description = "Read the contents of a file at the given path."
/// )]
/// async fn read_file(path: String) -> Result<String, ToolError> {
///     tokio::fs::read_to_string(&path)
///         .await
///         .map_err(|e| ToolError::ExecutionFailed(e.to_string()))
/// }
/// ```
#[proc_macro_attribute]
pub fn tool(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let attr_args: proc_macro2::TokenStream = attr.into();

    // Parse attributes
    let mut tool_name: Option<String> = None;
    let mut tool_description: Option<String> = None;

    // Simple attribute parsing: tool(name = "...", description = "...")
    let attr_str = attr_args.to_string();
    for segment in attr_str.split(',') {
        let segment = segment.trim();
        if let Some(val) = segment.strip_prefix("name") {
            let val = val.trim().trim_start_matches('=').trim();
            let val = val.trim_matches('"');
            tool_name = Some(val.to_string());
        } else if let Some(val) = segment.strip_prefix("description") {
            let val = val.trim().trim_start_matches('=').trim();
            let val = val.trim_matches('"');
            tool_description = Some(val.to_string());
        }
    }

    let fn_name = &input_fn.sig.ident;
    let fn_name_str = fn_name.to_string();
    let tool_name = tool_name.unwrap_or_else(|| fn_name_str.clone());
    let tool_description = tool_description.unwrap_or_else(|| format!("Tool: {}", fn_name_str));

    // Generate struct name from function name (PascalCase)
    let struct_name = syn::Ident::new(
        &to_pascal_case(&fn_name_str),
        fn_name.span(),
    );

    // Extract parameters for JSON Schema generation
    let params: Vec<_> = input_fn
        .sig
        .inputs
        .iter()
        .filter_map(|arg| {
            if let FnArg::Typed(pat_type) = arg {
                if let Pat::Ident(ident) = &*pat_type.pat {
                    let name = ident.ident.to_string();
                    let ty = &pat_type.ty;
                    let ty_str = quote!(#ty).to_string();
                    return Some((name, ty_str));
                }
            }
            None
        })
        .collect();

    // Build JSON Schema properties
    let schema_properties: Vec<proc_macro2::TokenStream> = params
        .iter()
        .map(|(name, ty)| {
            let json_type = rust_type_to_json_type(ty);
            quote! {
                properties.insert(
                    #name.to_string(),
                    serde_json::json!({ "type": #json_type }),
                );
                required.push(serde_json::Value::String(#name.to_string()));
            }
        })
        .collect();

    // Build the argument extraction
    let arg_extractions: Vec<proc_macro2::TokenStream> = params
        .iter()
        .map(|(name, ty)| {
            let ident = syn::Ident::new(name, proc_macro2::Span::call_site());
            let deser = match ty.as_str() {
                "String" => quote! {
                    let #ident: String = args.get(#name)
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .ok_or_else(|| lortex_core::error::ToolError::InvalidArguments(
                            format!("Missing or invalid argument: {}", #name)
                        ))?;
                },
                "bool" => quote! {
                    let #ident: bool = args.get(#name)
                        .and_then(|v| v.as_bool())
                        .ok_or_else(|| lortex_core::error::ToolError::InvalidArguments(
                            format!("Missing or invalid argument: {}", #name)
                        ))?;
                },
                "i64" | "i32" => quote! {
                    let #ident: i64 = args.get(#name)
                        .and_then(|v| v.as_i64())
                        .ok_or_else(|| lortex_core::error::ToolError::InvalidArguments(
                            format!("Missing or invalid argument: {}", #name)
                        ))?;
                },
                "f64" | "f32" => quote! {
                    let #ident: f64 = args.get(#name)
                        .and_then(|v| v.as_f64())
                        .ok_or_else(|| lortex_core::error::ToolError::InvalidArguments(
                            format!("Missing or invalid argument: {}", #name)
                        ))?;
                },
                _ => quote! {
                    let #ident = serde_json::from_value(
                        args.get(#name).cloned().unwrap_or(serde_json::Value::Null)
                    ).map_err(|e| lortex_core::error::ToolError::InvalidArguments(
                        format!("Invalid argument {}: {}", #name, e)
                    ))?;
                },
            };
            deser
        })
        .collect();

    let arg_names: Vec<syn::Ident> = params
        .iter()
        .map(|(name, _)| syn::Ident::new(name, proc_macro2::Span::call_site()))
        .collect();

    let output = quote! {
        // Keep the original function
        #input_fn

        /// Auto-generated tool struct for `#fn_name`.
        pub struct #struct_name;

        #[async_trait::async_trait]
        impl lortex_core::tool::Tool for #struct_name {
            fn name(&self) -> &str {
                #tool_name
            }

            fn description(&self) -> &str {
                #tool_description
            }

            fn parameters_schema(&self) -> serde_json::Value {
                let mut properties = serde_json::Map::new();
                let mut required = Vec::new();
                #(#schema_properties)*
                serde_json::json!({
                    "type": "object",
                    "properties": serde_json::Value::Object(properties),
                    "required": required,
                })
            }

            async fn execute(
                &self,
                args: serde_json::Value,
                _ctx: &lortex_core::tool::ToolContext,
            ) -> Result<lortex_core::tool::ToolOutput, lortex_core::error::ToolError> {
                #(#arg_extractions)*
                let result = #fn_name(#(#arg_names),*).await?;
                Ok(lortex_core::tool::ToolOutput::text(result.to_string()))
            }
        }
    };

    output.into()
}

fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
            }
        })
        .collect()
}

fn rust_type_to_json_type(ty: &str) -> &'static str {
    match ty.trim() {
        "String" | "&str" | "str" => "string",
        "bool" => "boolean",
        "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "isize" | "usize" => {
            "integer"
        }
        "f32" | "f64" => "number",
        _ => "object",
    }
}
