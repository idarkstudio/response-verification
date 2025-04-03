# Servir archivos estáticos a través de HTTP

Esta guía presenta un proyecto de ejemplo que demuestra cómo crear un canister
que pueda servir archivos estáticos certificados (HTML, CSS, JS) a través de
HTTP. El proyecto de ejemplo es una aplicación de una sola página en JavaScript
muy sencilla. Los archivos se incrustan en el canister cuando se compila.

Esta no es una guía para principiantes sobre el desarrollo de canisters. Se
omitirán muchos conceptos fundamentales que un desarrollador con experiencia en
canisters ya debería conocer. Aquí se explicarán conceptos específicos de la
certificación de archivos, lo que puede ayudar a comprender el
[código de ejemplo completo](https://github.com/dfinity/response-verification/tree/main/examples/http-certification/assets).

La certificación y el servicio de archivos se basan en la biblioteca de alto
nivel
[`ic-asset-certification`](https://crates.io/crates/ic-asset-certification).

Si se necesita más flexibilidad de la que proporciona esta biblioteca, se puede
utilizar la de bajo nivel
[`ic-http-certification`](https://crates.io/crates/ic-http-certification).
Asegúrate de revisar las guías
["Custom HTTP Canisters"](https://internetcomputer.org/docs/current/developer-docs/http-compatible-canisters/custom-http-canisters)
y
["Custom asset canisters"](https://internetcomputer.org/docs/current/developer-docs/web-apps/http-compatible-canisters/serving-static-assets-over-http)
para aprender más sobre cómo usar esa biblioteca para servir archivos.

## Los archivos del frontend

El proyecto frontend utilizado en este ejemplo es un proyecto inicial simple
generado con `npx degit solidjs/templates/ts my-app`. Los únicos cambios que se
han realizado están en el archivo `vite.config.ts`. Se agregó el plugin
`vite-plugin-compression` y se configuró para generar archivos codificados en
Gzip y Brotli, además de los archivos originales. La configuración `ext` afecta
la extensión de los archivos y es importante mantenerla consistente con el
código del canister en el backend, que se verá más adelante en esta guía.

```ts
import { defineConfig } from 'vite';
import solidPlugin from 'vite-plugin-solid';

// Importar el plugin de compresión
import viteCompressionPlugin from 'vite-plugin-compression';

export default defineConfig({
  plugins: [
    solidPlugin(),

    // Configurar la compresión Gzip
    viteCompressionPlugin({
      algorithm: 'gzip',
      // Esta extensión será referenciada más adelante en el código del canister
      ext: '.gz',
      // Asegurar que no se eliminen los archivos originales
      deleteOriginFile: false,
      threshold: 0,
    }),

    // Configurar la compresión Brotli
    viteCompressionPlugin({
      algorithm: 'brotliCompress',
      // Esta extensión será referenciada más adelante en el código del canister
      ext: '.br',
      // Asegurar que no se eliminen los archivos originales
      deleteOriginFile: false,
      threshold: 0,
    }),
  ],
  server: {
    port: 3000,
  },
  build: {
    target: 'esnext',
  },
});
```

El resto de esta guía abordará el código del canister.

## Hooks del ciclo de vida

Lo primero que se debe hacer cuando el canister se inicia por primera vez es
certificar todos los archivos. Esto se realiza en el hook `init`. La función
`certify_all_assets` se explicará en una sección posterior.

La certificación de archivos no se almacena en memoria estable, por lo que es
necesario volver a certificar los archivos después de una actualización del
canister. Esto se realiza en el hook `post_upgrade`.

```rust
#[init]
fn init() {
    certify_all_assets();
}

#[post_upgrade]
fn post_upgrade() {
    init();
}
```

## Endpoints del canister

Este ejemplo tiene un solo endpoint en el canister para servir archivos: el
endpoint de consulta `http_request`. El manejador `http_request` usa dos
funciones auxiliares, `serve_metrics` y `serve_asset`, que se explicarán más
adelante.

```rust
#[query]
fn http_request(req: HttpRequest) -> HttpResponse {
    let path = req.get_path().expect("Failed to parse request path");

    // Si la solicitud es para el endpoint de métricas, servir las métricas
    if path == "/metrics" {
        return serve_metrics();
    }

    // De lo contrario, servir el archivo solicitado
    serve_asset(&req)
}
```

## Carga de archivos

Los archivos se incrustan en el Wasm del canister en el momento de la
compilación. Esto se logra utilizando la biblioteca
[`include_dir`](https://michael-f-bryan.github.io/include_dir/include_dir/index.html).
Cabe señalar que esto funciona bien para un número pequeño de archivos, pero un
número mayor de archivos puede causar tiempos de compilación más largos, como se
menciona en la
[documentación de la biblioteca](https://michael-f-bryan.github.io/include_dir/include_dir/index.html#compile-time-considerations).

Los archivos se importan desde el directorio de compilación del frontend:

```rust
static ASSETS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../frontend/dist");
```

Una vez cargados los archivos, es necesario convertirlos al tipo `Asset` que usa
la biblioteca `ic-asset-certification`.

```rust
/// Recopila recursivamente todos los archivos del directorio proporcionado
fn collect_assets<'content, 'path>(
    dir: &'content Dir<'path>,
    assets: &mut Vec<Asset<'content, 'path>>,
) {
    for file in dir.files() {
        assets.push(Asset::new(file.path().to_string_lossy(), file.contents()));
    }

    for dir in dir.dirs() {
        collect_assets(dir, assets);
    }
}
```

## Certificación de archivos

La certificación de archivos se configura usando el tipo `AssetConfig`. Este
tipo se usa para especificar el tipo de contenido, los encabezados y cualquier
fallback para cada archivo.

Para manejar encabezados comunes, se usa una función auxiliar
`get_asset_headers`. Los encabezados de seguridad añadidos a las respuestas se
basan en el proyecto
[OWASP Secure Headers](https://owasp.org/www-project-secure-headers/index.html).

Estos encabezados de seguridad se han incluido como una configuración
razonablemente segura por defecto para la mayoría de las APIs de archivos
estáticos. Sin embargo, es fundamental que los desarrolladores se informen y
tomen decisiones bien fundamentadas según las necesidades de su propio proyecto.

```rust
fn get_asset_headers(additional_headers: Vec<HeaderField>) -> Vec<HeaderField> {
    // Configurar los encabezados predeterminados e incluir los adicionales proporcionados por el llamador
    let mut headers = vec![
        ("strict-transport-security".to_string(), "max-age=31536000; includeSubDomains".to_string()),
        ("x-frame-options".to_string(), "DENY".to_string()),
        ("x-content-type-options".to_string(), "nosniff".to_string()),
        ("content-security-policy".to_string(), "default-src 'self'; img-src 'self' data:; form-action 'self'; object-src 'none'; frame-ancestors 'none'; upgrade-insecure-requests; block-all-mixed-content".to_string()),
        ("referrer-policy".to_string(), "no-referrer".to_string()),
        ("permissions-policy".to_string(), "accelerometer=(),ambient-light-sensor=(),autoplay=(),battery=(),camera=(),display-capture=(),document-domain=(),encrypted-media=(),fullscreen=(),gamepad=(),geolocation=(),gyroscope=(),layout-animations=(self),legacy-image-formats=(self),magnetometer=(),microphone=(),midi=(),oversized-images=(self),payment=(),picture-in-picture=(),publickey-credentials-get=(),speaker-selection=(),sync-xhr=(self),unoptimized-images=(self),unsized-media=(self),usb=(),screen-wake-lock=(),web-share=(),xr-spatial-tracking=()".to_string()),
        ("cross-origin-embedder-policy".to_string(), "require-corp".to_string()),
        ("cross-origin-opener-policy".to_string(), "same-origin".to_string()),
    ];
    headers.extend(additional_headers);

    headers
}
```

Para el archivo `index.html`, se usa la variante `AssetConfig::File` para
configurarlo específicamente. El campo `fallback_for` de esta variante indica
que este archivo es el fallback para todas las rutas que no coincidan
exactamente con un archivo, y el campo `aliased_by` especifica rutas
alternativas que servirán el mismo archivo.

Para los demás archivos, se pueden configurar en bloque utilizando la variante
`AssetConfig::Pattern`, que usa un patrón de glob para coincidir con múltiples
archivos.

La función `certify_all_assets` realiza los siguientes pasos:

1. Definir las configuraciones de certificación de assets.
2. Recopilar todos los assets del directorio de compilación del frontend.
3. Omitir la certificación para el endpoint `/metrics`.
4. Certificar los assets utilizando la función `certify_assets` de la
   biblioteca `ic-asset-certification`.
5. Establecer los datos certificados del canister.

```rust
thread_local! {
    static HTTP_TREE: Rc<RefCell<HttpCertificationTree>> = Default::default();

    static ASSET_ROUTER: RefCell<AssetRouter<'static>> = RefCell::new(AssetRouter::with_tree(HTTP_TREE.with(|tree| tree.clone())));

    // initializing the asset router with an HTTP certification tree is optional.
    // if direct access to the HTTP certification tree is not needed for certifying
    // requests and responses outside of the asset router, then this step can be skipped
    // and the asset router can be initialized like so:
    static ASSET_ROUTER: RefCell<AssetRouter<'static>> = Default::default();
}

const IMMUTABLE_ASSET_CACHE_CONTROL: &str = "public, max-age=31536000, immutable";
const NO_CACHE_ASSET_CACHE_CONTROL: &str = "public, no-cache, no-store";

fn certify_all_assets() {
    // 1. Define the asset certification configurations.
    let encodings = vec![
        AssetEncoding::Brotli.default(),
        AssetEncoding::Gzip.default(),
    ];

    let asset_configs = vec![
        AssetConfig::File {
            path: "index.html".to_string(),
            content_type: Some("text/html".to_string()),
            headers: get_asset_headers(vec![(
                "cache-control".to_string(),
                NO_CACHE_ASSET_CACHE_CONTROL.to_string(),
            )]),
            fallback_for: vec![AssetFallbackConfig {
                scope: "/".to_string(),
                status_code: Some(StatusCode::OK),
            }],
            aliased_by: vec!["/".to_string()],
            encodings: encodings.clone(),
        },
        AssetConfig::Pattern {
            pattern: "**/*.js".to_string(),
            content_type: Some("text/javascript".to_string()),
            headers: get_asset_headers(vec![(
                "cache-control".to_string(),
                IMMUTABLE_ASSET_CACHE_CONTROL.to_string(),
            )]),
            encodings: encodings.clone(),
        },
        AssetConfig::Pattern {
            pattern: "**/*.css".to_string(),
            content_type: Some("text/css".to_string()),
            headers: get_asset_headers(vec![(
                "cache-control".to_string(),
                IMMUTABLE_ASSET_CACHE_CONTROL.to_string(),
            )]),
            encodings,
        },
        AssetConfig::Pattern {
            pattern: "**/*.ico".to_string(),
            content_type: Some("image/x-icon".to_string()),
            headers: get_asset_headers(vec![(
                "cache-control".to_string(),
                IMMUTABLE_ASSET_CACHE_CONTROL.to_string(),
            )]),
            encodings: vec![],
        },
        AssetConfig::Redirect {
            from: "/old-url".to_string(),
            to: "/".to_string(),
            kind: AssetRedirectKind::Permanent,
            headers: get_asset_headers(vec![
                ("content-type".to_string(), "text/plain".to_string()),
                (
                    "cache-control".to_string(),
                    NO_CACHE_ASSET_CACHE_CONTROL.to_string(),
                ),
            ]),
        },
    ];

    // 2. Collect all assets from the frontend build directory.
    let mut assets = Vec::new();
    collect_assets(&ASSETS_DIR, &mut assets);

    // 3. Skip certification for the metrics endpoint.
    HTTP_TREE.with(|tree| {
        let mut tree = tree.borrow_mut();

        let metrics_tree_path = HttpCertificationPath::exact("/metrics");
        let metrics_certification = HttpCertification::skip();
        let metrics_tree_entry =
            HttpCertificationTreeEntry::new(metrics_tree_path, metrics_certification);

        tree.insert(&metrics_tree_entry);
    });

    ASSET_ROUTER.with_borrow_mut(|asset_router| {
        // 4. Certify the assets using the `certify_assets` function from the `ic-asset-certification` crate.
        if let Err(err) = asset_router.certify_assets(assets, asset_configs) {
            ic_cdk::trap(&format!("Failed to certify assets: {}", err));
        }

        // 5. Set the canister's certified data.
        set_certified_data(&asset_router.root_hash());
    });
}
```

## Sirviendo assets

La función `serve_asset` del `AssetRouter` es responsable de servir los assets. Esta función devuelve una `HttpResponse` que puede ser devuelta al llamador.

```rust
fn serve_asset(req: &HttpRequest) -> HttpResponse<'static> {
    ASSET_ROUTER.with_borrow(|asset_router| {
        if let Ok(response) = asset_router.serve_asset(
            &data_certificate().expect("No data certificate available"),
            req,
        ) {
            response
        } else {
            ic_cdk::trap("Failed to serve asset");
        }
    })
}
```

### Sirviendo métricas

La función `serve_metrics` maneja la entrega de métricas. A diferencia de los
assets, las métricas no están certificadas, por lo que su manejo es más
complejo.

Es importante evaluar si omitir la certificación es seguro en cada caso de uso.
En este ejemplo, las métricas no contienen datos sensibles ni afectan la
seguridad del canister, por lo que es aceptable omitir su certificación.

La estructura `Metrics` recopila datos sobre la cantidad de assets, la cantidad de assets de respaldo (`fallback assets`) y el balance de ciclos del canister.
Se utiliza `add_v2_certificate_header` de `ic-http-certification` para agregar
el encabezado `IC-Certificate`. Además, se usa la función `get_asset_headers`
para incluir los mismos encabezados que en las respuestas de assets.

```rust
fn serve_metrics() -> HttpResponse<'static> {
    ASSET_ROUTER.with_borrow(|asset_router| {
        let metrics = Metrics {
            num_assets: asset_router.get_assets().len(),
            num_fallback_assets: asset_router.get_fallback_assets().len(),
            cycle_balance: canister_balance(),
        };
        let body = serde_json::to_vec(&metrics).expect("Failed to serialize metrics");
        let headers = get_asset_headers(vec![
            (
                CERTIFICATE_EXPRESSION_HEADER_NAME.to_string(),
                DefaultCelBuilder::skip_certification().to_string(),
            ),
            ("content-type".to_string(), "application/json".to_string()),
            (
                "cache-control".to_string(),
                NO_CACHE_ASSET_CACHE_CONTROL.to_string(),
            ),
        ]);
        let mut response = HttpResponse::builder()
            .with_status_code(200)
            .with_body(body)
            .with_headers(headers)
            .build();

        HTTP_TREE.with(|tree| {
            let tree = tree.borrow();

            let metrics_tree_path = HttpCertificationPath::exact("/metrics");
            let metrics_certification = HttpCertification::skip();
            let metrics_tree_entry =
                HttpCertificationTreeEntry::new(&metrics_tree_path, metrics_certification);
            add_v2_certificate_header(
                &data_certificate().expect("No data certificate available"),
                &mut response,
                &tree.witness(&metrics_tree_entry, "/metrics").unwrap(),
                &metrics_tree_path.to_expr_path(),
            );

            response
        })
    })
}
```

---

### Probar el canister

Este ejemplo usa un canister llamado `http_certification_assets_backend`.

Para probarlo, primero inicia una instancia local de la réplica de Internet
Computer con
[`dfx`](https://internetcomputer.org/docs/current/developer-docs/getting-started/install):

```sh
dfx start --background --clean
```

Luego, despliega el canister:

```sh
dfx deploy http_certification_assets_backend
```

Ahora puedes acceder a los assets del canister visitando la siguiente URL en un
navegador web:

```sh
echo "http://$(dfx canister id http_certification_assets_backend).localhost:$(dfx info webserver-port)"
```

También puedes hacer una petición con `curl`:

```sh
curl "http://$(dfx canister id http_certification_assets_backend).localhost:$(dfx info webserver-port)" \
     --resolve "$(dfx canister id http_certification_assets_backend).localhost:$(dfx info webserver-port):127.0.0.1"
```

---

### Recursos adicionales

- [Código fuente del ejemplo](https://github.com/dfinity/response-verification/tree/main/examples/http-certification/assets).
- [`ic-asset-certification` crate](https://crates.io/crates/ic-asset-certification).
- [Documentación de `ic-asset-certification`](https://docs.rs/ic-asset-certification/latest/ic_asset_certification).
- [Código fuente de `ic-asset-certification`](https://github.com/dfinity/response-verification/tree/main/packages/ic-asset-certification).
