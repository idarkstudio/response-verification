# Certificación HTTP

La certificación HTTP es un subprotocolo del
[ICP](https://internetcomputer.org/)
[protocolo de puerta de enlace HTTP](https://internetcomputer.org/docs/current/references/http-gateway-protocol-spec).
Se utiliza para verificar las respuestas HTTP recibidas por una puerta de enlace
HTTP desde un
[canister](https://internetcomputer.org/how-it-works/canister-lifecycle/), con
respecto a la solicitud HTTP correspondiente. Esto permite a las puertas de
enlace HTTP verificar que las respuestas que reciben de los canisters son
auténticas y no han sido manipuladas.

La biblioteca `ic-http-certification` proporciona la base para implementar el
protocolo de certificación HTTP en canisters de Rust. La certificación se
implementa en varios pasos:

1. [Definición de expresiones CEL](#defining-cel-expressions).
2. [Creación de certificaciones](#creating-certifications).
3. [Creación de un árbol de certificación HTTP](#creating-an-http-certification-tree).

## Definición de expresiones CEL

[CEL](https://github.com/google/cel-spec) (Common Expression Language) es un
lenguaje de expresión portátil que se puede utilizar para diferentes
aplicaciones interoperar fácilmente. Se puede ver como la contraparte
computacional o de expresión a
[protocol buffers](https://github.com/protocolbuffers/protobuf).

Las expresiones CEL son el núcleo del protocolo de certificación HTTP de ICP. Se
utilizan para definir las condiciones bajo las cuales se debe certificar un par
de solicitud y respuesta, así como lo que se debe incluir de la solicitud y
objetos de respuesta correspondientes en la certificación.

Las expresiones CEL se pueden crear de dos formas:

- Utilizando el [constructor CEL](#using-the-cel-builder).
- Creando directamente una [expresión CEL](#directly-creating-a-cel-expression).

### Conversión de expresiones CEL a su representación `String`

Tenga en cuenta que la enumeración `CelExpression` no es una expresión CEL en sí
misma, sino más bien una representación de Rust de una expresión CEL. Para
convertir una `CelExpression` en su representación `String`, use
`CelExpression.to_string` o `create_cel_expr`. Esto se aplica a las expresiones
CEL creadas tanto por el [constructor CEL](#using-the-cel-builder) como por
[directamente](#directly-creating-a-cel-expression).

```rust
use ic_http_certification::cel::{CelExpression, DefaultCelExpression};

let cel_expr = CelExpression::Default(DefaultCelExpression::Skip).to_string();
```

Alternativamente:

```rust
use ic_http_certification::cel::{CelExpression, DefaultCelExpression, create_cel_expr};

let certification = CelExpression::Default(DefaultCelExpression::Skip);
let cel_expr = create_cel_expr(&certification);
```

### Uso del constructor CEL

La interfaz del constructor CEL se proporciona para facilitar la creación de
expresiones CEL a través de una interfaz ergonómica. También es posible
[crear expresiones CEL directamente](#directly-creating-a-cel-expression). Para
definir una expresión CEL, comience con `DefaultCelBuilder`. Esta estructura
proporciona un conjunto de funciones asociadas que se pueden utilizar para
definir cómo se debe certificar un par de solicitud y respuesta.

Es posible

- [Certificar completamente solicitudes y respuestas](#fully-certified-request--response-pair).
- [Certificar parcialmente las solicitudes](#partially-certified-request).
- [Omitir la certificación de la solicitud](#skipping-request-certification).
- [Certificar parcialmente las respuestas](#partially-certified-response).
- [Omitir la certificación por completo](#skipping-certification).

Tenga en cuenta que si se certifica la solicitud, también se debe certificar la
respuesta. No es posible certificar una solicitud sin certificar también una
respuesta. Se pueden utilizar cualquier combinación de solicitudes y respuestas
completamente o parcialmente certificadas.

Cuando se certifica una solicitud:

- El cuerpo y el método de la solicitud siempre se certifican.
- Las cabeceras de la solicitud y los parámetros de consulta se pueden
  certificar opcionalmente utilizando las funciones asociadas
  `with_request_headers` y `with_request_query_parameters`, respectivamente.
  Ambas funciones asociadas toman una cadena `str` como argumento.

Cuando se certifica una respuesta:

- El cuerpo y el código de estado de la respuesta siempre se certifican.
- Las cabeceras de respuesta se pueden certificar opcionalmente utilizando la
  función asociada `with_response_certification`. Esta función toma la
  enumeración `DefaultResponseCertification` como argumento.   - Para
  especificar las inclusiones de cabecera, use la función asociada
  `certified_response_headers` de la enumeración `DefaultResponseCertification`.
    - Para certificar todas las cabeceras de respuesta (con algunas exclusiones
  opcionales), use la función asociada `response_header_exclusions` de la
  enumeración `DefaultResponseCertification`. Ambas funciones toman una cadena
  `str` como argumento.

Independientemente de lo que se incluya en la certificación, la ruta de la
solicitud siempre se utiliza para determinar si se debe utilizar esa
certificación. También es posible establecer una certificación para un "ámbito"
o "directorio" de rutas; consulte
[Definición de rutas de árbol](#defining-tree-paths) para obtener más
información al respecto.

Al definir expresiones CEL, es importante determinar qué se debe certificar y
qué se puede excluir de la certificación. Por ejemplo, si una cabecera de
respuesta no está certificada, no se incluirá en la certificación y no será
verificada por la puerta de enlace HTTP, lo que significa que el valor de esta
cabecera no se puede confiar por los clientes. Como regla general, es una buena
idea comenzar con un par de solicitud y respuesta completamente certificado y
luego eliminar partes de la certificación según sea necesario.

Se debe considerar inseguro excluir cualquier cosa de la certificación de la
solicitud que pueda cambiar la respuesta esperada. El método de la solicitud,
por ejemplo, puede afectar drásticamente qué acción toma el canister, por lo que
excluirlo de la certificación permitiría a una réplica malintencionada responder
con las respuestas esperadas para una solicitud `'GET'`, aunque se haya
realizado una solicitud `'POST'`.

Para las respuestas, se debe considerar inseguro excluir cualquier cosa de la
certificación de la respuesta que los clientes utilicen de manera significativa.
Por ejemplo, excluir la cabecera `Content-Type` de la certificación permitiría a
una réplica malintencionada responder con un tipo de contenido diferente al
esperado, lo que podría causar que los clientes interpreten incorrectamente la
respuesta.

#### Par de solicitud / respuesta completamente certificado

Para definir un par de solicitud y respuesta completamente certificado,
incluyendo la solicitud cabeceras, parámetros de consulta y cabeceras de
respuesta:

```rust
use ic_http_certification::{DefaultCelBuilder, DefaultResponseCertification};

let cel_expr = DefaultCelBuilder::full_certification()
    .with_request_headers(vec!["Accept", "Accept-Encoding", "If-None-Match"])
    .with_request_query_parameters(vec!["foo", "bar", "baz"])
    .with_response_certification(DefaultResponseCertification::certified_response_headers(vec![
        "Cache-Control",
        "ETag",
    ]))
    .build();
```

#### Solicitud parcialmente certificada

Se pueden certificar cualquier número de cabeceras de solicitud o parámetros de
consulta mediante las funciones asociadas `with_request_headers` y
`with_request_query_parameters`, respectivamente. Ambos métodos aceptarán
matrices vacías, lo que es lo mismo que no llamarlos en absoluto. Del mismo
modo, para `with_request_query_parameters`, si se llama con una matriz vacía o
no se llama en absoluto, entonces no se certificarán parámetros de consulta. Si
ambos se llaman con una matriz vacía, o ninguno se llama, entonces solo el
cuerpo y el método de la solicitud se certificarán, además de la respuesta. Como
recordatorio, la respuesta siempre se certifica al menos parcialmente si la
solicitud está certificada.

Por ejemplo, para certificar solo el cuerpo y el método de la solicitud, además
de la respuesta:

```rust
use ic_http_certification::{DefaultCelBuilder, DefaultResponseCertification};

let cel_expr = DefaultCelBuilder::full_certification()
    .with_response_certification(DefaultResponseCertification::certified_response_headers(vec![
        "Cache-Control",
        "ETag",
    ]))
    .build();
```

Alternativamente, esto se puede hacer de manera más explícita:

```rust
use ic_http_certification::{DefaultCelBuilder, DefaultResponseCertification};

let cel_expr = DefaultCelBuilder::full_certification()
    .with_request_headers(vec![])
    .with_request_query_parameters(vec![])
    .with_response_certification(DefaultResponseCertification::certified_response_headers(vec![
        "Cache-Control",
        "ETag",
    ]))
    .build();
```

#### Omitir la certificación de la solicitud

La certificación de la solicitud se puede omitir por completo utilizando
`DefaultCelBuilder::response_only_certification` en lugar de
`DefaultCelBuilder::full_certification`. La certificación de la solicitud solo
debe omitirse si la respuesta se determina únicamente por la ruta de la
solicitud. Si cualquier otra parte de la solicitud puede afectar la respuesta de
manera significativa, entonces no se debe omitir la certificación de la
solicitud.

Por ejemplo:

```rust
use ic_http_certification::{DefaultCelBuilder, DefaultResponseCertification};

let cel_expr = DefaultCelBuilder::response_only_certification()
    .with_response_certification(DefaultResponseCertification::response_header_exclusions(vec![
        "Date",
        "Cookie",
        "Set-Cookie",
    ]))
    .build();
```

#### Respuesta parcialmente certificada

Se pueden proporcionar cualquier número de cabeceras de respuesta mediante la
función asociada `certified_response_headers` de la enumeración
`DefaultResponseCertification` al llamar `with_response_certification`. La
matriz proporcionada también puede estar vacía. Si la matriz está vacía, o no se
llama a la función asociada, no se certificarán cabeceras de respuesta. Si se
certificarán todas las cabeceras de respuesta, con algunas exclusiones, use la
función asociada `response_header_exclusions` de la enumeración
`DefaultResponseCertification`. Se debe tener cuidado al elegir qué cabeceras
excluir de la certificación, ya que no se verificarán por la puerta de enlace
HTTP. Cualquier cabecera que contenga información significativa para los
clientes no debe ser excluida.

Por ejemplo, para certificar solo el cuerpo y el código de estado de la
respuesta:

```rust
use ic_http_certification::DefaultCelBuilder;

let cel_expr = DefaultCelBuilder::response_only_certification().build();
```

Esto también se puede hacer de manera más explícita:

````rust
`DefaultResponseCertification` enum when calling `with_response_certification`.
The provided array can also be empty. If the array is empty, or the associated
function is not called, no response headers will be certified. If all response
headers are to be certified, with some exclusions, use the
`response_header_exclusions` associated function of the
`DefaultResponseCertification` enum. Care should be taken when choosing what
headers to exclude from certification, as they will not be verified by the HTTP
gateway. Any headers that hold meaningful information for clients should not be
excluded.

# Certificación HTTP

Por ejemplo, para certificar solo el cuerpo de la respuesta y el código de estado:

```rust
use ic_http_certification::DefaultCelBuilder;

let cel_expr = DefaultCelBuilder::response_only_certification().build();
````

Esto también se puede hacer de manera más explícita:

```rust
use ic_http_certification::{DefaultCelBuilder, DefaultResponseCertification};

let cel_expr = DefaultCelBuilder::response_only_certification()
  .with_response_certification(DefaultResponseCertification::certified_response_headers(vec![]))
  .build();
```

Lo mismo se aplica al usar `DefaultCelBuilder::response_only_certification` y
`DefaultCelBuilder::full_certification`:

```rust
use ic_http_certification::DefaultCelBuilder;

let cel_expr = DefaultCelBuilder::full_certification()
  .with_request_headers(vec!["Accept", "Accept-Encoding", "If-None-Match"])
  .with_request_query_parameters(vec!["foo", "bar", "baz"])
  .build();
```

Para omitir completamente la certificación de la respuesta, se debe omitir
completamente la certificación en general. No sería útil certificar una
solicitud sin certificar una respuesta.

#### Omitir la certificación

Para omitir completamente la certificación, usa `skip_certification`, por
ejemplo:

```rust
use ic_http_certification::DefaultCelBuilder;

let cel_expr = DefaultCelBuilder::skip_certification();
```

Omitir la certificación puede parecer contraintuitivo al principio, pero no
siempre es posible certificar un par de solicitud y respuesta. Por ejemplo, un
método de canister que devolverá datos diferentes para cada usuario no se puede
certificar fácilmente.

Normalmente, estas solicitudes se han enrutado a través de URL de ICP `raw` en
el pasado, pero esto es peligroso porque las URL `raw` permiten que cualquier
réplica que responda decida si se requiere o no la certificación. En cambio, al
omitir la certificación usando el método anterior con una URL no `raw`, una
réplica ya no podrá decidir si se requiere o no la certificación y en su lugar
esta decisión se tomará por el propio canister y el resultado pasará por
consenso.

Se debe tener extrema precaución al decidir omitir completamente la
certificación. Solo debe hacerse cuando no sea posible certificar un par de
solicitud y respuesta, y una modificación del contenido de la respuesta no
representaría un riesgo de seguridad para la aplicación.

## Creación de certificaciones

Una vez que se ha definido una expresión CEL, se puede utilizar junto con una
`HttpRequest` y `HttpResponse` para crear una instancia de la estructura
`HttpCertification`. La estructura `HttpCertification` tiene tres funciones
asociadas:

- La función asociada `full` se utiliza para incluir tanto la `HttpRequest` como
  la correspondiente `HttpResponse` en la certificación.
- La función asociada `response_only` se utiliza para incluir solo la
  `HttpResponse` en la certificación y excluir la correspondiente `HttpRequest`
  de la certificación.
- La función asociada `skip` se utiliza para omitir completamente la
  certificación.

### Certificación completa

Para realizar una certificación completa, se requiere una expresión CEL creada a
partir de `DefaultCelBuilder::full_certification`, junto con una `HttpRequest` y
`HttpResponse`, y opcionalmente, un hash del cuerpo de respuesta precalculado.

Por ejemplo:

```rust
use ic_http_certification::{HttpCertification, HttpRequest, HttpResponse, DefaultCelBuilder, DefaultResponseCertification};

let cel_expr = DefaultCelBuilder::full_certification()
  .with_request_headers(vec!["Accept", "Accept-Encoding", "If-None-Match"])
  .with_request_query_parameters(vec!["foo", "bar", "baz"])
  .with_response_certification(DefaultResponseCertification::certified_response_headers(vec![
    "Cache-Control",
    "ETag",
  ]))
  .build();

let request = HttpRequest {
  method: "GET".to_string(),
  url: "/index.html?foo=a&bar=b&baz=c".to_string(),
  headers: vec![
    ("Accept".to_string(), "application/json".to_string()),
    ("Accept-Encoding".to_string(), "gzip".to_string()),
    ("If-None-Match".to_string(), "987654321".to_string()),
  ],
  body: vec![],
};

let response = HttpResponse {
  status_code: 200,
  headers: vec![
    ("Cache-Control".to_string(), "no-cache".to_string()),
    ("ETag".to_string(), "123456789".to_string()),
    ("IC-CertificateExpression".to_string(), cel_expr.to_string()),
  ],
  body: vec![1, 2, 3, 4, 5, 6],
  upgrade: None,
};

let certification = HttpCertification::full(&cel_expr, &request, &response, None);
```

### Certificación solo de respuesta

Para realizar una certificación solo de respuesta, se requiere una expresión CEL
creada a partir de `DefaultCelBuilder::response_only_certification`, junto con
una `HttpResponse` y, opcionalmente, un hash del cuerpo de respuesta
precalculado.

Por ejemplo:

```rust
use ic_http_certification::{HttpCertification, HttpResponse, DefaultCelBuilder, DefaultResponseCertification};

let cel_expr = DefaultCelBuilder::response_only_certification()
  .with_response_certification(DefaultResponseCertification::certified_response_headers(vec![
    "Cache-Control",
    "ETag",
  ]))
  .build();

let response = HttpResponse {
  status_code: 200,
  headers: vec![
    ("Cache-Control".to_string(), "no-cache".to_string()),
    ("ETag".to_string(), "123456789".to_string()),
    ("IC-CertificateExpression".to_string(), cel_expr.to_string()),
  ],
  body: vec![1, 2, 3, 4, 5, 6],
  upgrade: None,
};

let certification = HttpCertification::response_only(&cel_expr, &response, None).unwrap();
```

### Omitir la certificación

Omitir la certificación no requiere que se defina una expresión CEL explícita ya
que siempre es la misma.

Por ejemplo:

```rust
use ic_http_certification::HttpCertification;

let certification = HttpCertification::skip();
```

## Creación de un árbol de certificación HTTP

### Definir rutas del árbol

Las rutas para el árbol se pueden definir utilizando la estructura
`HttpCertificationPath` y vienen en dos tipos: `wildcard()` y `exact()`. Ambos
tipos de rutas pueden terminar con o sin una barra diagonal al final, pero ten
en cuenta que una ruta que termina con una barra diagonal es una ruta distinta
de una que no termina con una barra diagonal, y se tratarán como tales en el
árbol.

Las rutas de comodín se pueden utilizar para hacer coincidir una subruta de una
URL de solicitud. Esto puede ser útil para respuestas 404, fallbacks o
reescrituras. Se definen utilizando la función asociada `wildcard()`.

En este ejemplo, la certificación ingresada en el árbol con esta ruta será
válida para cualquier URL de solicitud que comience con `/js`, a menos que haya
una ruta más específica en el árbol (por ejemplo, `/js/example.js`).

```rust
use ic_http_certification::HttpCertificationPath;

let path = HttpCertificationPath::wildcard("/js");
```

Las rutas exactas se utilizan para hacer coincidir una URL de solicitud
completa. Una ruta exacta que termina con una barra diagonal se refiere a un
directorio del sistema de archivos, mientras que una sin una barra diagonal se
refiere a un archivo individual. Ambas son rutas separadas dentro del árbol de
certificación y se tratarán completamente de forma independiente.

En este ejemplo, la certificación ingresada en el árbol con esta ruta solo será
válida para una URL de solicitud que sea exactamente `/js/example.js`.

```rust
use ic_http_certification::HttpCertificationPath;

let path = HttpCertificationPath::exact("/js/example.js");
```

### Uso del árbol de certificación HTTP

El `HttpCertificationTree` se puede inicializar fácilmente con el trait
`Default`, y se pueden agregar, eliminar o generar testigos para las entradas
del árbol utilizando la estructura `HttpCertificationTreeEntry`. La
`HttpCertificationTreeEntry` requiere una `HttpCertification` y una
`HttpCertificationPath`.

Por ejemplo:

```rust
use ic_http_certification::{HttpCertification, HttpRequest, HttpResponse, DefaultCelBuilder, DefaultResponseCertification, HttpCertificationTree, HttpCertificationTreeEntry, HttpCertificationPath};

let cel_expr = DefaultCelBuilder::full_certification()
  .with_request_headers(vec!["Accept", "Accept-Encoding", "If-None-Match"])
  .with_request_query_parameters(vec!["foo", "bar", "baz"])
  .with_response_certification(DefaultResponseCertification::certified_response_headers(vec![
    "Cache-Control",
    "ETag",
  ]))
  .build();

let request = HttpRequest {
  method: "GET".to_string(),
  url: "/index.html?foo=a&bar=b&baz=c".to_string(),
  headers: vec![
    ("Accept".to_string(), "application/json".to_string()),
    ("Accept-Encoding".to_string(), "gzip".to_string()),
    ("If-None-Match".to_string(), "987654321".to_string()),
  ],
  body: vec![],
};

let response = HttpResponse {
  status_code: 200,
  headers: vec![
    ("Cache-Control".to_string(), "no-cache".to_string()),
    ("ETag".to_string(), "123456789".to_string()),
    ("IC-CertificateExpression".to_string(), cel_expr.to_string()),
  ],
  body: vec![1, 2, 3, 4, 5, 6],
  upgrade: None,
};

let request_url = "/example.json";
let path = HttpCertificationPath::exact(request_url);
let certification = HttpCertification::full(&cel_expr, &request, &response, None);

let mut http_certification_tree = HttpCertificationTree::default();

let entry = HttpCertificationTreeEntry::new(&path, &certification);

// insert the entry into the tree
http_certification_tree.insert(&entry);

// generate a witness for this entry in the tree
let witness = http_certification_tree.witness(&entry, request_url);

// delete the entry from the tree
http_certification_tree.delete(&entry);
```

### Manejo de actualizaciones

Las expresiones CEL, las certificaciones, el árbol de certificación y las
correspondientes solicitudes y respuestas no se persisten en las
actualizaciones, de forma predeterminada. Esto significa que si un canister se
actualiza, toda esta información se perderá. Para manejar las actualizaciones de
manera efectiva, toda la lógica de inicialización que se ejecuta en el gancho
`init` del canister también debe ejecutarse en el gancho `post_upgrade`. Esto
asegurará que el árbol de certificación se reinicialice correctamente después de
una actualización. La mayoría de las estructuras de datos, excepto el árbol de
certificación, se pueden persistir utilizando memoria estable, y el árbol de
certificación se puede reinicializar utilizando estos datos persistidos. Se debe
tener cuidado de no exceder el límite de instrucciones del canister al
reinicializar el árbol de certificación, lo cual puede ocurrir fácilmente si el
número de respuestas que se certifican crece mucho. Este caso podría abordarse
en el futuro desarrollando un árbol de certificación compatible con memoria
estable.

### Cambio de datos

Además de inicializar las certificaciones en los ganchos `init` y
`post_upgrade`, si una respuesta cambia durante la vida útil del canister en
respuesta a una llamada `update`, el árbol de certificación debe actualizarse
para reflejar este cambio. Esto se puede hacer eliminando la certificación
antigua del árbol e insertando la nueva certificación. Esto debe hacerse en la
misma llamada `update` en la que se cambia la respuesta para asegurarse de que
el árbol de certificación esté siempre actualizado; de lo contrario, las
llamadas `query` que devuelvan esa respuesta no superarán la verificación.

## Creación directa de una expresión CEL

Para definir una expresión CEL, comienza con la enumeración `CelExpression`.
Esta enumeración proporciona un conjunto de variantes que se pueden utilizar
para definir diferentes tipos de expresiones CEL admitidas por las pasarelas
HTTP de ICP. Actualmente, solo se admite una variante, conocida como la
expresión de certificación "predeterminada", pero se pueden agregar más en el
futuro a medida que el protocolo de certificación HTTP evolucione con el tiempo.

Al certificar solicitudes:

- El cuerpo de la solicitud y el método siempre se certifican.

- Para certificar las cabeceras de la solicitud y los parámetros de consulta,
  utiliza los campos `headers` y `query_parameters` de la estructura
  `DefaultRequestCertification`. Ambos campos toman una lista de `str` como
  argumento.

Al certificar respuestas:

- El cuerpo de la respuesta y el código de estado siempre se certifican.

- Para certificar las cabeceras de respuesta, utiliza la función asociada
  `certified_response_headers` del enum `DefaultResponseCertification`. O para
  certificar todas las cabeceras de respuesta, con algunas exclusiones, utiliza
  la función asociada `response_header_exclusions` del enum
  `DefaultResponseCertification`. Ambas funciones asociadas toman una lista de
  `str` como argumento.

Ten en cuenta que las expresiones CEL de ejemplo proporcionadas a continuación
están formateadas para legibilidad. Las expresiones CEL reales producidas por
`CelExpression::to_string` y `create_cel_expr` están minificadas. La expresión
CEL minificada es preferible porque es más compacta, lo que resulta en un tamaño
de carga más pequeño y un tiempo de evaluación más rápido para la pasarela HTTP
que verifica la certificación, pero también se aceptan las versiones
formateadas.

### Par solicitud / respuesta completamente certificado

Para definir un par de solicitud y respuesta completamente certificado,
incluyendo las cabeceras de solicitud, los parámetros de consulta y las
cabeceras de respuesta:

```rust
use std::borrow::Cow;
use ic_http_certification::cel::{CelExpression, DefaultCelExpression, DefaultFullCelExpression, DefaultRequestCertification, DefaultResponseCertification};

let cel_expr = CelExpression::Default(DefaultCelExpression::Full(
  DefaultFullCelExpression {
  request: DefaultRequestCertification::new(
    vec!["Accept", "Accept-Encoding", "If-None-Match"],
    vec!["foo", "bar", "baz"],
  ),
  response: DefaultResponseCertification::certified_response_headers(vec![
    "ETag",
    "Cache-Control",
  ]),
  }));
```

Esto producirá la siguiente expresión CEL:

```protobuf
default_certification (
  ValidationArgs {
  request_certification: RequestCertification {
    certified_request_headers: ["Accept", "Accept-Encoding", "If-None-Match"],
    certified_query_parameters: ["foo", "bar", "baz"]
  },
  response_certification: ResponseCertification {
    certified_response_headers: ResponseHeaderList {
    headers: [
      "ETag",
      "Cache-Control"
    ]
    }
  }
  }
)
```

### Solicitud parcialmente certificada

Se pueden proporcionar cualquier número de cabeceras de solicitud o parámetros
de consulta a través de los campos `headers` y `query_parameters` de la
estructura `DefaultRequestCertification`, y ambos también pueden ser una matriz
vacía. Si el campo `headers` está vacío, no se certificarán las cabeceras de la
solicitud. Del mismo modo, si el campo `query_parameters` está vacío, no se
certificarán los parámetros de consulta. Si ambos están vacíos, solo se
certificará el cuerpo de la solicitud y el método.

Por ejemplo, para certificar solo el cuerpo de la solicitud y el método:

```rust
use std::borrow::Cow;
use ic_http_certification::cel::{CelExpression, DefaultCelExpression, DefaultFullCelExpression, DefaultRequestCertification, DefaultResponseCertification};

let cel_expr = CelExpression::Default(DefaultCelExpression::Full(
  DefaultFullCelExpression {
  request: DefaultRequestCertification::new(
    vec![],
    vec![],
  ),
  response: DefaultResponseCertification::certified_response_headers(vec![
    "ETag",
    "Cache-Control",
  ]),
  }));
```

Esto producirá la siguiente expresión CEL:

```protobuf
default_certification (
  ValidationArgs {
  request_certification: RequestCertification {
    certified_request_headers: [],
    certified_query_parameters: []
  },
  response_certification: ResponseCertification {
    certified_response_headers: ResponseHeaderList {
    headers: [
      "ETag",
      "Cache-Control"
    ]
    }
  }
  }
)
```

### Omitir la certificación de solicitud

La certificación de solicitud se puede omitir por completo utilizando la
variante `ResponseOnly` de la estructura `DefaultCelExpression`.

Por ejemplo:

```rust
use std::borrow::Cow;
use ic_http_certification::cel::{CelExpression, DefaultCelExpression, DefaultResponseOnlyCelExpression, DefaultResponseCertification};

let cel_expr = CelExpression::Default(DefaultCelExpression::ResponseOnly(
  DefaultResponseOnlyCelExpression {
  response: DefaultResponseCertification::certified_response_headers(vec![
    "ETag",
    "Cache-Control",
  ]),
  }));
```

Esto producirá la siguiente expresión CEL:

```protobuf
default_certification (
  ValidationArgs {
    no_request_certification: Empty {},
    response_certification: ResponseCertification {
      certified_response_headers: ResponseHeaderList {
        headers: [
          "ETag",
          "Cache-Control"
        ]
      }
    }
  }
)
```

### Respuesta parcialmente certificada

De manera similar a la certificación de solicitud, se pueden proporcionar
cualquier número de cabeceras de respuesta a través de la función asociada
`certified_response_headers` del enum `DefaultResponseCertification`, y también
puede ser un arreglo vacío. Si el arreglo está vacío, no se certificarán las
cabeceras de respuesta.

Por ejemplo:

```rust
use std::borrow::Cow;
use ic_http_certification::cel::{CelExpression, DefaultCertification, DefaultRequestCertification, DefaultResponseCertification};

let cel_expr = CelExpression::DefaultCertification(Some(DefaultCertification {
  request: DefaultRequestCertification::new(
    vec!["Accept", "Accept-Encoding", "If-None-Match"],
    vec!["foo", "bar", "baz"],
  ),
  response_certification: DefaultResponseCertification::certified_response_headers(vec![]),
}));
```

Esto producirá la siguiente expresión CEL:

```protobuf
default_certification (
  ValidationArgs {
    request_certification: RequestCertification {
      certified_request_headers: ["Accept", "Accept-Encoding", "If-None-Match"],
      certified_query_parameters: ["foo", "bar", "baz"]
    },
    response_certification: ResponseCertification {
      certified_response_headers: ResponseHeaderList {
        headers: []
      }
    }
  }
)
```

Si se utiliza la función asociada `response_header_exclusions`, un arreglo vacío
certificará _todas_ las cabeceras de respuesta. Por ejemplo:

```rust
use std::borrow::Cow;
use ic_http_certification::cel::{CelExpression, DefaultCelExpression, DefaultFullCelExpression, DefaultRequestCertification, DefaultResponseCertification};

let cel_expr = CelExpression::Default(DefaultCelExpression::Full(
  DefaultFullCelExpression {
    request: DefaultRequestCertification::new(
      vec!["Accept", "Accept-Encoding", "If-None-Match"],
      vec!["foo", "bar", "baz"],
    ),
    response: DefaultResponseCertification::response_header_exclusions(vec![]),
  }));
```

Esto producirá la siguiente expresión CEL:

```protobuf
default_certification (
  ValidationArgs {
    request_certification: RequestCertification {
      certified_request_headers: ["Accept", "Accept-Encoding", "If-None-Match"],
      certified_query_parameters: ["foo", "bar", "baz"]
    },
    response_certification: ResponseCertification {
      response_header_exclusions: ResponseHeaderList {
        headers: []
      }
    }
  }
)
```

Para omitir completamente la certificación de respuesta, también se debe omitir
la certificación en general. No sería útil certificar una solicitud sin
certificar una respuesta.

### Omitir la certificación

Para omitir completamente la certificación:

```rust
use ic_http_certification::cel::{CelExpression, DefaultCelExpression};

let cel_expr = CelExpression::Default(DefaultCelExpression::Skip);
```

Esto producirá la siguiente expresión CEL:

```protobuf
default_certification (
  ValidationArgs {
    no_certification: Empty {}
  }
)
```
