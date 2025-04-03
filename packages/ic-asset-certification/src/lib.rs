//! # Certificación de assets
//!
//! La certificación de assets es una forma especializada de
//! [certificación HTTP](https://internetcomputer.org/docs/current/developer-docs/http-compatible-canisters/custom-http-canisters),
//! diseñada específicamente para certificar assets estáticos en canisters de [ICP](https://internetcomputer.org/).
//!
//! El crate `ic-asset-certification` proporciona la funcionalidad necesaria para
//! certificar y servir assets estáticos desde canisters en Rust.
//!
//! Esto se implementa en los siguientes pasos:
//!
//! 1. [Preparar los assets](#preparing-assets).
//! 2. [Configurar la certificación de assets](#configuring-asset-certification).
//! 3. [Insertar assets en el enrutador de assets](#inserting-assets-into-the-asset-router).
//! 4. [Servir assets](#serving-assets).
//! 5. [Eliminar assets](#deleting-assets).
//! 6. [Consultar assets](#querying-assets).
//!
//! Para los canisters que lo necesiten, también es posible [eliminar assets](#deleting-assets).
//!
//! ## Preparar los assets
//!
//! Esta biblioteca no impone restricciones sobre el origen de los assets, por lo que
//! este aspecto no se cubre en detalle aquí. Sin embargo, existen tres opciones principales:
//!
//! - Incrustar assets en el canister en tiempo de compilación:
//!   - [include_bytes!](https://doc.rust-lang.org/std/macro.include_bytes.html)
//!   - [include_dir!](https://docs.rs/include_dir/latest/include_dir/index.html)
//! - Subir assets a través de endpoints del canister en tiempo de ejecución:
//!   - El [`dfx` asset canister](https://github.com/dfinity/sdk/blob/master/docs/design/asset-canister-interface.md) es un buen ejemplo de este enfoque.
//! - Generar assets dinámicamente en código, en tiempo de ejecución.
//!
//! Con los assets en memoria, se pueden convertir al tipo [Asset]:
//!
//! ```rust
//! use ic_asset_certification::Asset;
//!
//! let asset = Asset::new(
//!     "index.html",
//!     b"<html><body><h1>¡Hola Mundo!</h1></body></html>".as_slice(),
//! );
//! ```
//!
//! Se recomienda utilizar referencias al incluir assets directamente en el
//! canister para evitar la duplicación de contenido, especialmente para assets grandes.
//!
//! ```rust
//! use ic_asset_certification::Asset;
//!
//! let pretty_big_asset = include_bytes!("lib.rs");
//! let asset = Asset::new(
//!     "assets/pretty-big-asset.gz",
//!     pretty_big_asset.as_slice(),
//! );
//! ```
//!
//! En algunos casos, puede ser necesario usar valores poseídos, como cuando los
//! assets se generan o modifican dinámicamente en tiempo de ejecución.
//!
//! ```rust
//! use ic_asset_certification::Asset;
//!
//! let name = "Mundo";
//! let asset = Asset::new(
//!     "index.html",
//!     format!("<html><body><h1>¡Hola {name}!</h1></body></html>").into_bytes(),
//! );
//! ```
//!
//! ## Configurar la certificación de assets
//!
//! [AssetConfig] define la configuración para cualquier archivo que será certificado.
//! La configuración puede coincidir con un archivo individual mediante [path](AssetConfig::File)
//! o con múltiples archivos utilizando un [patrón](AssetConfig::Pattern).
//!
//! En ambos casos, se pueden configurar las siguientes opciones para cada asset:
//!
//! - `content_type`
//!   - Al proporcionar esta opción, se certificará y servirá un encabezado `Content-Type`
//!     con el valor proporcionado.
//!   - Si este valor no se proporciona, el encabezado `Content-Type` no se insertará.
//!   - Si el navegador no recibe el encabezado `Content-Type`, intentará adivinar
//!     el tipo de contenido según la extensión del archivo, a menos que se envíe
//!     un encabezado `X-Content-Type-Options: nosniff`.
//!   - No certificar el encabezado `Content-Type` permitiría a una réplica maliciosa
//!     insertar su propio encabezado `Content-Type`, lo que podría generar una vulnerabilidad de seguridad.
//!
//! - `headers`
//!   - Cualquier encabezado adicional proporcionado será certificado y servido con el asset.
//!   - Es importante incluir encabezados que puedan afectar el comportamiento del navegador,
//!     en particular los [encabezados de seguridad](https://owasp.org/www-project-secure-headers/index.html).
//!
//! - `encodings`
//!     - Una lista de codificaciones alternativas que se pueden utilizar para servir el asset.
//!     - Cada entrada es una tupla con el [nombre de la codificación](AssetEncoding) y la extensión
//!       de archivo utilizada en la ruta, que se puede crear con el método `default_config`.
//!       Por ejemplo, para incluir las codificaciones Brotli y Gzip:
//!       `vec![AssetEncoding::Brotli.default_config(), AssetEncoding::Gzip.default_config()]`.
//!     - Extensiones de archivo predeterminadas para cada codificación:
//!         - Brotli: `br`
//!         - Gzip: `gz`
//!         - Deflate: `zz`
//!         - Zstd: `zst`
//!     - También se puede proporcionar una extensión personalizada usando `custom_config`.
//!       Ejemplo para Brotli y Gzip:
//!       `vec![AssetEncoding::Brotli.custom_config("brotli"), AssetEncoding::Gzip.custom_config("gzip")]`.
//!     - Cada codificación referenciada debe proporcionarse como un archivo separado en el enrutador,
//!       con el mismo nombre que el archivo original, pero con la extensión configurada. Por ejemplo,
//!       si el archivo original es `file.html`, se buscarán `file.html.br` y `file.html.gz`.
//!     - Si el archivo se encuentra, se certificará y servirá según el encabezado `Accept-Encoding`.
//!     - Orden de prioridad de las codificaciones:
//!         - Brotli
//!         - Zstd
//!         - Gzip
//!         - Deflate
//!         - Identidad
//!     - El enrutador de assets devolverá la codificación de mayor prioridad certificada y
//!       soportada por el cliente.
//!
//! ### Configuración de archivos individuales
//!
//! Al configurar un archivo individual, se proporciona la propiedad [path](AssetConfig::File::path),
//! que debe coincidir con la ruta pasada al constructor de [Asset] en el paso anterior.
//!
//! Además de las opciones comunes, los assets individuales pueden registrarse como
//! [respuestas de respaldo](AssetConfig::File::fallback_for) para un ámbito específico.
//! Esto se puede utilizar para configurar páginas 404 o puntos de entrada de aplicaciones de una sola página.
//!
//! Cuando se sirven assets, si no se encuentra una coincidencia exacta con la ruta solicitada,
//! se buscará un asset configurado con el ámbito de respaldo más cercano.
//!
//! Por ejemplo, si se solicita `/app.js` y no se encuentra un asset con esa ruta exacta,
//! se intentará servir un asset configurado con un ámbito de respaldo en `/`.
//!
//! La búsqueda continuará recursivamente hasta que no sea posible encontrar un respaldo válido.
//! Por ejemplo, si se solicita `/assets/js/app/core/index.js` y no se encuentra una coincidencia exacta,
//! se buscarán respaldos en el siguiente orden:
//!
//! - `/assets/js/app/core`
//! - `/assets/js/app`
//! - `/assets/js`
//! - `/assets`
//! - `/`
//!
//! Si se configuran múltiples respaldos, se usará el primero encontrado, ya que es el más específico.
//! Si no se encuentra ninguno, no se devolverá ninguna respuesta.
//!
//! También es posible registrar alias para un asset. Esto es útil cuando se necesitan múltiples
//! rutas para servir el mismo asset. Por ejemplo, si un asset tiene la ruta `index.html`,
//! se puede agregar un alias para que `/` sirva el mismo archivo.
//!
//! ```rust
//! use ic_http_certification::StatusCode;
//! use ic_asset_certification::{AssetConfig, AssetFallbackConfig, AssetEncoding};
//!
//! let config = AssetConfig::File {
//!     path: "index.html".to_string(),
//!     content_type: Some("text/html".to_string()),
//!     headers: vec![
//!         ("Cache-Control".to_string(), "public, no-cache, no-store".to_string()),
//!     ],
//!     fallback_for: vec![AssetFallbackConfig {
//!         scope: "/".to_string(),
//!         status_code: Some(StatusCode::OK),
//!     }],
//!     aliased_by: vec!["/".to_string()],
//!     encodings: vec![
//!         AssetEncoding::Brotli.default_config(),
//!         AssetEncoding::Gzip.default_config(),
//!     ],
//! };
//! ```
//! También es posible configurar múltiples recursos de respaldo para un solo asset.
//! El siguiente ejemplo configura un archivo HTML individual para ser servido en la ruta
//! `/404.html`, además de servir como respaldo para los ámbitos `/js` y `/css`.
//!
//! Cualquier solicitud a rutas que comiencen en los directorios `/js` y `/css` y que no
//! coincidan exactamente con un asset será redirigida al asset `/404.html`.
//!
//! También se configuran múltiples alias para este asset, a saber:
//! - `/404`,
//! - `/404/`,
//! - `/404.html`
//! - `/not-found`
//! - `/not-found/`
//! - `/not-found/index.html`
//!
//! Las solicitudes a cualquiera de estos alias servirán el asset `/404.html`.
//!
//! ```rust
//! use ic_http_certification::StatusCode;
//! use ic_asset_certification::{AssetConfig, AssetFallbackConfig, AssetEncoding};
//!
//! let config = AssetConfig::File {
//!     path: "404.html".to_string(),
//!     content_type: Some("text/html".to_string()),
//!     headers: vec![
//!         ("Cache-Control".to_string(), "public, no-cache, no-store".to_string()),
//!     ],
//!     fallback_for: vec![
//!         AssetFallbackConfig {
//!             scope: "/css".to_string(),
//!             status_code: Some(StatusCode::NOT_FOUND),
//!         },
//!         AssetFallbackConfig {
//!             scope: "/js".to_string(),
//!             status_code: Some(StatusCode::NOT_FOUND),
//!         },
//!     ],
//!     aliased_by: vec![
//!         "/404".to_string(),
//!         "/404/".to_string(),
//!         "/404.html".to_string(),
//!         "/not-found".to_string(),
//!         "/not-found/".to_string(),
//!         "/not-found/index.html".to_string(),
//!     ],
//!     encodings: vec![
//!         AssetEncoding::Brotli.default_config(),
//!         AssetEncoding::Gzip.default_config(),
//!     ],
//! };
//! ```
//!
//! ### Configuración de patrones de archivos
//!
//! Al configurar patrones de archivos, se proporciona la propiedad `pattern`.
//! Esta propiedad es un patrón de glob que se utilizará para hacer coincidir múltiples archivos.
//!
//! Se admite la sintaxis estándar de glob estilo Unix:
//!
//! - `?` coincide con cualquier carácter único.
//! - `*` coincide con cero o más caracteres.
//! - `**` coincide recursivamente con directorios, pero solo es válido en tres situaciones:
//!   - Si el glob comienza con `**\/`, entonces coincide con todos los directorios.  
//!     Por ejemplo, `**\/foo` coincide con `foo` y `bar\/foo`, pero no con `foo\/bar\`.
//!   - Si el glob termina con `\/**`, entonces coincide con todas las subentradas.  
//!     Por ejemplo, `foo\/\**` coincide con `foo\/a` y `foo\/a\/b`, pero no con `foo`.
//!   - Si el glob contiene `\/\**\/` en cualquier parte dentro del patrón, entonces coincide
//!     con cero o más directorios.
//!   - Usar `**` en cualquier otro lugar no es válido.
//!   - El glob `**` está permitido y significa "coincidir con todo".
//!   - `{a,b}` coincide con `a` o `b`, donde `a` y `b` son patrones de glob arbitrarios.
//!     (N.B. No se permite anidar `{...}` actualmente).
//!   - `[ab]` coincide con `a` o `b`, donde `a` y `b` son caracteres.
//!   - `[!ab]` coincide con cualquier carácter excepto `a` y `b`.
//!   - Los metacaracteres como `*` y `?` pueden escaparse con notación de clase de caracteres,
//!     por ejemplo, `[*]` coincide con `*`.
//!
//! Por ejemplo, el siguiente patrón coincidirá con todos los archivos `.js` en el directorio `js`:
//!
//! ```rust
//! use ic_http_certification::StatusCode;
//! use ic_asset_certification::{AssetConfig, AssetEncoding};
//!
//! let config = AssetConfig::Pattern {
//!     pattern: "js/*.js".to_string(),
//!     content_type: Some("application/javascript".to_string()),
//!     headers: vec![
//!         ("Cache-Control".to_string(), "public, max-age=31536000, immutable".to_string()),
//!     ],
//!     encodings: vec![
//!         AssetEncoding::Brotli.default_config(),
//!         AssetEncoding::Gzip.default_config(),
//!     ],
//! };
//! ```
//!
//! ### Configuración de redirecciones
//!
//! Las redirecciones se pueden configurar utilizando la variante [AssetConfig::Redirect].
//! Esta variante toma rutas `from` y `to`, y un tipo de redirección [kind](AssetRedirectKind).
//! Cuando se realiza una solicitud a la ruta `from`, el cliente será redirigido a la ruta `to`.
//! La configuración [AssetConfig::Redirect] no se compara con ningún [Asset].
//!
//! Las redirecciones pueden configurarse como [permanentes](AssetRedirectKind::Permanent)
//! o [temporales](AssetRedirectKind::Temporary).
//!
//! El navegador almacenará en caché las redirecciones permanentes y no volverá a solicitar
//! la ubicación antigua. Esto es útil cuando el recurso se ha trasladado permanentemente a
//! una nueva ubicación. El navegador actualizará sus marcadores y los resultados de los
//! motores de búsqueda.
//!
//! Consulta la documentación de MDN Web Docs sobre  
//! [redirecciones permanentes](https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/301)
//! para más información.
//!
//! El navegador no almacenará en caché las redirecciones temporales y volverá a solicitar
//! la ubicación antigua. Esto es útil cuando el recurso se ha trasladado temporalmente a
//! una nueva ubicación. El navegador no actualizará sus marcadores ni los resultados de los motores de búsqueda.
//!
//! Consulta la documentación de MDN Web Docs sobre  
//! [redirecciones temporales](https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/307)
//! para más información.
//!
//! El siguiente ejemplo configura una redirección permanente de `/old` a `/new`:
//!
//! ```rust
//! use ic_asset_certification::{AssetConfig, AssetRedirectKind};
//!
//! let config = AssetConfig::Redirect {
//!     from: "/old".to_string(),
//!     to: "/new".to_string(),
//!     kind: AssetRedirectKind::Permanent,
//!     headers: vec![(
//!         "content-type".to_string(),
//!         "text/plain; charset=utf-8".to_string(),
//!     )],
//! };
//! ```
//!
//! ## Inserción de assets en el enrutador de assets
//!
//! El [AssetRouter] es responsable de certificar respuestas y enrutar solicitudes
//! a la respuesta adecuada.
//!
//! Los assets pueden insertarse utilizando el método  
//! [certify_assets](AssetRouter::certify_assets):
//!
//! ```rust
//! use ic_http_certification::StatusCode;
//! use ic_asset_certification::{Asset, AssetConfig, AssetFallbackConfig, AssetRouter, AssetRedirectKind, AssetEncoding};
//!
//! let mut asset_router = AssetRouter::default();
//!
//! let assets = vec![
//!     Asset::new(
//!         "index.html",
//!         b"<html><body><h1>Hello World!</h1></body></html>".as_slice(),
//!     ),
//!     Asset::new(
//!         "index.html.gz",
//!         &[0, 1, 2, 3, 4, 5]
//!     ),
//!     Asset::new(
//!         "index.html.br",
//!         &[6, 7, 8, 9, 10, 11]
//!     ),
//!     Asset::new(
//!         "app.js",
//!         b"console.log('Hello World!');".as_slice(),
//!     ),
//!     Asset::new(
//!         "app.js.gz",
//!         &[12, 13, 14, 15, 16, 17],
//!     ),
//!     Asset::new(
//!         "app.js.br",
//!         &[18, 19, 20, 21, 22, 23],
//!     ),
//!     Asset::new(
//!         "css/app-ba74b708.css",
//!         b"html,body{min-height:100vh;}".as_slice(),
//!     ),
//!     Asset::new(
//!         "css/app-ba74b708.css.gz",
//!         &[24, 25, 26, 27, 28, 29],
//!     ),
//!     Asset::new(
//!         "css/app-ba74b708.css.br",
//!         &[30, 31, 32, 33, 34, 35],
//!     ),
//! ];
//!
//! let asset_configs = vec![
//!     AssetConfig::File {
//!         path: "index.html".to_string(),
//!         content_type: Some("text/html".to_string()),
//!         headers: vec![(
//!             "cache-control".to_string(),
//!             "public, no-cache, no-store".to_string(),
//!         )],
//!         fallback_for: vec![AssetFallbackConfig {
//!             scope: "/".to_string(),
//!             status_code: Some(StatusCode::OK),
//!         }],
//!         aliased_by: vec!["/".to_string()],
//!         encodings: vec![
//!             AssetEncoding::Brotli.default_config(),
//!             AssetEncoding::Gzip.default_config(),
//!         ],
//!     },
//!     AssetConfig::Pattern {
//!         pattern: "**/*.js".to_string(),
//!         content_type: Some("text/javascript".to_string()),
//!         headers: vec![(
//!             "cache-control".to_string(),
//!             "public, max-age=31536000, immutable".to_string(),
//!         )],
//!         encodings: vec![
//!             AssetEncoding::Brotli.default_config(),
//!             AssetEncoding::Gzip.default_config(),
//!         ],
//!     },
//!     AssetConfig::Pattern {
//!         pattern: "**/*.css".to_string(),
//!         content_type: Some("text/css".to_string()),
//!         headers: vec![(
//!             "cache-control".to_string(),
//!             "public, max-age=31536000, immutable".to_string(),
//!         )],
//!         encodings: vec![
//!             AssetEncoding::Brotli.default_config(),
//!             AssetEncoding::Gzip.default_config(),
//!         ],
//!     },
//!     AssetConfig::Redirect {
//!         from: "/old".to_string(),
//!         to: "/new".to_string(),
//!         kind: AssetRedirectKind::Permanent,
//!         headers: vec![(
//!             "content-type".to_string(),
//!             "text/plain; charset=utf-8".to_string(),
//!         )],
//!     },
//! ];
//!
//! asset_router.certify_assets(assets, asset_configs).unwrap();
//! ```
//!
//! Después de certificar los assets, asegúrate de establecer los datos
//! certificados del canister:
//!
//! ```ignore
//! use ic_cdk::api::set_certified_data;
//!
//! set_certified_data(&asset_router.root_hash());
//! ```
//!
//! También es posible inicializar el enrutador con un
//! [HttpCertificationTree](ic_http_certification::HttpCertificationTree). Esto es
//! útil cuando se requiere acceso directo al
//! [HttpCertificationTree](ic_http_certification::HttpCertificationTree) para certificar
//! [HttpRequest](ic_http_certification::HttpRequest)s y
//! [HttpResponse](ic_http_certification::HttpResponse)s fuera del [AssetRouter].
//!
//! ```rust
//! use std::{cell::RefCell, rc::Rc};
//! use ic_http_certification::HttpCertificationTree;
//! use ic_asset_certification::AssetRouter;
//!
//! let mut http_certification_tree: Rc<RefCell<HttpCertificationTree>> = Default::default();
//! let mut asset_router = AssetRouter::with_tree(http_certification_tree.clone());
//! ```
//!
//! ## Servir assets
//!
//! Los assets pueden servirse llamando al método `serve_asset` en `AssetRouter`.
//! Este método devolverá una respuesta, un testigo y una ruta de expresión, que pueden utilizarse
//! junto con el certificado de datos del canister para agregar el encabezado de certificado necesario a la respuesta.
//!
//! ```rust
//! use ic_http_certification::{HttpRequest, utils::add_v2_certificate_header, StatusCode};
//! use ic_asset_certification::{Asset, AssetConfig, AssetFallbackConfig, AssetRouter};
//!
//! let mut asset_router = AssetRouter::default();
//!
//! let asset = Asset::new(
//!     "index.html",
//!     b"<html><body><h1>¡Hola Mundo!</h1></body></html>".as_slice(),
//! );
//!
//! let asset_config = AssetConfig::File {
//!     path: "index.html".to_string(),
//!     content_type: Some("text/html".to_string()),
//!     headers: vec![
//!         ("Cache-Control".to_string(), "public, no-cache, no-store".to_string()),
//!     ],
//!     fallback_for: vec![AssetFallbackConfig {
//!         scope: "/".to_string(),
//!         status_code: Some(StatusCode::OK),
//!     }],
//!     aliased_by: vec!["/".to_string()],
//!     encodings: vec![],
//! };
//!
//! let http_request = HttpRequest::get("/").build();
//!
//! asset_router.certify_assets(vec![asset], vec![asset_config]).unwrap();
//!
//! // Esto normalmente debería obtenerse usando `ic_cdk::api::data_certificate()`.
//! let data_certificate = vec![1, 2, 3];
//! let response = asset_router.serve_asset(&data_certificate, &http_request).unwrap();
//!```
//!
//! ## Eliminar assets
//!
//! Hay tres formas de eliminar assets del enrutador de assets:
//! 1. [Por configuración](#deleting-assets-by-configuration).
//! 1. [Por ruta](#deleting-assets-by-path).
//! 1. [Todos a la vez](#deleting-all-assets).
//!
//! ### Eliminar assets por configuración
//!
//! Eliminar assets por configuración es similar a [certificarlos](#inserting-assets-into-the-asset-router).
//!
//! Dependiendo de la configuración proporcionada a la función [certify_assets](AssetRouter::certify_assets),
//! pueden generarse múltiples respuestas para el mismo asset. Para garantizar que todas las respuestas generadas
//! sean eliminadas, la función [delete_assets](AssetRouter::delete_assets) acepta la misma configuración.
//!
//! Si se proporciona una configuración diferente a la utilizada originalmente para certificar los assets,
//! pueden ocurrir dos cosas:
//!
//! 1. Si la configuración incluye un archivo que no fue certificado inicialmente, será ignorado silenciosamente.
//! Por ejemplo, si la configuración proporcionada a `certify_assets` incluye las codificaciones Brotli y Gzip,
//! pero la configuración proporcionada a `delete_assets` incluye Brotli, Gzip y Deflate, los archivos codificados
//! en Brotli y Gzip serán eliminados, mientras que el archivo Deflate será ignorado, ya que no existe.
//!
//! 2. Si la configuración excluye un archivo que fue certificado, este no será eliminado. Por ejemplo,
//! si la configuración proporcionada a `certify_assets` incluye las codificaciones Brotli y Gzip, pero
//! la configuración proporcionada a `delete_assets` solo incluye Brotli, entonces el archivo Gzip no será eliminado.
//!
//! Suponiendo el mismo ejemplo base utilizado anteriormente para demostrar la certificación de assets:
//!
//! ```rust
//! use ic_http_certification::StatusCode;
//! use ic_asset_certification::{Asset, AssetConfig, AssetFallbackConfig, AssetRouter, AssetRedirectKind, AssetEncoding};
//!
//! let mut asset_router = AssetRouter::default();
//!
//! let assets = vec![
//!     Asset::new(
//!         "index.html",
//!         b"<html><body><h1>Hello World!</h1></body></html>".as_slice(),
//!     ),
//!     Asset::new(
//!         "index.html.gz",
//!         &[0, 1, 2, 3, 4, 5]
//!     ),
//!     Asset::new(
//!         "index.html.br",
//!         &[6, 7, 8, 9, 10, 11]
//!     ),
//!     Asset::new(
//!         "app.js",
//!         b"console.log('Hello World!');".as_slice(),
//!     ),
//!     Asset::new(
//!         "app.js.gz",
//!         &[12, 13, 14, 15, 16, 17],
//!     ),
//!     Asset::new(
//!         "app.js.br",
//!         &[18, 19, 20, 21, 22, 23],
//!     ),
//!     Asset::new(
//!         "css/app-ba74b708.css",
//!         b"html,body{min-height:100vh;}".as_slice(),
//!     ),
//!     Asset::new(
//!         "css/app-ba74b708.css.gz",
//!         &[24, 25, 26, 27, 28, 29],
//!     ),
//!     Asset::new(
//!         "css/app-ba74b708.css.br",
//!         &[30, 31, 32, 33, 34, 35],
//!     ),
//! ];
//!
//! let asset_configs = vec![
//!     AssetConfig::File {
//!         path: "index.html".to_string(),
//!         content_type: Some("text/html".to_string()),
//!         headers: vec![(
//!             "cache-control".to_string(),
//!             "public, no-cache, no-store".to_string(),
//!         )],
//!         fallback_for: vec![AssetFallbackConfig {
//!             scope: "/".to_string(),
//!             status_code: Some(StatusCode::OK),
//!         }],
//!         aliased_by: vec!["/".to_string()],
//!         encodings: vec![
//!             AssetEncoding::Brotli.default_config(),
//!             AssetEncoding::Gzip.default_config(),
//!         ],
//!     },
//!     AssetConfig::Pattern {
//!         pattern: "**/*.js".to_string(),
//!         content_type: Some("text/javascript".to_string()),
//!         headers: vec![(
//!             "cache-control".to_string(),
//!             "public, max-age=31536000, immutable".to_string(),
//!         )],
//!         encodings: vec![
//!             AssetEncoding::Brotli.default_config(),
//!             AssetEncoding::Gzip.default_config(),
//!         ],
//!     },
//!     AssetConfig::Pattern {
//!         pattern: "**/*.css".to_string(),
//!         content_type: Some("text/css".to_string()),
//!         headers: vec![(
//!             "cache-control".to_string(),
//!             "public, max-age=31536000, immutable".to_string(),
//!         )],
//!         encodings: vec![
//!             AssetEncoding::Brotli.default_config(),
//!             AssetEncoding::Gzip.default_config(),
//!         ],
//!     },
//!     AssetConfig::Redirect {
//!         from: "/old".to_string(),
//!         to: "/new".to_string(),
//!         kind: AssetRedirectKind::Permanent,
//!         headers: vec![(
//!             "content-type".to_string(),
//!             "text/plain; charset=utf-8".to_string(),
//!         )],
//!     },
//! ];
//!
//! asset_router.certify_assets(assets, asset_configs).unwrap();
//! ```
//!
//! Para eliminar el asset `index.html`, junto con la configuración de respaldo para el ámbito `/`, el alias `/` y las codificaciones alternativas:
//!
//! ```rust
//! # use ic_http_certification::StatusCode;
//! # use ic_asset_certification::{Asset, AssetConfig, AssetFallbackConfig, AssetRouter, AssetRedirectKind, AssetEncoding};
//!
//! # let mut asset_router = AssetRouter::default();
//!
//! asset_router
//!     .delete_assets(
//!         vec![
//!             Asset::new(
//!                 "index.html",
//!                 b"<html><body><h1>Hello World!</h1></body></html>".as_slice(),
//!             ),
//!             Asset::new("index.html.gz", &[0, 1, 2, 3, 4, 5]),
//!             Asset::new("index.html.br", &[6, 7, 8, 9, 10, 11]),
//!         ],
//!         vec![AssetConfig::File {
//!             path: "index.html".to_string(),
//!             content_type: Some("text/html".to_string()),
//!             headers: vec![(
//!                 "cache-control".to_string(),
//!                 "public, no-cache, no-store".to_string(),
//!             )],
//!             fallback_for: vec![AssetFallbackConfig {
//!                 scope: "/".to_string(),
//!                 status_code: Some(StatusCode::OK),
//!             }],
//!             aliased_by: vec!["/".to_string()],
//!             encodings: vec![
//!                 AssetEncoding::Brotli.default_config(),
//!                 AssetEncoding::Gzip.default_config(),
//!             ],
//!         }],
//!     )
//!     .unwrap();
//! ```
//!
//! Para eliminar el asset `app.js`, junto con las codificaciones alternativas:
//!
//! ```rust
//! # use ic_http_certification::StatusCode;
//! # use ic_asset_certification::{Asset, AssetConfig, AssetFallbackConfig, AssetRouter, AssetRedirectKind, AssetEncoding};
//!
//! # let mut asset_router = AssetRouter::default();
//!
//! asset_router
//!     .delete_assets(
//!         vec![
//!             Asset::new("app.js", b"console.log('Hello World!');".as_slice()),
//!             Asset::new("app.js.gz", &[12, 13, 14, 15, 16, 17]),
//!             Asset::new("app.js.br", &[18, 19, 20, 21, 22, 23]),
//!         ],
//!         vec![AssetConfig::Pattern {
//!             pattern: "**/*.js".to_string(),
//!             content_type: Some("text/javascript".to_string()),
//!             headers: vec![(
//!                 "cache-control".to_string(),
//!                 "public, max-age=31536000, immutable".to_string(),
//!             )],
//!             encodings: vec![
//!                 AssetEncoding::Brotli.default_config(),
//!                 AssetEncoding::Gzip.default_config(),
//!             ],
//!         }],
//!     )
//!     .unwrap();
//! ```
//!
//! Para eliminar el asset `css/app-ba74b708.css`, junto con las codificaciones alternativas:
//!
//! ```rust
//! # use ic_http_certification::StatusCode;
//! # use ic_asset_certification::{Asset, AssetConfig, AssetFallbackConfig, AssetRouter, AssetRedirectKind, AssetEncoding};
//!
//! # let mut asset_router = AssetRouter::default();
//!
//! asset_router.delete_assets(
//!     vec![
//!         Asset::new(
//!             "css/app-ba74b708.css",
//!             b"html,body{min-height:100vh;}".as_slice(),
//!         ),
//!         Asset::new(
//!             "css/app-ba74b708.css.gz",
//!             &[24, 25, 26, 27, 28, 29],
//!         ),
//!         Asset::new(
//!             "css/app-ba74b708.css.br",
//!             &[30, 31, 32, 33, 34, 35],
//!         ),
//!     ],
//!     vec![
//!         AssetConfig::Pattern {
//!             pattern: "**/*.css".to_string(),
//!             content_type: Some("text/css".to_string()),
//!             headers: vec![(
//!                 "cache-control".to_string(),
//!                 "public, max-age=31536000, immutable".to_string(),
//!             )],
//!             encodings: vec![
//!                 AssetEncoding::Brotli.default_config(),
//!                 AssetEncoding::Gzip.default_config(),
//!             ],
//!         },
//!     ]
//! ).unwrap();
//! ```
//!
//! Y finalmente, para eliminar la redirección `/old`:
//!
//! ```rust
//! # use ic_asset_certification::{Asset, AssetConfig, AssetFallbackConfig, AssetRouter, AssetRedirectKind, AssetEncoding};
//!
//! # let mut asset_router = AssetRouter::default();
//!
//! asset_router
//!     .delete_assets(
//!         vec![],
//!         vec![AssetConfig::Redirect {
//!             from: "/old".to_string(),
//!             to: "/new".to_string(),
//!             kind: AssetRedirectKind::Permanent,
//!             headers: vec![(
//!                 "content-type".to_string(),
//!                 "text/plain; charset=utf-8".to_string(),
//!              )],
//!         }],
//!     )
//!     .unwrap();
//! ```
//!
//! Después de eliminar cualquier asset, asegúrese de establecer los datos certificados del canister nuevamente:
//!
//! ```ignore
//! use ic_cdk::api::set_certified_data;
//!
//! set_certified_data(&asset_router.root_hash());
//! ```
//!
//! ### Eliminación de assets por ruta
//!
//! Para eliminar assets por ruta, utilice la función [delete_assets_by_path](AssetRouter::delete_assets_by_path).
//!
//! Dependiendo de la configuración proporcionada a la función [certify_assets](AssetRouter::certify_assets),
//! se pueden generar múltiples respuestas para el mismo asset. Estos assets pueden existir en diferentes rutas,
//! por ejemplo, si se utiliza la configuración de `alias`. Si no se pasan las rutas de `alias` a esta función,
//! no se eliminarán.
//!
//! Si existen múltiples codificaciones para una ruta, se eliminarán todas las codificaciones.
//!
//! Las alternativas tampoco se eliminan; para eliminarlas, utilice la función
//! [delete_fallback_assets_by_path](AssetRouter::delete_fallback_assets_by_path).
//!
//! Suponiendo el mismo ejemplo base utilizado anteriormente para demostrar la certificación de assets:
//!
//! ```rust
//! use ic_http_certification::StatusCode;
//! use ic_asset_certification::{Asset, AssetConfig, AssetFallbackConfig, AssetRouter, AssetRedirectKind, AssetEncoding};
//!
//! let mut asset_router = AssetRouter::default();
//!
//! let assets = vec![
//!     Asset::new(
//!         "index.html",
//!         b"<html><body><h1>Hello World!</h1></body></html>".as_slice(),
//!     ),
//!     Asset::new(
//!         "index.html.gz",
//!         &[0, 1, 2, 3, 4, 5]
//!     ),
//!     Asset::new(
//!         "index.html.br",
//!         &[6, 7, 8, 9, 10, 11]
//!     ),
//!     Asset::new(
//!         "app.js",
//!         b"console.log('Hello World!');".as_slice(),
//!     ),
//!     Asset::new(
//!         "app.js.gz",
//!         &[12, 13, 14, 15, 16, 17],
//!     ),
//!     Asset::new(
//!         "app.js.br",
//!         &[18, 19, 20, 21, 22, 23],
//!     ),
//!     Asset::new(
//!         "css/app-ba74b708.css",
//!         b"html,body{min-height:100vh;}".as_slice(),
//!     ),
//!     Asset::new(
//!         "css/app-ba74b708.css.gz",
//!         &[24, 25, 26, 27, 28, 29],
//!     ),
//!     Asset::new(
//!         "css/app-ba74b708.css.br",
//!         &[30, 31, 32, 33, 34, 35],
//!     ),
//! ];
//!
//! let asset_configs = vec![
//!     AssetConfig::File {
//!         path: "index.html".to_string(),
//!         content_type: Some("text/html".to_string()),
//!         headers: vec![(
//!             "cache-control".to_string(),
//!             "public, no-cache, no-store".to_string(),
//!         )],
//!         fallback_for: vec![AssetFallbackConfig {
//!             scope: "/".to_string(),
//!             status_code: Some(StatusCode::OK),
//!         }],
//!         aliased_by: vec!["/".to_string()],
//!         encodings: vec![
//!             AssetEncoding::Brotli.default_config(),
//!             AssetEncoding::Gzip.default_config(),
//!         ],
//!     },
//!     AssetConfig::Pattern {
//!         pattern: "**/*.js".to_string(),
//!         content_type: Some("text/javascript".to_string()),
//!         headers: vec![(
//!             "cache-control".to_string(),
//!             "public, max-age=31536000, immutable".to_string(),
//!         )],
//!         encodings: vec![
//!             AssetEncoding::Brotli.default_config(),
//!             AssetEncoding::Gzip.default_config(),
//!         ],
//!     },
//!     AssetConfig::Pattern {
//!         pattern: "**/*.css".to_string(),
//!         content_type: Some("text/css".to_string()),
//!         headers: vec![(
//!             "cache-control".to_string(),
//!             "public, max-age=31536000, immutable".to_string(),
//!         )],
//!         encodings: vec![
//!             AssetEncoding::Brotli.default_config(),
//!             AssetEncoding::Gzip.default_config(),
//!         ],
//!     },
//!     AssetConfig::Redirect {
//!         from: "/old".to_string(),
//!         to: "/new".to_string(),
//!         kind: AssetRedirectKind::Permanent,
//!         headers: vec![("content-type".to_string(), "text/plain".to_string())],
//!     },
//! ];
//!
//! asset_router.certify_assets(assets, asset_configs).unwrap();
//! ```
//!
//! Para eliminar el asset `index.html`, junto con la configuración de respaldo para el ámbito `/`, el alias `/` y las codificaciones alternativas:
//!
//! ```rust
//! # use ic_http_certification::StatusCode;
//! # use ic_asset_certification::{Asset, AssetConfig, AssetFallbackConfig, AssetRouter, AssetRedirectKind, AssetEncoding};
//!
//! # let mut asset_router = AssetRouter::default();
//!
//! asset_router
//!     .delete_assets_by_path(
//!         vec![
//!             "/index.html", // deletes the index.html asset, along with all encodings
//!             "/" // deletes the `/` alias for index.html, along with all encodings
//!         ],
//!     );
//!
//! asset_router
//!     .delete_fallback_assets_by_path(
//!        vec![
//!           "/" // deletes the fallback configuration for the `/` scope, along with all encodings
//!       ]
//!    );
//! ```
//!
//! Para eliminar el asset `app.js`, junto con las codificaciones alternativas:
//!
//! ```rust
//! # use ic_http_certification::StatusCode;
//! # use ic_asset_certification::{Asset, AssetConfig, AssetFallbackConfig, AssetRouter, AssetRedirectKind, AssetEncoding};
//!
//! # let mut asset_router = AssetRouter::default();
//!
//! asset_router.delete_assets_by_path(vec!["/app.js"]);
//! ```
//!
//! Para eliminar el asset `css/app-ba74b708.css`, junto con las codificaciones alternativas:
//!
//! ```rust
//! # use ic_http_certification::StatusCode;
//! # use ic_asset_certification::{Asset, AssetConfig, AssetFallbackConfig, AssetRouter, AssetRedirectKind, AssetEncoding};
//!
//! # let mut asset_router = AssetRouter::default();
//!
//! asset_router.delete_assets_by_path(vec!["/css/app-ba74b708.css"]);
//! ```
//!
//! Y finalmente, para eliminar la redirección `/old`:
//!
//! ```rust
//! # use ic_asset_certification::{Asset, AssetConfig, AssetFallbackConfig, AssetRouter, AssetRedirectKind, AssetEncoding};
//!
//! # let mut asset_router = AssetRouter::default();
//!
//! asset_router.delete_assets_by_path(vec!["/old"]);
//! ```
//!
//! Luego de eliminar cualquier asset, asegúrate de establecer los datos certificados del canister nuevamente:
//!
//! ```ignore
//! use ic_cdk::api::set_certified_data;
//!
//! set_certified_data(&asset_router.root_hash());
//! ```
//!
//! ### Eliminando todos los assets
//!
//! También es posible eliminar todos los assets y su certificación de una sola vez:
//!
//! ```rust
//! # use ic_asset_certification::AssetRouter;
//!
//! # let mut asset_router = AssetRouter::default();
//!
//! asset_router.delete_all_assets();
//! ```
//!
//! Luego de eliminar cualquier asset, asegúrate de establecer los datos certificados del canister nuevamente:
//!
//! ```ignore
//! use ic_cdk::api::set_certified_data;
//!
//! set_certified_data(&asset_router.root_hash());
//! ```
//!
//! ## Consulta de assets
//!
//! El [AssetRouter] tiene dos funciones para obtener un [AssetMap] que contiene assets.
//!
//! La función [get_assets()](AssetRouter::get_assets) devuelve todos los assets estándar, mientras que la
//! función [get_fallback_assets()](AssetRouter::get_fallback_assets) devuelve todos los assets de respaldo.
//!
//! El [AssetMap] se puede utilizar para consultar assets por `path`, `encoding` y `starting_range`.
//! Para los assets estándar, el path se refiere al path del asset, por ejemplo, `/index.html`.
//!
//! Para los assets de respaldo, el path se refiere al ámbito para el cual el respaldo es válido, por ejemplo, `/`.
//! Consulta la opción de configuración [fallback_for](crate::AssetConfig::File::fallback_for) para obtener más información
//! sobre los ámbitos de respaldo.
//!
//! Para todos los tipos de assets, la codificación se refiere a la codificación del asset; consulta [AssetEncoding].
//!
//! Los assets mayores a 2 MiB se dividen en múltiples rangos; el rango inicial permite obtener
//! fragmentos individuales de estos assets grandes. El primer rango es `Some(0)`, el segundo rango es
//! `Some(ASSET_CHUNK_SIZE)`, el tercer rango es `Some(ASSET_CHUNK_SIZE * 2)`, y así sucesivamente. El asset completo también se puede obtener
//! pasando `None` como `starting_range`.
//! Consulta [ASSET_CHUNK_SIZE] para conocer el tamaño de cada fragmento.

#![deny(missing_docs, missing_debug_implementations, rustdoc::all, clippy::all)]

mod asset;
mod asset_config;
mod asset_map;
mod asset_router;
mod error;
mod types;

pub use asset::*;
pub use asset_config::*;
pub use asset_map::*;
pub use asset_router::*;
pub use error::*;
pub(crate) use types::*;
