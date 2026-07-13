use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    Expr, FnArg, GenericArgument, ItemFn, LitBool, LitStr, Meta, Pat, PathArguments, ReturnType,
    Token, Type,
};

/// `#[api_get("/path", tag = "...", ...)]`
#[proc_macro_attribute]
pub fn api_get(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand("get", attr, item)
}

/// `#[api_post("/path", tag = "...", ...)]`
#[proc_macro_attribute]
pub fn api_post(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand("post", attr, item)
}

fn expand(method: &str, attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = syn::parse_macro_input!(attr as ApiArgs);
    let mut func = syn::parse_macro_input!(item as ItemFn);

    if args.openapi == Some(false) {
        let dioxus_attr = dioxus_route_attr(method, &args.path);
        return quote! {
            #dioxus_attr
            #func
        }
        .into();
    }

    let tag = match &args.tag {
        Some(t) => t.clone(),
        None => {
            return syn::Error::new_spanned(
                &func.sig.ident,
                "api_get/api_post require `tag = \"...\"` (or `openapi = false`)",
            )
            .to_compile_error()
            .into();
        }
    };

    let path_lit = &args.path;
    let path_value = path_lit.value();
    let fn_ident = func.sig.ident.clone();
    let openapi_fn = format_ident!("__openapi_{}", fn_ident);
    let body_struct = format_ident!("__openapi_body_{}", fn_ident);
    let operation_id = fn_ident.to_string();

    let path_params = parse_path_params(&path_value);
    let arg_map = fn_arg_map(&func);

    // Path param entries for utoipa.
    let mut param_tokens = Vec::new();
    for name in &path_params {
        let Some((_, (ty, _))) = arg_map.iter().find(|(n, _)| n == name) else {
            return syn::Error::new_spanned(
                &func.sig.ident,
                format!("path parameter `{{{name}}}` has no matching function argument"),
            )
            .to_compile_error()
            .into();
        };
        let name_lit = LitStr::new(name, proc_macro2::Span::call_site());
        param_tokens.push(quote! {
            (#name_lit = #ty, Path, description = #name_lit)
        });
    }

    // Body args: non-path, non-receiver typed args (order matches Dioxus packing).
    let body_fields: Vec<_> = arg_map
        .iter()
        .filter(|(name, _)| !path_params.iter().any(|p| p == name))
        .map(|(name, (ty, _))| {
            let ident = format_ident!("{}", name);
            (ident, ty.clone())
        })
        .collect();

    let has_body = method == "post" && !body_fields.is_empty() && args.request_body.is_none();
    let request_body_override = args.request_body.clone();

    let response_ty = match &args.response {
        Some(ty) => Some(ty.clone()),
        None => match extract_result_ok(&func.sig.output) {
            Ok(ty) => ty,
            Err(e) => return e.to_compile_error().into(),
        },
    };

    let dioxus_attr = dioxus_route_attr(method, path_lit);
    let method_ident = format_ident!("{}", method);

    // Doc comments on the real fn — copy onto the openapi stub so utoipa
    // picks up summary/description.
    let docs: Vec<_> = func
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("doc"))
        .cloned()
        .collect();

    // Strip nothing else; leave user attrs on the real function.
    let _ = &mut func;

    let body_struct_tokens = if has_body {
        let fields = body_fields.iter().map(|(ident, ty)| {
            quote! { pub #ident: #ty }
        });
        quote! {
            #[cfg(feature = "server")]
            #[derive(utoipa::ToSchema)]
            #[allow(non_camel_case_types)]
            #[doc(hidden)]
            pub struct #body_struct {
                #(#fields,)*
            }
        }
    } else {
        quote! {}
    };

    let request_body_tokens = if let Some(ty) = request_body_override {
        quote! { request_body = #ty, }
    } else if has_body {
        quote! { request_body = #body_struct, }
    } else {
        quote! {}
    };

    let params_tokens = if param_tokens.is_empty() {
        quote! {}
    } else {
        quote! { params( #(#param_tokens),* ), }
    };

    let responses_tokens = match response_ty {
        None => quote! {
            responses(
                (status = 200, description = "OK"),
                (status = 500, description = "Server error")
            )
        },
        Some(Type::Tuple(t)) if t.elems.is_empty() => quote! {
            responses(
                (status = 200, description = "OK"),
                (status = 500, description = "Server error")
            )
        },
        Some(ty) => quote! {
            responses(
                (status = 200, description = "OK", body = #ty),
                (status = 500, description = "Server error")
            )
        },
    };

    // utoipa names the Path impl struct `__path_{fn_name}`.
    let path_type = format_ident!("__path_{}", openapi_fn);

    quote! {
        #dioxus_attr
        #func

        #body_struct_tokens

        #[cfg(feature = "server")]
        #(#docs)*
        #[utoipa::path(
            #method_ident,
            path = #path_lit,
            operation_id = #operation_id,
            tag = #tag,
            #params_tokens
            #request_body_tokens
            #responses_tokens
        )]
        #[doc(hidden)]
        pub fn #openapi_fn() {}

        // Auto-register with the dioxus-openapi inventory so apps do not need
        // a hand-maintained paths(...) / schemas(...) list. Uses
        // append_tagged_path so utoipa Tags land on the operation.
        #[cfg(feature = "server")]
        ::dioxus_openapi::inventory::submit! {
            ::dioxus_openapi::RegisteredPath {
                append: |paths| ::dioxus_openapi::append_tagged_path::<#path_type>(paths),
                schemas: |schemas| {
                    <#path_type as ::utoipa::__dev::SchemaReferences>::schemas(schemas);
                },
            }
        }
    }
    .into()
}

fn dioxus_route_attr(method: &str, path: &LitStr) -> proc_macro2::TokenStream {
    match method {
        "get" => quote! { #[::dioxus::prelude::get(#path)] },
        "post" => quote! { #[::dioxus::prelude::post(#path)] },
        "put" => quote! { #[::dioxus::prelude::put(#path)] },
        "delete" => quote! { #[::dioxus::prelude::delete(#path)] },
        "patch" => quote! { #[::dioxus::prelude::patch(#path)] },
        other => {
            let msg = format!("unsupported method {other}");
            quote! { compile_error!(#msg) }
        }
    }
}

struct ApiArgs {
    path: LitStr,
    tag: Option<LitStr>,
    request_body: Option<Type>,
    response: Option<Type>,
    openapi: Option<bool>,
}

impl Parse for ApiArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let path: LitStr = input.parse()?;
        let mut tag = None;
        let mut request_body = None;
        let mut response = None;
        let mut openapi = None;

        if input.peek(Token![,]) {
            let _ = input.parse::<Token![,]>()?;
            let punct: Punctuated<Meta, Token![,]> = Punctuated::parse_terminated(input)?;
            for meta in punct {
                match meta {
                    Meta::NameValue(nv) if nv.path.is_ident("tag") => {
                        tag = Some(expr_to_lit_str(&nv.value)?);
                    }
                    Meta::NameValue(nv) if nv.path.is_ident("request_body") => {
                        request_body = Some(expr_to_type(&nv.value)?);
                    }
                    Meta::NameValue(nv) if nv.path.is_ident("response") => {
                        response = Some(expr_to_type(&nv.value)?);
                    }
                    Meta::NameValue(nv) if nv.path.is_ident("openapi") => {
                        openapi = Some(expr_to_bool(&nv.value)?);
                    }
                    other => {
                        return Err(syn::Error::new_spanned(
                            other,
                            "expected tag / request_body / response / openapi",
                        ));
                    }
                }
            }
        }

        Ok(Self {
            path,
            tag,
            request_body,
            response,
            openapi,
        })
    }
}

fn expr_to_lit_str(expr: &Expr) -> syn::Result<LitStr> {
    match expr {
        Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(s),
            ..
        }) => Ok(s.clone()),
        _ => Err(syn::Error::new_spanned(expr, "expected string literal")),
    }
}

fn expr_to_bool(expr: &Expr) -> syn::Result<bool> {
    match expr {
        Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Bool(LitBool { value, .. }),
            ..
        }) => Ok(*value),
        _ => Err(syn::Error::new_spanned(expr, "expected bool literal")),
    }
}

fn expr_to_type(expr: &Expr) -> syn::Result<Type> {
    // `response = Vec<Mac>` comes through as an expression path / etc.
    // Re-parse the token stream as a Type.
    syn::parse2(quote! { #expr })
}

/// `{serial}` / `{role}` captures in an axum-style path.
fn parse_path_params(path: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = path;
    while let Some(start) = rest.find('{') {
        rest = &rest[start + 1..];
        let Some(end) = rest.find('}') else { break };
        let name = &rest[..end];
        // Skip wildcards like `{*path}` if ever used.
        let name = name.strip_prefix('*').unwrap_or(name);
        if !name.is_empty() {
            out.push(name.to_string());
        }
        rest = &rest[end + 1..];
    }
    out
}

/// Map arg name → (type, original index). Skips receivers.
fn fn_arg_map(func: &ItemFn) -> Vec<(String, (Type, usize))> {
    // Use Vec to preserve order (important for body field order matching Dioxus).
    let mut out = Vec::new();
    for (i, arg) in func.sig.inputs.iter().enumerate() {
        let FnArg::Typed(pat_ty) = arg else {
            continue;
        };
        let Pat::Ident(pat_ident) = pat_ty.pat.as_ref() else {
            continue;
        };
        out.push((
            pat_ident.ident.to_string(),
            ((*pat_ty.ty).clone(), i),
        ));
    }
    out
}

/// `Result<T>` / `Result<T, E>` → `Some(T)`; bare non-Result is error for now.
fn extract_result_ok(ret: &ReturnType) -> syn::Result<Option<Type>> {
    let ReturnType::Type(_, ty) = ret else {
        return Ok(None);
    };
    let Type::Path(type_path) = ty.as_ref() else {
        return Ok(Some(ty.as_ref().clone()));
    };
    let last = type_path
        .path
        .segments
        .last()
        .ok_or_else(|| syn::Error::new_spanned(ty, "empty return type path"))?;
    if last.ident != "Result" {
        return Ok(Some(ty.as_ref().clone()));
    }
    let PathArguments::AngleBracketed(args) = &last.arguments else {
        return Ok(None);
    };
    let mut generics = args.args.iter().filter_map(|a| match a {
        GenericArgument::Type(t) => Some(t.clone()),
        _ => None,
    });
    Ok(generics.next())
}
