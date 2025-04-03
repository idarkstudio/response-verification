# Servir assets estáticos a través de HTTP (personalizado)

Esta guía muestra un proyecto de ejemplo que demuestra cómo crear un contenedor
que puede servir assets estáticos certificados (HTML, CSS, JS) a través de
HTTP. El proyecto de ejemplo presenta una aplicación JavaScript de una sola
página muy simple. Los assets se incrustan en el contenedor cuando se compila.

Esta no es una guía de desarrollo de contenedores para principiantes. Se
omitirán muchos conceptos fundamentales que un desarrollador de contenedores
relativamente experimentado debería conocer. Los conceptos específicos de la
Certificación HTTP se destacarán aquí y pueden ayudar a comprender el
[ejemplo de código completo](https://github.com/dfinity/response-verification/tree/main/examples/http-certification/custom-assets).

## Requisitos previos

Se recomienda revisar las guías anteriores antes de leer esta. El ejemplo de la
API JSON en particular se referenciará y la guía anterior de assets estáticos
será la más adecuada para la mayoría de los proyectos. El enfoque seguido en
esta guía está mejor adaptado para casos extremos que requieren flexibilidad
adicional.

- [x] Completa la guía
      ["Servir assets estáticos a través de HTTP"](https://internetcomputer.org/docs/current/developer-docs/web-apps/http-compatible-canisters/serving-static-assets-over-http).
- [x] Completa la guía
      ["Contenedores HTTP personalizados"](https://internetcomputer.org/docs/current/developer-docs/http-compatible-canisters/custom-http-canisters).
- [x] Completa la guía
      ["Servir JSON a través de HTTP"](https://internetcomputer.org/docs/current/developer-docs/http-compatible-canisters/serving-json-over-http).

## Los assets del frontend

El proyecto frontend utilizado para este ejemplo es un proyecto de inicio simple
generado con `npx degit solidjs/templates/ts my-app`. Los únicos cambios que se
han realizado están en el archivo `vite.config.ts`. Se agregó y configuró el
complemento `vite-plugin-compression` para generar assets codificados en Gzip y
Brotli, junto con los assets originales. La configuración `ext` afecta la
extensión del archivo y es importante mantenerla consistente con el código del
contenedor backend que se verá más adelante en esta guía.

```ts
import { defineConfig } from 'vite';
import solidPlugin from 'vite-plugin-solid';

// importar el complemento de compresión
import viteCompressionPlugin from 'vite-plugin-compression';

export default defineConfig({
  plugins: [
    solidPlugin(),

    // configurar la compresión Gzip
    viteCompressionPlugin({
      algorithm: 'gzip',
      // esta extensión se referenciará más adelante en el código del contenedor
      ext: '.gzip',
      // asegurarse de no eliminar los archivos originales
      deleteOriginFile: false,
      threshold: 0,
    }),

    // configurar la compresión Brotli
    viteCompressionPlugin({
      algorithm: 'brotliCompress',
      // esta extensión se referenciará más adelante en el código del contenedor
      ext: '.br',
      // asegurarse de no eliminar los archivos originales
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

El resto de esta guía abordará el código del contenedor.

## Ciclo de vida

Los ganchos del ciclo de vida se configuran de manera similar a la API JSON.

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

## Expresiones CEL

La definición de la expresión CEL es más simple en el caso de los assets en
comparación con el
[ejemplo de API JSON](https://internetcomputer.org/docs/current/developer-docs/http-compatible-canisters/serving-json-over-http),
ya que se utiliza la misma expresión CEL para cada activo, incluida la respuesta
de reserva.

```rust
lazy_static! {
    static ref ASSET_CEL_EXPR_DEF: DefaultResponseOnlyCelExpression<'static> =
        DefaultCelBuilder::response_only_certification()
            .with_response_certification(DefaultResponseCertification::response_header_exclusions(
                vec![],
            ))
            .build();
    static ref ASSET_CEL_EXPR: String = ASSET_CEL_EXPR_DEF.to_string();
}
```

## Assets

Los assets se incrustan en el Wasm del contenedor en tiempo de compilación.
Esto se logra utilizando la biblioteca
[`include_dir`](https://michael-f-bryan.github.io/include_dir/include_dir/index.html).
Tenga en cuenta que esto funciona bien para un número pequeño de assets, pero
un mayor número de assets puede provocar tiempos de compilación más largos,
como se menciona en la
[documentación de la biblioteca](https://michael-f-bryan.github.io/include_dir/include_dir/index.html#compile-time-considerations).

Los assets se importan desde el directorio de compilación del frontend:

```rust
static ASSETS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../frontend/dist");
```

Con los assets cargados, de manera similar a la
[API JSON](https://internetcomputer.org/docs/current/developer-docs/http-compatible-canisters/serving-json-over-http),
es necesario almacenar en algún lugar las respuestas y certificaciones
precalculadas. Sin embargo, en este ejemplo se utiliza una estructura
ligeramente diferente.

Los assets codificados se almacenan en un `HashMap` separado para facilitar el
enrutamiento. Esto será más evidente más adelante en esta guía.

```rust
#[derive(Clone)]
struct CertifiedHttpResponse<'a> {
    response: HttpResponse<'a>,
    certification: HttpCertification,
}

thread_local! {
    static RESPONSES: RefCell<HashMap<String, CertifiedHttpResponse<'static>>> = RefCell::new(HashMap::new());
    static ENCODED_RESPONSES: RefCell<HashMap<(String, String), CertifiedHttpResponse<'static>>> = RefCell::new(HashMap::new());
}
```

La certificación de respuestas es más compleja aquí en comparación con el
enfoque más simple utilizado en el
[ejemplo de API JSON](https://internetcomputer.org/docs/current/developer-docs/http-compatible-canisters/serving-json-over-http).
Hay algunas rutas utilizadas en las siguientes funciones que requieren cierta
explicación:

- `asset_tree_path`: la `HttpCertificationPath` que se utilizará para almacenar
  el activo en el árbol, por ejemplo,
  `HttpCertificationPath::exact("/assets/app.js")`.
- `asset_file_path`: la ruta de archivo relativa del activo en el disco antes de
  importarlo al contenedor, por ejemplo, `assets/app.js`.
- `asset_req_path`: la ruta absoluta que se utilizará para solicitar el activo
  `/assets/app.js` desde un navegador.

El primer paso es definir una función reutilizable para crear una respuesta con
todos los encabezados predeterminados necesarios. Esta función es muy similar a
la contraparte en el
[ejemplo de API JSON](https://internetcomputer.org/docs/current/developer-docs/http-compatible-canisters/serving-json-over-http),
con la mayor diferencia en los encabezados que se utilizan. Dado que las
respuestas de una API que sirve assets estáticos se renderizarán directamente
en el navegador, se necesitan encabezados centrados en la seguridad:

```rust
fn get_asset_headers(
    additional_headers: Vec<HeaderField>,
    content_length: usize,
    cel_expr: String,
) -> Vec<(String, String)> {
    // set up the default headers and include additional headers provided by the caller
    let mut headers = vec![
        ("strict-transport-security".to_string(), "max-age=31536000; includeSubDomains".to_string()),
        ("x-frame-options".to_string(), "DENY".to_string()),
        ("x-content-type-options".to_string(), "nosniff".to_string()),
        ("content-security-policy".to_string(), "default-src 'self'; form-action 'self'; object-src 'none'; frame-ancestors 'none'; upgrade-insecure-requests; block-all-mixed-content".to_string()),
        ("referrer-policy".to_string(), "no-referrer".to_string()),
        ("permissions-policy".to_string(), "accelerometer=(),ambient-light-sensor=(),autoplay=(),battery=(),camera=(),display-capture=(),document-domain=(),encrypted-media=(),fullscreen=(),gamepad=(),geolocation=(),gyroscope=(),layout-animations=(self),legacy-image-formats=(self),magnetometer=(),microphone=(),midi=(),oversized-images=(self),payment=(),picture-in-picture=(),publickey-credentials-get=(),speaker-selection=(),sync-xhr=(self),unoptimized-images=(self),unsized-media=(self),usb=(),screen-wake-lock=(),web-share=(),xr-spatial-tracking=()".to_string()),
        ("cross-origin-embedder-policy".to_string(), "require-corp".to_string()),
        ("cross-origin-opener-policy".to_string(), "same-origin".to_string()),
        ("content-length".to_string(), content_length.to_string()),
        (CERTIFICATE_EXPRESSION_HEADER_NAME.to_string(), cel_expr),
    ];
    headers.extend(additional_headers);

    headers
}

fn create_asset_response(
    additional_headers: Vec<HeaderField>,
    body: &[u8],
    cel_expr: String,
) -> HttpResponse {
    let headers = get_asset_headers(additional_headers, body.len(), cel_expr);

    HttpResponse::ok(body, headers).build()
}
```

La siguiente función es una función reutilizable que puede certificar cualquier
activo.

```rust
fn certify_asset_response(
    body: &'static [u8],
    additional_headers: Vec<HeaderField>,
    asset_tree_path: &HttpCertificationPath,
    asset_req_path: String,
) {
    // create the response
    let response = create_asset_response(additional_headers, body, ASSET_CEL_EXPR.clone());

    // certify the response
    let certification =
        HttpCertification::response_only(&ASSET_CEL_EXPR_DEF, &response, None).unwrap();

    HTTP_TREE.with_borrow_mut(|http_tree| {
        // add the certification to the certification tree
        http_tree.insert(&HttpCertificationTreeEntry::new(
            asset_tree_path,
            &certification,
        ));
    });

    RESPONSES.with_borrow_mut(|responses| {
        // store the response for later retrieval
        responses.insert(
            asset_req_path,
            CertifiedHttpResponse {
                response,
                certification,
            },
        );
    });
}
```

A continuación, una función reutilizable para certificar un activo con una
codificación específica. Esta función verificará si existe un archivo con una
extensión de archivo adicional que coincida con la codificación solicitada en el
directorio de assets incluido estáticamente.

Por ejemplo, al certificar `index.html` con la codificación `gzip`, esta función
verificará si existe `index.html.gzip`. Si el activo codificado existe, se
certificará utilizando un procedimiento similar a la función
`certify_asset_response` previamente definida. La diferencia principal en esta
función es dónde se almacena la respuesta del activo codificado.

Esta función fallará silenciosamente si el archivo codificado no existe. Esto es
necesario porque el proyecto frontend contiene assets que no se codificarán.
Las imágenes, por ejemplo, ya están en un formato comprimido, por lo que no se
codifican.

```rust
fn certify_asset_with_encoding(
    asset_file_path: &str,
    asset_tree_path: &HttpCertificationPath,
    asset_req_path: String,
    encoding: &str,
    additional_headers: Vec<HeaderField>,
) {
    // check if the file exists before certifying it
    if let Some(file) = ASSETS_DIR.get_file(format!("{}.{}", asset_file_path, encoding)) {
        let body = file.contents();

        // add the content encoding header
        let mut headers = vec![("content-encoding".to_string(), encoding.to_string())];
        headers.extend(additional_headers);

        // create the response
        let response = create_asset_response(headers, body, ASSET_CEL_EXPR.clone());

        // certify the response
        let certification =
            HttpCertification::response_only(&ASSET_CEL_EXPR_DEF, &response, None).unwrap();

        HTTP_TREE.with_borrow_mut(|http_tree| {
            // add the certification to the certification tree
            http_tree.insert(&HttpCertificationTreeEntry::new(
                asset_tree_path,
                &certification,
            ));
        });

        ENCODED_RESPONSES.with_borrow_mut(|responses| {
            // store the response for later retrieval
            responses.insert(
                (asset_req_path, encoding.to_string()),
                CertifiedHttpResponse {
                    response,
                    certification,
                },
            );
        });
    };
}
```

A continuación, otra función simple que certificará un activo para todas las
codificaciones: Identidad (la original), Gzip y Brotli. Esta función aprovecha
la función `certify_asset_response` para la codificación de Identidad y
`certify_asset_with_encoding` para las otras codificaciones.

```rust
fn certify_asset(
    body: &'static [u8],
    asset_file_path: String,
    asset_tree_path: &HttpCertificationPath,
    asset_req_path: String,
    additional_headers: Vec<HeaderField>,
) {
    certify_asset_response(
        body,
        additional_headers.clone(),
        asset_tree_path,
        asset_req_path.to_string(),
    );
    certify_asset_with_encoding(
        &asset_file_path,
        asset_tree_path,
        asset_req_path.to_string(),
        "gzip",
        additional_headers.clone(),
    );
    certify_asset_with_encoding(
        &asset_file_path,
        asset_tree_path,
        asset_req_path.to_string(),
        "br",
        additional_headers,
    );
}
```

Ahora, una función ligeramente más compleja certifica una serie de assets que
coinciden con un patrón (por ejemplo, `assets/**/*.js`) con un tipo de contenido
(por ejemplo, `text/javascript`).

```rust
fn certify_asset_glob(glob: &str, content_type: &str) {
    // iterate over every asset matching the glob
    for identity_file in ASSETS_DIR
        .find(glob)
        .unwrap()
        .map(|entry| entry.as_file().unwrap())
    {
        // compute the different paths needed for this asset
        let asset_file_path = identity_file.path().to_str().unwrap().to_string();
        let asset_req_path = if !asset_file_path.starts_with("/") {
            format!("/{}", asset_file_path)
        } else {
            asset_file_path.clone()
        };
        let asset_tree_path = HttpCertificationPath::exact(&asset_req_path);

        // add the content-type and cache-control headers
        let additional_headers = vec![
            ("content-type".to_string(), content_type.to_string()),
            (
                "cache-control".to_string(),
                "public, max-age=31536000, immutable".to_string(),
            ),
        ];

        let body = identity_file.contents();
        certify_asset(
            body,
            asset_file_path,
            &asset_tree_path,
            asset_req_path.clone(),
            additional_headers,
        );
    }
}
```

Por último, una función específica para certificar el archivo `index.html`. Dado
que el proyecto frontend es una aplicación de una sola página, cualquier
solicitud que no coincida exactamente con un archivo existente debe redirigirse
a `index.html`, por lo que la certificación se maneja de manera diferente para
este archivo, en particular utilizando `HttpCertificationPath::wildcard()` en
lugar de `HttpCertificationPath::exact()` como la ruta del árbol de
certificación.

Esto permitirá que el contenedor devuelva este archivo para cualquier ruta que
no coincida exactamente con una ruta existente en el árbol. Si el contenedor
intenta devolver este archivo en lugar de una coincidencia exacta que existe, la
verificación fallará.

```rust
lazy_static! {
    static ref INDEX_REQ_PATH: &'static str = "";
    static ref INDEX_TREE_PATH: HttpCertificationPath<'static> = HttpCertificationPath::wildcard(*INDEX_REQ_PATH);
    static ref INDEX_FILE_PATH: &'static str = "index.html";
}

const NO_CACHE_ASSET_CACHE_CONTROL: &str = "public, no-cache, no-store";

fn certify_index_asset() {
    let additional_headers = vec![
        ("content-type".to_string(), "text/html".to_string()),
        (
            "cache-control".to_string(),
            NO_CACHE_ASSET_CACHE_CONTROL.to_string(),
        ),
    ];

    let identity_file = ASSETS_DIR
        .get_file(*INDEX_FILE_PATH)
        .expect("No index.html file found!!!");
    let body = identity_file.contents();

    certify_asset(
        body,
        INDEX_FILE_PATH.to_string(),
        &*INDEX_TREE_PATH,
        INDEX_REQ_PATH.to_string(),
        additional_headers,
    );
}
```

También es posible omitir la certificación para ciertas rutas. Esto puede ser
útil en escenarios donde es difícil predecir cómo será la respuesta para una
determinada ruta y el contenido no es muy sensible a la seguridad. Esto se puede
hacer, por ejemplo, con las métricas servidas en la ruta `/metrics` de la
siguiente manera:

```rust
const METRICS_REQ_PATH: &str = "/metrics";

fn add_certification_skips() {
    let metrics_tree_path = HttpCertificationPath::exact(METRICS_REQ_PATH);
    let metrics_certification = HttpCertification::skip();

    HTTP_TREE.with_borrow_mut(|http_tree| {
        http_tree.insert(&HttpCertificationTreeEntry::new(
            metrics_tree_path,
            &metrics_certification,
        ));
    });
}
```

Después de configurar todas las certificaciones, los [datos certificados](https://internetcomputer.org/docs/current/references/ic-interface-spec#system-api-certified-data) del canister deben establecerse. Esto asegurará que los datos certificados correctos estén configurados para que puedan ser firmados durante la próxima ronda de consenso:

```rust
fn update_certified_data() {
    HTTP_TREE.with_borrow(|http_tree| {
        set_certified_data(&http_tree.root_hash());
    });
}
```

Con todas las funciones anteriores, ahora es posible certificar todos los
assets del proyecto frontend de manera sencilla.

```rust
fn certify_all_assets() {
    add_certification_skips();

    certify_index_asset();
    certify_asset_glob("assets/**/*.css", "text/css");
    certify_asset_glob("assets/**/*.js", "text/javascript");
    certify_asset_glob("assets/**/*.ico", "image/x-icon");
    certify_asset_glob("assets/**/*.svg", "image/svg+xml");

    update_certified_data();
}
```

## Sirviendo assets

Con todos los assets certificados, se pueden servir a través de HTTP. Los pasos
a seguir al servir los assets son:

- Verificar si la ruta de la solicitud coincide con la ruta no certificada.
  - Si la ruta solicitada coincide exactamente con la ruta no certificada,
    servir la respuesta no certificada.
- Verificar si la ruta solicitada coincide con un archivo (por ejemplo,
  `/assets/app.js`).
  - Si la ruta de la solicitud coincide exactamente con un archivo existente,
    servir ese archivo.
  - De lo contrario, servir el archivo `index.html`.
- Extraer el encabezado `content-encoding` de la solicitud.
  - Servir el activo codificado en Brotli si existe y fue solicitado.
  - De lo contrario, servir el activo codificado en Gzip si existe y fue
    solicitado.
  - De lo contrario, servir el activo original.
- Agregar el encabezado de certificación. Este es el mismo proceso que con la
  [API JSON](https://internetcomputer.org/docs/current/developer-docs/http-compatible-canisters/serving-json-over-http).

```rust
fn asset_handler(req: &HttpRequest) -> HttpResponse<'static> {
    let req_path = req.get_path().expect("No se pudo obtener la ruta de la solicitud");

    RESPONSES.with_borrow(|responses| {
        ENCODED_RESPONSES.with_borrow(|encoded_responses| {
            let (asset_req_path, asset_tree_path, identity_response) =
            // si la ruta de la solicitud coincide con la ruta de las métricas, servir esa respuesta no certificada
            if req_path == METRICS_REQ_PATH {
                (
                    METRICS_REQ_PATH.to_string(),
                    HttpCertificationPath::exact(METRICS_REQ_PATH),
                    CertifiedHttpResponse {
                        response: create_metrics_response(),
                        certification: HttpCertification::skip(),
                    },
                )
            }
            // si la ruta solicitada coincide con un activo estático, servir ese activo
            else if let Some(identity_response) = responses.get(&req_path) {
                (
                    req_path.to_string(),
                    HttpCertificationPath::exact(&req_path),
                    identity_response.clone(),
                )
            // de lo contrario, servir el archivo index.html
            } else {
                (
                    INDEX_REQ_PATH.to_string(),
                    INDEX_TREE_PATH.to_owned(),
                    responses.get(*INDEX_REQ_PATH).unwrap().clone(),
                )
            };

            // extraer el encabezado de codificación de contenido
            let content_encoding = req.headers().iter().find_map(|(name, value)| {
                if name.to_lowercase() == "accept-encoding" {
                    Some(value)
                } else {
                    None
                }
            });

            let CertifiedHttpResponse {
                certification,
                response,
            } = content_encoding
                .and_then(|encoding| {
                    // si la solicitud pide Brotli y está disponible para este archivo, servir esa versión
                    if encoding.contains("br") {
                        if let Some(br_response) =
                            encoded_responses.get(&(asset_req_path.clone(), "br".to_string()))
                        {
                            return Some(br_response.clone());
                        }
                    }

                    // si la solicitud pide Gzip y está disponible para este archivo, servir esa versión
                    if encoding.contains("gzip") {
                        if let Some(gzip_response) =
                            encoded_responses.get(&(asset_req_path, "gzip".to_string()))
                        {
                            return Some(gzip_response.clone());
                        }
                    }

                    None
                })
                // de lo contrario, servir la versión original
                .unwrap_or(identity_response);

            let mut response = response.clone();

            HTTP_TREE.with_borrow(|http_tree| {
                add_v2_certificate_header(
                    &data_certificate().expect("No hay certificado de datos disponible"),
                    &mut response,
                    &http_tree
                        .witness(
                            &HttpCertificationTreeEntry::new(&asset_tree_path, certification),
                            &req_path,
                        )
                        .unwrap(),
                    &asset_tree_path.to_expr_path(),
                );
            });

            response
        })
    })
}
```

La creación de la respuesta no certificada se realiza de la siguiente manera:

```rust
fn create_metrics_response() -> HttpResponse<'static> {
    let metrics = Metrics {
        cycle_balance: canister_balance(),
    };
    let body = serde_json::to_vec(&metrics).expect("No se pudo serializar las métricas");
    let additional_headers = vec![
        ("content-type".to_string(), "application/json".to_string()),
        (
            "cache-control".to_string(),
            NO_CACHE_ASSET_CACHE_CONTROL.to_string(),
        ),
    ];
    let headers = get_asset_headers(
        additional_headers,
        body.len(),
        DefaultCelBuilder::skip_certification().to_string(),
    );

    HttpResponse::ok(body, headers).build()
}
```

Recuerda que se omite la verificación para este activo, por lo que la respuesta
no se validará y es posible que el canister (o la réplica) devuelva virtualmente
cualquier cosa, maliciosa o no.

Esta función se puede vincular fácilmente al controlador `http_request`:

```rust
#[query]
fn http_request(req: HttpRequest) -> HttpResponse {
    asset_handler(&req)
}
```

## Probando el canister

Este ejemplo utiliza un canister llamado
`http_certification_custom_assets_backend`.

Para probar el canister, puedes usar
[`dfx`](https://internetcomputer.org/docs/current/developer-docs/getting-started/install)
para iniciar una instancia local de la réplica:

```shell
dfx start --background --clean
```

Luego, implementa el canister:

```shell
dfx deploy http_certification_custom_assets_backend
```

Ahora puedes acceder a los assets del canister navegando a la URL del canister
en un navegador web. La URL también se puede encontrar utilizando el siguiente
comando:

```shell
echo "http://$(dfx canister id http_certification_custom_assets_backend).localhost:$(dfx info webserver-port)"
```

Alternativamente, para hacer una solicitud con `curl`:

```shell
curl "http://$(dfx canister id http_certification_custom_assets_backend).localhost:$(dfx info webserver-port)" --resolve "$(dfx canister id http_certification_custom_assets_backend).localhost:$(dfx info webserver-port):127.0.0.1"
```

## Recursos

- [Código fuente de ejemplo](https://github.com/dfinity/response-verification/tree/main/examples/http-certification/custom-assets).
- Paquete
  [`ic-http-certification`](https://crates.io/crates/ic-http-certification).
- Documentación de
  [`ic-http-certification`](https://docs.rs/ic-http-certification/latest/ic_http_certification).
- Código fuente de
  [`ic-http-certification`](https://github.com/dfinity/response-verification/tree/main/packages/ic-http-certification).
