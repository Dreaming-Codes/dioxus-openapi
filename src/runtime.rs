//! Server-only OpenAPI assembly + Scalar HTML helper.

use utoipa::openapi::{
    info::InfoBuilder,
    path::{PathItem, PathItemBuilder, PathsBuilder},
    schema::ComponentsBuilder,
    tag::TagBuilder,
    OpenApi, OpenApiBuilder,
};

/// One documented endpoint, registered by [`crate::api_get`] / [`crate::api_post`].
///
/// `append` folds the path into a [`PathsBuilder`]; `schemas` contributes
/// component schemas referenced by that path (request bodies, responses).
pub struct RegisteredPath {
    pub append: fn(PathsBuilder) -> PathsBuilder,
    pub schemas: fn(
        &mut Vec<(
            String,
            utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>,
        )>,
    ),
}

inventory::collect!(RegisteredPath);

/// Tag metadata for the top-level OpenAPI `tags` array (sidebar descriptions).
#[derive(Debug, Clone, Copy)]
pub struct TagMeta {
    pub name: &'static str,
    pub description: Option<&'static str>,
}

/// Document-level OpenAPI info + optional tag descriptions.
#[derive(Debug, Clone, Copy)]
pub struct SpecOptions {
    pub title: &'static str,
    pub version: &'static str,
    pub description: Option<&'static str>,
    pub tags: &'static [TagMeta],
}

/// Like [`PathsBuilder::path_from`], but also copies tags from
/// [`utoipa::__dev::Tags`] onto the operation.
///
/// utoipa stores path tags on a separate trait; the `OpenApi` derive merges
/// them when listing `paths(...)`. Inventory registration must do the same
/// or Scalar dumps every operation under the first sidebar tag.
pub fn append_tagged_path<P>(paths: PathsBuilder) -> PathsBuilder
where
    P: utoipa::Path,
    P: for<'t> utoipa::__dev::Tags<'t>,
{
    let mut operation = P::operation();
    let tags: Vec<String> = P::tags().into_iter().map(str::to_string).collect();
    if !tags.is_empty() {
        operation.tags = Some(tags);
    }

    let methods = P::methods();
    let path_item = if methods.len() == 1 {
        PathItem::new(
            methods
                .into_iter()
                .next()
                .expect("path must declare at least one method"),
            operation,
        )
    } else {
        methods
            .into_iter()
            .fold(PathItemBuilder::new(), |item, method| {
                item.operation(method, operation.clone())
            })
            .build()
    };

    paths.path(P::path(), path_item)
}

/// Build the full OpenAPI document from all inventory-registered paths.
pub fn build_openapi(opts: &SpecOptions) -> OpenApi {
    let mut paths = PathsBuilder::new();
    let mut schema_list: Vec<(
        String,
        utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>,
    )> = Vec::new();

    for reg in inventory::iter::<RegisteredPath> {
        paths = (reg.append)(paths);
        (reg.schemas)(&mut schema_list);
    }

    // Deduplicate schemas by name (same type referenced from many paths).
    let mut seen = std::collections::HashSet::new();
    schema_list.retain(|(name, _)| seen.insert(name.clone()));

    let mut info = InfoBuilder::new()
        .title(opts.title)
        .version(opts.version);
    if let Some(desc) = opts.description {
        info = info.description(Some(desc));
    }

    let tags: Vec<_> = opts
        .tags
        .iter()
        .map(|t| {
            let mut b = TagBuilder::new().name(t.name);
            if let Some(d) = t.description {
                b = b.description(Some(d));
            }
            b.build()
        })
        .collect();

    OpenApiBuilder::new()
        .info(info.build())
        .paths(paths.build())
        .components(Some(
            ComponentsBuilder::new()
                .schemas_from_iter(schema_list)
                .build(),
        ))
        .tags(if tags.is_empty() { None } else { Some(tags) })
        .build()
}

/// Options for the Scalar interactive API reference HTML shell.
#[derive(Debug, Clone)]
pub struct ScalarOptions<'a> {
    pub title: &'a str,
    /// URL the browser fetches for the OpenAPI JSON (e.g. `/api/openapi.json`).
    pub openapi_url: &'a str,
    /// Extra CSS injected into the page (and as Scalar `customCss`).
    /// Typically theme tokens (`include_str!` of Tailwind) + a small bridge.
    pub custom_css: &'a str,
    pub dark_mode: bool,
    pub hide_agent: bool,
    pub hide_mcp: bool,
    pub hide_client_button: bool,
    pub hide_clients: bool,
}

impl Default for ScalarOptions<'static> {
    fn default() -> Self {
        Self {
            title: "API Reference",
            openapi_url: "/api/openapi.json",
            custom_css: "",
            dark_mode: true,
            hide_agent: true,
            hide_mcp: true,
            hide_client_button: true,
            hide_clients: true,
        }
    }
}

/// Build a standalone Scalar HTML document for `GET /api/docs` (or similar).
pub fn scalar_html(opts: &ScalarOptions<'_>) -> String {
    let css_for_js = opts
        .custom_css
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace("${", "\\${");

    let dark_class = if opts.dark_mode { "dark-mode" } else { "" };
    let color_scheme = if opts.dark_mode { "dark" } else { "light" };

    format!(
        r#"<!doctype html>
<html class="{dark_class}">
<head>
    <title>{title}</title>
    <meta charset="utf-8"/>
    <meta name="viewport" content="width=device-width, initial-scale=1"/>
    <meta name="color-scheme" content="{color_scheme}"/>
    <style>{css}</style>
</head>
<body>
    <div id="app"></div>
    <script src="https://cdn.jsdelivr.net/npm/@scalar/api-reference"></script>
    <script>
        Scalar.createApiReference('#app', {{
            url: '{openapi_url}',
            theme: 'none',
            darkMode: {dark_mode},
            forceDarkModeState: '{force_dark}',
            hideDarkModeToggle: {hide_toggle},
            withDefaultFonts: false,
            agent: {{ disabled: {hide_agent} }},
            mcp: {{ disabled: {hide_mcp} }},
            hideClientButton: {hide_client_button},
            hiddenClients: {hide_clients},
            customCss: `{css_js}`,
        }});
    </script>
</body>
</html>
"#,
        dark_class = dark_class,
        title = opts.title,
        color_scheme = color_scheme,
        css = opts.custom_css,
        openapi_url = opts.openapi_url,
        dark_mode = opts.dark_mode,
        force_dark = if opts.dark_mode { "dark" } else { "light" },
        hide_toggle = opts.dark_mode,
        hide_agent = opts.hide_agent,
        hide_mcp = opts.hide_mcp,
        hide_client_button = opts.hide_client_button,
        hide_clients = opts.hide_clients,
        css_js = css_for_js,
    )
}
