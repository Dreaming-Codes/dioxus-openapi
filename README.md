# dioxus-openapi

[![Crates.io](https://img.shields.io/crates/v/dioxus-openapi.svg)](https://crates.io/crates/dioxus-openapi)
[![Docs.rs](https://docs.rs/dioxus-openapi/badge.svg)](https://docs.rs/dioxus-openapi)

OpenAPI ([utoipa](https://crates.io/crates/utoipa)) for **Dioxus 0.7 server functions**, without retyping paths.

## What it does

| Piece | Role |
|-------|------|
| `#[api_get]` / `#[api_post]` | Expand to Dioxus `#[get]`/`#[post]` **and** a utoipa path stub |
| Inventory registration | Paths/schemas collected automatically (no manual `paths(...)` list) |
| `build_openapi` | Assemble OpenAPI 3.x from registered paths |
| `scalar_html` | Standalone Scalar UI shell (Agent/MCP/client off by default) |

POST bodies that are not path params become a generated `ToSchema` struct matching Dioxus’s **named JSON object** packing. Operation tags are applied correctly for Scalar’s sidebar.

## Install

```toml
[dependencies]
dioxus-openapi = "0.1"
utoipa = { version = "5", optional = true }   # if you derive ToSchema on models

[features]
server = [
    "dioxus/server",
    "dioxus-openapi/server",
    "dep:utoipa",
]
```

## Usage

```rust
use dioxus_openapi::{api_get, api_post, build_openapi, SpecOptions, TagMeta};

#[api_get("/api/hello", tag = "demo")]
pub async fn hello() -> Result<String> {
    Ok("hi".into())
}

const SPEC: SpecOptions = SpecOptions {
    title: "My API",
    version: "0.1.0",
    description: Some("…"),
    tags: &[TagMeta {
        name: "demo",
        description: Some("Demo routes"),
    }],
};

// Mount as plain axum routes (do not Router::merge a different state into Dioxus):
// .route("/api/openapi.json", get(|| async { Json(build_openapi(&SPEC)) }))
// .route("/api/docs", get(|| async { Html(scalar_html(&ScalarOptions { … })) }))
```

Wire types used in responses should derive `utoipa::ToSchema` on the server:

```rust
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Mac { /* … */ }
```
