# Certificación de assets

La certificación de assets es una forma especializada de
[certificación HTTP](https://internetcomputer.org/docs/current/developer-docs/http-compatible-canisters/custom-http-canisters)
diseñada para certificar assets estáticos en canisters de
[ICP](https://internetcomputer.org/).

El crate `ic-asset-certification` proporciona la funcionalidad necesaria para
certificar y servir assets estáticos desde canisters en Rust.

Esto se implementa en los siguientes pasos:

1. [Preparar assets](#preparar-assets).
2. [Configurar la certificación de assets](#configurar-la-certificación-de-assets).
3. [Insertar assets en el enrutador de assets](#insertar-assets-en-el-enrutador-de-assets).
4. [Servir assets](#servir-assets).
5. [Eliminar assets](#eliminar-assets).
6. [Consultar assets](#consultar-assets).

Para los canisters que lo necesiten, también es posible
[eliminar assets](#eliminar-assets).

## Preparar assets

Esta biblioteca no impone restricciones sobre el origen de los assets. Sin
embargo, existen tres opciones principales:

- Incluir assets en el canister en tiempo de compilación:
  - [`include_bytes!`](https://doc.rust-lang.org/std/macro.include_bytes.html)
  - [`include_dir!`](https://docs.rs/include_dir/latest/include_dir/index.html)
- Cargar assets a través de endpoints del canister en tiempo de ejecución:
  - El
    [`dfx` asset canister](https://github.com/dfinity/sdk/blob/master/docs/design/asset-canister-interface.md)
    es un buen ejemplo de este enfoque.
- Generar assets dinámicamente en código en tiempo de ejecución.

Una vez que los assets están en memoria, pueden convertirse en el tipo `Asset`:

```rust
use ic_asset_certification::Asset;

let asset = Asset::new(
    "index.html",
    b"<html><body><h1>¡Hola Mundo!</h1></body></html>".as_slice(),
);
```

Se recomienda utilizar referencias al incluir assets directamente en el canister
para evitar duplicar el contenido. Esto es especialmente importante para assets
grandes.

```rust
use ic_asset_certification::Asset;

let pretty_big_asset = include_bytes!("lib.rs");
let asset = Asset::new(
    "assets/pretty-big-asset.gz",
    pretty_big_asset.as_slice(),
);
```

En algunos casos, puede ser necesario usar valores propios, como cuando los
assets se generan dinámicamente o se modifican en tiempo de ejecución.

```rust
use ic_asset_certification::Asset;

let name = "Mundo";
let asset = Asset::new(
    "index.html",
    format!("<html><body><h1>¡Hola {name}!</h1></body></html>").into_bytes(),
);
```

## Configurar la certificación de assets

`AssetConfig` define la configuración para cualquier archivo que será
certificado. La configuración puede aplicarse a un archivo individual por ruta o
a múltiples archivos mediante un patrón.

En ambos casos, las siguientes opciones pueden configurarse para cada asset:

- `content_type`
  - Si se proporciona esta opción, se certificará y servirá un encabezado
    `Content-Type` con el valor especificado.
  - Si no se proporciona, el encabezado `Content-Type` no será insertado.
  - Si el encabezado `Content-Type` no es enviado al navegador, este intentará
    adivinar el tipo de contenido basado en la extensión del archivo, a menos
    que se envíe un encabezado `X-Content-Type-Options: nosniff`.
  - No certificar el encabezado `Content-Type` podría permitir que una réplica
    maliciosa inserte su propio encabezado `Content-Type`, lo que podría generar
    una vulnerabilidad de seguridad.
- `headers`
  - Cualquier encabezado adicional que se proporcione será certificado y servido
    con el asset.
  - Es importante incluir encabezados que puedan afectar el comportamiento del
    navegador, especialmente
    [encabezados de seguridad](https://owasp.org/www-project-secure-headers/index.html).
- `encodings`
  - Una lista de codificaciones alternativas que pueden usarse para servir el
    asset.
  - Cada entrada es una tupla con el nombre de la codificación y la extensión de
    archivo utilizada en la ruta.
  - Cada entrada es una tupla con el nombre de la codificación y la extensión de
    archivo utilizada en la ruta, que se puede crear de manera conveniente con
    el método de fábrica `default`. Por ejemplo, para incluir las codificaciones Brotli y Gzip:
    `vec![AssetEncoding::Brotli.default(), AssetEncoding::Gzip.default()]`.
  - Las extensiones de archivo predeterminadas para cada codificación son:
    - Brotli: `br`
    - Gzip: `gz`
    - Deflate: `zz`
    - Zstd: `zst`
  - Alternativamente, se puede proporcionar una extensión de archivo personalizada para cada codificación
    utilizando el método de fábrica `custom`. Por ejemplo, para incluir una extensión de archivo personalizada
    para las codificaciones Brotli y Gzip:
    `vec![AssetEncoding::Brotli.custom("brotli"), AssetEncoding::Gzip.custom("gzip")]`.
  - Cada codificación referenciada debe ser proporcionada al enrutador de assets como un
    archivo separado con el mismo nombre de archivo que el archivo original, pero con una
    extensión de archivo adicional que coincida con la configuración. Por ejemplo, si el
    archivo coincidente actual se llama `file.html`, entonces el enrutador de assets
    buscará `file.html.br` y `file.html.gz`.
  - Si se encuentra el archivo, el asset será certificado y servido con la
    codificación proporcionada según el `Accept-Encoding`.
  - Las codificaciones se priorizan en el siguiente orden:
    - Brotli
    - Zstd
    - Gzip
    - Deflate
    - Identity
  - El enrutador de assets devolverá la codificación de mayor prioridad que ha sido
    certificada y es compatible con el cliente.

### Configurar archivos individuales

Cuando se configura un archivo individual, la propiedad `path` debe coincidir
con la ruta utilizada en el constructor de `Asset`.

También es posible registrar un asset como respuesta alternativa para un
determinado alcance (scope), útil para configurar páginas 404 o puntos de
entrada de aplicaciones de una sola página (SPA).
Cuando se sirven assets, si una ruta solicitada no coincide exactamente con ningún asset, se realiza una búsqueda de un asset configurado con el alcance de fallback que coincida más estrechamente con la ruta del asset solicitado.

Por ejemplo, si se realiza una solicitud para `/app.js` y no se encuentra ningún asset con esa ruta exacta, se intentará servir un asset configurado con un alcance de fallback de `/`.

Esto se hará de forma recursiva hasta que ya no sea posible encontrar un fallback válido. Por ejemplo, si se realiza una solicitud para `/assets/js/app/core/index.js` y no se encuentra ningún asset con esa ruta exacta, entonces la búsqueda verificará los assets configurados con los siguientes alcances de fallback, en orden:

- `/assets/js/app/core`
- `/assets/js/app`
- `/assets/js`
- `/assets`
- `/`

Si se configuran varios assets de fallback, se utilizará el primero que se encuentre, ya que será el más específico disponible para esa ruta. Si no se encuentra ningún asset con ninguno de estos alcances de fallback, no se devolverá ninguna respuesta.

También es posible registrar alias para un asset. Esto puede ser útil para configurar múltiples rutas que deben servir el mismo asset. Por ejemplo, si se configura un asset con la ruta `index.html`, se puede alias la ruta `/`.

El siguiente ejemplo configura un archivo HTML individual para ser servido en la ruta `/index.html`, además de servir como fallback para el alcance `/` y establecer `/` como un alias para este asset.

```rust
use ic_http_certification::StatusCode;
use ic_asset_certification::{AssetConfig, AssetFallbackConfig};

let config = AssetConfig::File {
    path: "index.html".to_string(),
    content_type: Some("text/html".to_string()),
    headers: vec![
        ("Cache-Control".to_string(), "public, no-cache, no-store".to_string()),
    ],
    fallback_for: vec![AssetFallbackConfig {
        scope: "/".to_string(),
        status_code: Some(StatusCode::OK),
    }],
    aliased_by: vec!["/".to_string()],
    encodings: vec![
        AssetEncoding::Brotli.default(),
        AssetEncoding::Gzip.default()
    ],
};
```

También es posible configurar múltiples fallbacks para un solo asset. El siguiente ejemplo configura un archivo HTML individual para ser servido en la ruta `/404.html`, además de servir como fallback para los alcances `/js` y `/css`.

Cualquier solicitud a rutas que comiencen en los directorios `/js` y `/css` y que no coincidan exactamente con un asset será enrutada al asset `/404.html`.

También se configuran múltiples alias para este asset, a saber:

- `/404`,
- `/404/`,
- `/404.html`
- `/not-found`
- `/not-found/`
- `/not-found/index.html`

Las solicitudes a cualquiera de esos alias servirán el asset `/404.html`.

```rust
use ic_http_certification::StatusCode;
use ic_asset_certification::{AssetConfig, AssetFallbackConfig};

let config = AssetConfig::File {
    path: "404.html".to_string(),
    content_type: Some("text/html".to_string()),
    headers: vec![
        ("Cache-Control".to_string(), "public, no-cache, no-store".to_string()),
    ],
    fallback_for: vec![
        AssetFallbackConfig {
            scope: "/css".to_string(),
            status_code: Some(StatusCode::NOT_FOUND),
        },
        AssetFallbackConfig {
            scope: "/js".to_string(),
            status_code: Some(StatusCode::NOT_FOUND),
        },
    ],
    aliased_by: vec![
        "/404".to_string(),
        "/404/".to_string(),
        "/404.html".to_string(),
        "/not-found".to_string(),
        "/not-found/".to_string(),
        "/not-found/index.html".to_string(),
    ],
    encodings: vec![
        AssetEncoding::Brotli.default(),
        AssetEncoding::Gzip.default(),
    ],
};
```

### Configurar patrones de archivos

Cuando se configuran patrones de archivos, se usa la propiedad `pattern`, que
define una expresión glob para hacer coincidir múltiples archivos.

La sintaxis de glob estilo Unix estándar es compatible:

- `?` coincide con cualquier carácter individual.
- `*` coincide con cero o más caracteres.
- `**` coincide recursivamente con directorios, pero solo es válido en tres situaciones.
  - Si el glob comienza con `**/`, entonces coincide con todos los directorios.
    Por ejemplo, `**/foo` coincide con `foo` y `bar/foo`, pero no con `foo/bar`.
  - Si el glob termina con `/**`, entonces coincide con todas las subentradas.
    Por ejemplo, `foo/**` coincide con `foo/a` y `foo/a/b`, pero no con `foo`.
  - Si el glob contiene `/**/` en cualquier lugar dentro del patrón, entonces coincide con cero o más directorios.
  - Usar `**` en cualquier otro lugar es ilegal.
  - El glob `**` está permitido y significa "coincidir con todo".
- `{a,b}` coincide con `a` o `b`, donde `a` y `b` son patrones de glob arbitrarios. (N.B. Anidar `{...}` no está permitido actualmente.)
- `[ab]` coincide con `a` o `b`, donde `a` y `b` son caracteres.
- `[!ab]` para coincidir con cualquier carácter excepto `a` y `b`.
- Los metacaracteres como `*` y `?` se pueden escapar con la notación de clase de caracteres, por ejemplo, `[*]` coincide con `*`.

Por ejemplo, el siguiente patrón coincidirá con todos los archivos `.js` en el directorio `js`:

```rust
use ic_http_certification::StatusCode;
use ic_asset_certification::AssetConfig;

let config = AssetConfig::Pattern {
    pattern: "js/*.js".to_string(),
    content_type: Some("application/javascript".to_string()),
    headers: vec![
        ("Cache-Control".to_string(), "public, max-age=31536000, immutable".to_string()),
    ],
    encodings: vec![
        AssetEncoding::Brotli.default(),
        AssetEncoding::Gzip.default(),
    ],
};
```

### Configurar redirecciones

Las redirecciones se pueden configurar utilizando la variante `AssetConfig::Redirect`. Esta variante toma las rutas `from` y `to`, y un tipo de redirección `kind`.
Cuando se realiza una solicitud a la ruta `from`, el cliente será redirigido a la ruta `to`. La configuración `AssetConfig::Redirect` no se compara con ningún `Asset`.

Las redirecciones se pueden configurar como permanentes o temporales.

El navegador almacenará en caché las redirecciones permanentes y no solicitará la ubicación anterior nuevamente. Esto es útil cuando el recurso se ha movido permanentemente a una nueva ubicación. El navegador actualizará sus marcadores y resultados de búsqueda en los motores de búsqueda.

Consulte la documentación de [MDN Web Docs](https://developer.mozilla.org/es/docs/Web/HTTP/Status/301) para obtener más información sobre las redirecciones permanentes.

El navegador no almacenará en caché las redirecciones temporales y solicitará la ubicación anterior nuevamente. Esto es útil cuando el recurso se ha movido temporalmente a una nueva ubicación. El navegador no actualizará sus marcadores y resultados de búsqueda en los motores de búsqueda.

Consulte la documentación de [MDN Web Docs](https://developer.mozilla.org/es/docs/Web/HTTP/Status/307) para obtener más información sobre las redirecciones temporales.

El siguiente ejemplo configura una redirección permanente desde `/old` a `/new`:

```rust
use ic_asset_certification::{AssetConfig, AssetRedirectKind};

let config = AssetConfig::Redirect {
    from: "/old".to_string(),
    to: "/new".to_string(),
    kind: AssetRedirectKind::Permanent,
    headers: vec![(
        "content-type".to_string(),
        "text/plain; charset=utf-8".to_string(),
    )],
};
```

## Insertar assets en el enrutador de assets

El `AssetRouter` es responsable de certificar respuestas y enrutar solicitudes a
la respuesta adecuada.

Los assets pueden insertarse usando el método `certify_assets`:

```rust
use ic_http_certification::StatusCode;
use ic_asset_certification::{Asset, AssetConfig, AssetFallbackConfig, AssetRouter, AssetRedirectKind};

let mut asset_router = AssetRouter::default();

let assets = vec![
    Asset::new(
        "index.html",
        b"<html><body><h1>Hello World!</h1></body></html>".as_slice(),
    ),
    Asset::new(
        "index.html.gz",
        [0, 1, 2, 3, 4, 5]
    ),
    Asset::new(
        "index.html.br",
        [6, 7, 8, 9, 10, 11]
    ),
    Asset::new(
        "app.js",
        b"console.log('Hello World!');".as_slice(),
    ),
    Asset::new(
        "app.js.gz",
        [12, 13, 14, 15, 16, 17],
    ),
    Asset::new(
        "app.js.br",
        [18, 19, 20, 21, 22, 23],
    ),
    Asset::new(
      "css/app-ba74b708.css",
      b"html,body{min-height:100vh;}".as_slice(),
    ),
    Asset::new(
        "css/app-ba74b708.css.gz",
        [24, 25, 26, 27, 28, 29],
    ),
    Asset::new(
        "css/app-ba74b708.css.br",
        [30, 31, 32, 33, 34, 35],
    ),
];

let asset_configs = vec![
    AssetConfig::File {
        path: "index.html".to_string(),
        content_type: Some("text/html".to_string()),
        headers: vec![(
            "cache-control".to_string(),
            "public, no-cache, no-store".to_string(),
        )],
        fallback_for: vec![AssetFallbackConfig {
            scope: "/".to_string(),
            status_code: Some(StatusCode::OK),
        }],
        aliased_by: vec!["/".to_string()],
        encodings: vec![
            AssetEncoding::Brotli.default(),
            AssetEncoding::Gzip.default(),
        ],
    },
    AssetConfig::Pattern {
        pattern: "**/*.js".to_string(),
        content_type: Some("text/javascript".to_string()),
        headers: vec![(
            "cache-control".to_string(),
            "public, max-age=31536000, immutable".to_string(),
        )],
        encodings: vec![
            AssetEncoding::Brotli.default(),
            AssetEncoding::Gzip.default(),
        ],
    },
    AssetConfig::Pattern {
        pattern: "**/*.css".to_string(),
        content_type: Some("text/css".to_string()),
        headers: vec![(
            "cache-control".to_string(),
            "public, max-age=31536000, immutable".to_string(),
        )],
        encodings: vec![
            AssetEncoding::Brotli.default(),
            AssetEncoding::Gzip.default(),
        ],
    },
    AssetConfig::Redirect {
        from: "/old".to_string(),
        to: "/new".to_string(),
        kind: AssetRedirectKind::Permanent,
        headers: vec![(
            "content-type".to_string(),
            "text/plain; charset=utf-8".to_string(),
        )],
    },
];

asset_router.certify_assets(assets, asset_configs).unwrap();
```

Después de certificar los assets, asegúrate de establecer los datos certificados
del canister:

```rust
use ic_cdk::api::set_certified_data;

set_certified_data(&asset_router.root_hash());
```

Después de crear el `AssetRouter`, también es posible inicializar el enrutador
con un `HttpCertificationTree`. Esto es útil cuando se requiere acceso directo
al `HttpCertificationTree` para certificar `HttpRequest`s y `HttpResponse`s
fuera del `AssetRouter`.

La inicialización del `AssetRouter` debe realizarse antes de certificar
cualquier asset, ya que la función de inicialización restablecerá el estado
interno del `AssetRouter`.

```rust
use std::{cell::RefCell, rc::Rc};
use ic_http_certification::HttpCertificationTree;
use ic_asset_certification::AssetRouter;

let mut http_certification_tree: Rc<RefCell<HttpCertificationTree>> = Default::default();
let mut asset_router = AssetRouter::default();

asset_router.init_with_tree(http_certification_tree.clone());
```

## Servir assets

Los assets pueden servirse llamando al método `serve_asset` en el `AssetRouter`.
Este método devolverá una respuesta, un testigo y una ruta de expresión, que
pueden utilizarse junto con el certificado de datos del canister para agregar el
encabezado de certificado requerido a la respuesta.

```rust
use ic_http_certification::{HttpRequest, utils::add_v2_certificate_header, StatusCode};
use ic_asset_certification::{Asset, AssetConfig, AssetFallbackConfig, AssetRouter};

let mut asset_router = AssetRouter::default();

let asset = Asset::new(
    "index.html",
    b"<html><body><h1>Hello World!</h1></body></html>".as_slice(),
);

let asset_config = AssetConfig::File {
    path: "index.html".to_string(),
    content_type: Some("text/html".to_string()),
    headers: vec![
        ("Cache-Control".to_string(), "public, no-cache, no-store".to_string()),
    ],
    fallback_for: vec![AssetFallbackConfig {
        scope: "/".to_string(),
        status_code: Some(StatusCode::OK),
    }],
    aliased_by: vec!["/".to_string()],
    encodings: vec![],
};

let http_request = HttpRequest::get("/").build();

asset_router.certify_assets(vec![asset], vec![asset_config]).unwrap();

let (mut response, witness, expr_path) = asset_router.serve_asset(&http_request).unwrap();

// Normalmente, esto debería obtenerse usando `ic_cdk::api::data_certificate()`.
let data_certificate = vec![1, 2, 3];

add_v2_certificate_header(
    data_certificate,
    &mut response,
    &witness,
    &expr_path,
);
```

## Eliminar assets

Hay tres formas de eliminar assets del enrutador de assets:

1. [Por configuración](#deleting-assets-by-configuration).
2. [Por ruta](#deleting-assets-by-path).
3. [Todos a la vez](#deleting-all-assets).

### Eliminar assets por configuración

Eliminar assets por configuración es similar a
[certificarlos](#inserting-assets-into-the-asset-router).

Dependiendo de la configuración proporcionada a la función `certify_assets`,
pueden generarse múltiples respuestas para el mismo asset. Para asegurarse de
que todas las respuestas generadas sean eliminadas, la función `delete_assets`
acepta la misma configuración.

Si se proporciona una configuración diferente a la utilizada originalmente para
certificar los assets, pueden ocurrir dos cosas:

1. Si la configuración incluye un archivo que no fue certificado, será ignorado
   silenciosamente. Por ejemplo, si la configuración proporcionada a
   `certify_assets` incluye las codificaciones Brotli y Gzip, pero la
   configuración proporcionada a `delete_assets` incluye Brotli, Gzip y Deflate.
   Los archivos codificados en Brotli y Gzip serán eliminados, mientras que el
   archivo Deflate será ignorado, ya que no existe.

2. Si la configuración excluye un archivo que fue certificado, este no será
   eliminado. Por ejemplo, si la configuración proporcionada a `certify_assets`
   incluye las codificaciones Brotli y Gzip, pero la configuración proporcionada
   a `delete_assets` solo incluye Brotli, entonces el archivo Gzip no será
   eliminado.

Asumiendo el mismo ejemplo base utilizado anteriormente para demostrar la
certificación de assets:

```rust
use ic_http_certification::StatusCode;
use ic_asset_certification::{Asset, AssetConfig, AssetFallbackConfig, AssetRouter, AssetRedirectKind, AssetEncoding};

let mut asset_router = AssetRouter::default();

let assets = vec![
    Asset::new(
        "index.html",
        b"<html><body><h1>Hello World!</h1></body></html>".as_slice(),
    ),
    Asset::new(
        "index.html.gz",
        &[0, 1, 2, 3, 4, 5]
    ),
    Asset::new(
        "index.html.br",
        &[6, 7, 8, 9, 10, 11]
    ),
    Asset::new(
        "app.js",
        b"console.log('Hello World!');".as_slice(),
    ),
    Asset::new(
        "app.js.gz",
        &[12, 13, 14, 15, 16, 17],
    ),
    Asset::new(
        "app.js.br",
        &[18, 19, 20, 21, 22, 23],
    ),
    Asset::new(
        "css/app-ba74b708.css",
        b"html,body{min-height:100vh;}".as_slice(),
    ),
    Asset::new(
        "css/app-ba74b708.css.gz",
        &[24, 25, 26, 27, 28, 29],
    ),
    Asset::new(
        "css/app-ba74b708.css.br",
        &[30, 31, 32, 33, 34, 35],
    ),
];

let asset_configs = vec![
    AssetConfig::File {
        path: "index.html".to_string(),
        content_type: Some("text/html".to_string()),
        headers: vec![(
            "cache-control".to_string(),
            "public, no-cache, no-store".to_string(),
        )],
        fallback_for: vec![AssetFallbackConfig {
            scope: "/".to_string(),
            status_code: Some(StatusCode::OK),
        }],
        aliased_by: vec!["/".to_string()],
        encodings: vec![
            AssetEncoding::Brotli.default_config(),
            AssetEncoding::Gzip.default_config(),
        ],
    },
    AssetConfig::Pattern {
        pattern: "**/*.js".to_string(),
        content_type: Some("text/javascript".to_string()),
        headers: vec![(
            "cache-control".to_string(),
            "public, max-age=31536000, immutable".to_string(),
        )],
        encodings: vec![
            AssetEncoding::Brotli.default_config(),
            AssetEncoding::Gzip.default_config(),
        ],
    },
    AssetConfig::Pattern {
        pattern: "**/*.css".to_string(),
        content_type: Some("text/css".to_string()),
        headers: vec![(
            "cache-control".to_string(),
            "public, max-age=31536000, immutable".to_string(),
        )],
        encodings: vec![
            AssetEncoding::Brotli.default_config(),
            AssetEncoding::Gzip.default_config(),
        ],
    },
    AssetConfig::Redirect {
        from: "/old".to_string(),
        to: "/new".to_string(),
        kind: AssetRedirectKind::Permanent,
        headers: vec![(
            "content-type".to_string(),
            "text/plain; charset=utf-8".to_string(),
        )],
    },
];

asset_router.certify_assets(assets, asset_configs).unwrap();
```

Para eliminar el asset `index.html`, junto con la configuración de fallback para
el alcance `/`, el alias `/` y las codificaciones alternativas:

```rust
asset_router
    .delete_assets(
        vec![
            Asset::new(
                "index.html",
                b"<html><body><h1>Hello World!</h1></body></html>".as_slice(),
            ),
            Asset::new("index.html.gz", &[0, 1, 2, 3, 4, 5]),
            Asset::new("index.html.br", &[6, 7, 8, 9, 10, 11]),
        ],
        vec![AssetConfig::File {
            path: "index.html".to_string(),
            content_type: Some("text/html".to_string()),
            headers: vec![(
                "cache-control".to_string(),
                "public, no-cache, no-store".to_string(),
            )],
            fallback_for: vec![AssetFallbackConfig {
                scope: "/".to_string(),
                status_code: Some(StatusCode::OK),
            }],
            aliased_by: vec!["/".to_string()],
            encodings: vec![
                AssetEncoding::Brotli.default_config(),
                AssetEncoding::Gzip.default_config(),
            ],
        }],
    )
    .unwrap();
```

Para eliminar el asset `app.js`, junto con las codificaciones alternativas:

```rust
asset_router
    .delete_assets(
        vec![
            Asset::new("app.js", b"console.log('Hello World!');".as_slice()),
            Asset::new("app.js.gz", &[12, 13, 14, 15, 16, 17]),
            Asset::new("app.js.br", &[18, 19, 20, 21, 22, 23]),
        ],
        vec![AssetConfig::Pattern {
            pattern: "**/*.js".to_string(),
            content_type: Some("text/javascript".to_string()),
            headers: vec![(
                "cache-control".to_string(),
                "public, max-age=31536000, immutable".to_string(),
            )],
            encodings: vec![
                AssetEncoding::Brotli.default_config(),
                AssetEncoding::Gzip.default_config(),
            ],
        }],
    )
    .unwrap();
```

Para eliminar el asset `css/app-ba74b708.css`, junto con las codificaciones
alternativas:

```rust
asset_router.delete_assets(
    vec![
        Asset::new(
            "css/app-ba74b708.css",
            b"html,body{min-height:100vh;}".as_slice(),
        ),
        Asset::new(
            "css/app-ba74b708.css.gz",
            &[24, 25, 26, 27, 28, 29],
        ),
        Asset::new(
            "css/app-ba74b708.css.br",
            &[30, 31, 32, 33, 34, 35],
        ),
    ],
    vec![
        AssetConfig::Pattern {
            pattern: "**/*.css".to_string(),
            content_type: Some("text/css".to_string()),
            headers: vec![(
                "cache-control".to_string(),
                "public, max-age=31536000, immutable".to_string(),
            )],
            encodings: vec![
                AssetEncoding::Brotli.default_config(),
                AssetEncoding::Gzip.default_config(),
            ],
        },
    ]
).unwrap();
```

Y finalmente, para eliminar la redirección `/old`:

```rust
asset_router
    .delete_assets(
        vec![],
        vec![AssetConfig::Redirect {
            from: "/old".to_string(),
            to: "/new".to_string(),
            kind: AssetRedirectKind::Permanent,
            headers: vec![(
                "content-type".to_string(),
                "text/plain; charset=utf-8".to_string(),
            )],
        }],
    )
    .unwrap();
```

Luego de eliminar cualquier asset, asegúrate de establecer los datos
certificados del canister:

```rust
use ic_cdk::api::set_certified_data;

set_certified_data(&asset_router.root_hash());
```

### Eliminando assets por ruta

Para eliminar assets por ruta, utiliza la función `delete_assets_by_path`.

Dependiendo de la configuración proporcionada a la función `certify_assets`, se
pueden generar múltiples respuestas para el mismo asset. Estos assets pueden
existir en diferentes rutas, por ejemplo, si se utiliza la configuración de
`alias`. Si no se pasan las rutas de `alias` a esta función, no se eliminarán.

Si existen múltiples codificaciones para una ruta, se eliminarán todas las
codificaciones.

Las alternativas tampoco se eliminan; para eliminarlas, utiliza la función
`delete_fallback_assets_by_path`.

Asumiendo el mismo ejemplo base utilizado anteriormente para demostrar la
certificación de assets:

```rust
use ic_http_certification::StatusCode;
use ic_asset_certification::{Asset, AssetConfig, AssetFallbackConfig, AssetRouter, AssetRedirectKind, AssetEncoding};

let mut asset_router = AssetRouter::default();

let assets = vec![
    Asset::new(
        "index.html",
        b"<html><body><h1>Hello World!</h1></body></html>".as_slice(),
    ),
    Asset::new(
        "index.html.gz",
        &[0, 1, 2, 3, 4, 5]
    ),
    Asset::new(
        "index.html.br",
        &[6, 7, 8, 9, 10, 11]
    ),
    Asset::new(
        "app.js",
        b"console.log('Hello World!');".as_slice(),
    ),
    Asset::new(
        "app.js.gz",
        &[12, 13, 14, 15, 16, 17],
    ),
    Asset::new(
        "app.js.br",
        &[18, 19, 20, 21, 22, 23],
    ),
    Asset::new(
        "css/app-ba74b708.css",
        b"html,body{min-height:100vh;}".as_slice(),
    ),
    Asset::new(
        "css/app-ba74b708.css.gz",
        &[24, 25, 26, 27, 28, 29],
    ),
    Asset::new(
        "css/app-ba74b708.css.br",
        &[30, 31, 32, 33, 34, 35],
    ),
];

let asset_configs = vec![
    AssetConfig::File {
        path: "index.html".to_string(),
        content_type: Some("text/html".to_string()),
        headers: vec![(
            "cache-control".to_string(),
            "public, no-cache, no-store".to_string(),
        )],
        fallback_for: vec![AssetFallbackConfig {
            scope: "/".to_string(),
            status_code: Some(StatusCode::OK),
        }],
        aliased_by: vec!["/".to_string()],
        encodings: vec![
            AssetEncoding::Brotli.default_config(),
            AssetEncoding::Gzip.default_config(),
        ],
    },
    AssetConfig::Pattern {
        pattern: "**/*.js".to_string(),
        content_type: Some("text/javascript".to_string()),
        headers: vec![(
            "cache-control".to_string(),
            "public, max-age=31536000, immutable".to_string(),
        )],
        encodings: vec![
            AssetEncoding::Brotli.default_config(),
            AssetEncoding::Gzip.default_config(),
        ],
    },
    AssetConfig::Pattern {
        pattern: "**/*.css".to_string(),
        content_type: Some("text/css".to_string()),
        headers: vec![(
            "cache-control".to_string(),
            "public, max-age=31536000, immutable".to_string(),
        )],
        encodings: vec![
            AssetEncoding::Brotli.default_config(),
            AssetEncoding::Gzip.default_config(),
        ],
    },
    AssetConfig::Redirect {
        from: "/old".to_string(),
        to: "/new".to_string(),
        kind: AssetRedirectKind::Permanent,
        headers: vec![("content-type".to_string(), "text/plain".to_string())],
    },
];

asset_router.certify_assets(assets, asset_configs).unwrap();
```

Para eliminar el asset `index.html`, junto con la configuración de fallback para
el alcance `/`, el alias `/` y las codificaciones alternativas:

```rust
asset_router
    .delete_assets_by_path(
        vec![
            "/index.html", // deletes the index.html asset, along with all encodings
            "/" // deletes the `/` alias for index.html, along with all encodings
        ],
    )
    .unwrap();

asset_router
    .delete_fallback_assets_by_path(
       vec![
          "/" // deletes the fallback configuration for the `/` scope, along with all encodings
      ]
   )
  .unwrap();
```

Para eliminar el asset `app.js`, junto con las codificaciones alternativas:

```rust
asset_router.delete_assets(vec!["/app.js"]).unwrap();
```

Para eliminar el asset `css/app-ba74b708.css`, junto con las codificaciones
alternativas:

```rust
asset_router.delete_assets(vec!["/css/app-ba74b708.css"]).unwrap();
```

Y finalmente, para eliminar la redirección `/old`:

```rust
asset_router.delete_assets_by_path(vec!["/old"]).unwrap();
```

Luego de eliminar cualquier asset, asegúrate de establecer los datos
certificados del canister:

```rust
use ic_cdk::api::set_certified_data;

set_certified_data(&asset_router.root_hash());
```

### Eliminando todos los assets

También es posible eliminar todos los assets y su certificación de una vez:

```rust
asset_router.delete_all_assets();
```

Luego de eliminar cualquier asset, asegúrate de establecer los datos
certificados del canister:

```rust
use ic_cdk::api::set_certified_data;

set_certified_data(&asset_router.root_hash());
```

## Consultando assets

El `AssetRouter` tiene dos funciones para obtener un `AssetMap` que contiene
assets.

La función `get_assets()` devuelve todos los assets estándar, mientras que la
función `get_fallback_assets()` devuelve todos los assets de fallback.

El `AssetMap` se puede utilizar para consultar assets por `ruta`, `codificación`
y `rango inicial`.

Para los assets estándar, la ruta se refiere a la ruta del asset, por ejemplo,
`/index.html`.

Para los assets de fallback, la ruta se refiere al alcance para el cual se
aplica el fallback, por ejemplo, `/`. Consulta la opción de configuración
`fallback_for` para obtener más información sobre los alcances de fallback.

Para todos los tipos de assets, la codificación se refiere a la codificación del
asset; consulta `AssetEncoding`.

Los assets mayores a 2 MiB se dividen en múltiples rangos; el rango inicial
permite recuperar fragmentos individuales de estos assets grandes. El primer
rango es `Some(0)`, el segundo rango es `Some(ASSET_CHUNK_SIZE)`, el tercer
rango es `Some(ASSET_CHUNK_SIZE * 2)`, y así sucesivamente. El asset completo
también se puede recuperar pasando `None` como el `starting_range`. Ten en
cuenta que `ASSET_CHUNK_SIZE` es una constante definida en el paquete
`ic_asset_certification`.
