//! OpenAPI for Dioxus server functions.
//!
//! Pair [`api_get`] / [`api_post`] with [`build_openapi`] so paths never need
//! to be listed twice: the macros expand to Dioxus `#[get]`/`#[post]` **and**
//! inventory-register a utoipa path (with tags applied correctly for Scalar).
//!
//! # App setup
//!
//! ```toml
//! [dependencies]
//! dioxus-openapi = "0.1"
//!
//! [features]
//! server = ["dioxus/server", "dioxus-openapi/server"]
//! ```
//!
//! ```rust,ignore
//! use dioxus_openapi::{api_get, api_post, build_openapi, SpecOptions, TagMeta};
//!
//! #[api_get("/api/hello", tag = "demo")]
//! pub async fn hello() -> Result<String> { Ok("hi".into()) }
//!
//! const SPEC: SpecOptions = SpecOptions {
//!     title: "My API",
//!     version: "0.1.0",
//!     description: Some("…"),
//!     tags: &[TagMeta { name: "demo", description: Some("Demo routes") }],
//! };
//!
//! // GET /api/openapi.json
//! async fn openapi_json() -> axum::Json<utoipa::openapi::OpenApi> {
//!     axum::Json(build_openapi(&SPEC))
//! }
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(not(feature = "server"), allow(unused_imports))]

pub use dioxus_openapi_macros::{api_get, api_post};

#[cfg(feature = "server")]
mod runtime;

#[cfg(feature = "server")]
#[cfg_attr(docsrs, doc(cfg(feature = "server")))]
pub use runtime::{
    append_tagged_path, build_openapi, scalar_html, RegisteredPath, ScalarOptions, SpecOptions,
    TagMeta,
};

/// Re-export so macro expansions can `::dioxus_openapi::inventory::submit!`
/// without the app depending on `inventory` directly.
#[cfg(feature = "server")]
#[cfg_attr(docsrs, doc(cfg(feature = "server")))]
#[doc(hidden)]
pub use inventory;
