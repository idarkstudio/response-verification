# Servir JSON sobre HTTP

Esta guía muestra un proyecto de ejemplo que demuestra cómo crear un contenedor
que puede servir JSON certificado sobre HTTP. El proyecto de ejemplo presenta
una API REST muy simple para crear y listar elementos de tareas pendientes. No
hay autenticación ni almacenamiento persistente.

Esta no es una guía de desarrollo de contenedores para principiantes. Se
omitirán muchos conceptos fundamentales que un desarrollador de contenedores
relativamente experimentado debería conocer. Los conceptos específicos de la
certificación HTTP se destacarán aquí y pueden ayudar a comprender el
[ejemplo de código completo](https://github.com/dfinity/response-verification/tree/main/examples/http-certification/json-api).

## Requisitos previos

Se recomienda revisar las guías anteriores antes de leer esta.

- [x] Completar la guía
      ["Contenedores HTTP personalizados"](https://internetcomputer.org/docs/current/developer-docs/http-compatible-canisters/custom-http-canisters).

## Ciclo de vida

Las respuestas se certifican en el gancho `init`. La misma función se ejecuta
durante el gancho `post_upgrade` ya que el árbol de certificación no persiste en
las actualizaciones.

```rust
// run when a canister is first installed
#[init]
fn init() {
    // certify all static responses
    certify_list_todos_response();
    certify_not_allowed_todo_responses();
    certify_not_found_response();

    // prepare query and update handlers
    prepare_query_handlers();
    prepare_update_handlers();
}

// run every time a canister is upgraded
#[post_upgrade]
fn post_upgrade() {
    // run the same initialization logic
    init();
}
```

## Expresiones CEL

Las expresiones CEL solo necesitan configurarse una vez y luego se pueden
reutilizar hasta la próxima actualización del contenedor. Las respuestas también
se pueden configurar una vez y reutilizar. Si la respuesta es estática y no
cambiará durante la vida útil del contenedor, solo necesita certificarse una
vez. Sin embargo, si la respuesta puede cambiar, entonces deberá certificarse
cada vez que cambie.

`DefaultResponseOnlyCelExpression` se utiliza cuando solo se va a certificar la
respuesta. Si también se va a certificar la solicitud, se debe usar
`DefaultFullCelExpression`. Alternativamente, la expresión CEL de nivel superior
`DefaultCelExpression` puede contener cualquier tipo de expresión CEL utilizando
el esquema "Default". En el futuro, puede haber más esquemas y la expresión
`CelExpression` de nivel superior podrá contener expresiones CEL de esos
diferentes esquemas. Depende de los desarrolladores decidir cómo desean
almacenar y organizar sus expresiones CEL.

En este ejemplo, se utilizan dos expresiones CEL diferentes, una expresión CEL
"completa" y una expresión CEL "solo respuesta". La expresión CEL "completa" se
utiliza para los "todos" certificados y la expresión CEL "solo respuesta" para
la respuesta "No encontrada". Para obtener más información sobre cómo definir
expresiones CEL, consulte la sección relevante en la documentación de
[`ic-http-certification`](https://docs.rs/ic-http-certification/latest/ic_http_certification/#defining-cel-expressions).

```rust
lazy_static! {
    // definir una expresión CEL completa que certificará lo siguiente:
    // - solicitud
    //   - método
    //   - cuerpo
    //   - sin headers
    //   - sin parámetros de consulta
    // - respuesta
    //   - código de estado
    //   - cuerpo
    //   - todos los headers
    // esta expresión CEL se utilizará para todas las rutas excepto la ruta no encontrada
    static ref TODO_CEL_EXPR_DEF: DefaultFullCelExpression<'static> = DefaultCelBuilder::full_certification()
        .with_request_headers(vec![])
        .with_request_query_parameters(vec![])
        .with_response_certification(DefaultResponseCertification::response_header_exclusions(
            vec![],
        ))
        .build();
    static ref TODO_CEL_EXPR: String = TODO_CEL_EXPR_DEF.to_string();

    // definir una expresión CEL solo respuesta que certificará lo siguiente:
    // - respuesta
    //   - código de estado
    //   - cuerpo
    //   - todos los headers
    // esta expresión CEL se utilizará para la ruta no encontrada
    static ref NOT_FOUND_CEL_EXPR_DEF: DefaultResponseOnlyCelExpression<'static> = DefaultCelBuilder::response_only_certification()
        .with_response_certification(DefaultResponseCertification::response_header_exclusions(
            vec![],
        ))
        .build();
    static ref NOT_FOUND_CEL_EXPR: String = NOT_FOUND_CEL_EXPR_DEF.to_string();
}
```

## Headers de respuesta

Los headers de seguridad agregados a las respuestas se basan en el proyecto
[OWASP Secure Headers](https://owasp.org/www-project-secure-headers/index.html).

Estos headers de seguridad se han incluido como un valor predeterminado
razonablemente seguro para la mayoría de las API basadas en JSON. Sin embargo,
es de vital importancia que los desarrolladores se eduquen y tomen decisiones
informadas en el contexto de las necesidades de su propio proyecto.

Algunos headers de este proyecto no se han incluido:

- `X-Frame-Options`: Este encabezado se utiliza para prevenir ataques de
  clickjacking al incrustar contenido web dentro de una página web maliciosa.
  Las API JSON puras generalmente no son vulnerables a este tipo de ataque, ya
  que no se pueden representar directamente en un navegador. Sin embargo, los
  desarrolladores pueden incluir este encabezado adicionalmente con un valor de
  `deny` o `sameorigin` para ser cautelosos.
- `Content-Security-Policy` (CSP): Este encabezado se utiliza para prevenir
  ataques de scripting entre sitios (XSS) en sitios que representan contenido
  HTML. Define qué fuentes debe considerar válidas el navegador para cargar
  scripts, hojas de estilo u otros recursos en el contexto de la página cargada.
  Dado que las API JSON puras no se representan directamente en un navegador, no
  son vulnerables a este tipo de ataque. Sin embargo, los desarrolladores pueden
  incluir este encabezado adicionalmente para ser cautelosos.
- `X-Permitted-Cross-Domain-Policies`: Este encabezado se utilizaba para
  proporcionar control de acceso a tecnologías heredadas como Adobe Flash o
  Acrobat, pero estas tecnologías son en gran medida obsoletas ahora y las API
  basadas en JSON modernas deben preferir el uso de headers
  `Access-Control-Allow-Origin` (CORS).
- `Clear-Site-Data`: Este encabezado se utiliza para indicar a los navegadores
  que borren datos específicos del sitio, como almacenamiento local, cookies o
  cachés. Dado que esta API no establece cookies, no es necesario incluir el
  encabezado.
- `Cross-Origin-Embedder-Policy`: Este encabezado se utiliza para mitigar
  ataques Spectre o Meltdown al evitar que un sitio web incruste los recursos
  secundarios de otro sitio web. Dado que las API JSON puras no se representan
  directamente en un navegador, no son vulnerables a estos ataques.
- `Cross-Origin-Opener-Policy`: Este encabezado se utiliza para evitar que los
  sitios web abiertos en una nueva pestaña o ventana mantengan acceso a la
  pestaña o ventana abridora original. Dado que las API JSON puras no se
  representan directamente en un navegador, no son vulnerables a este tipo de
  ataque.
- `Cross-Origin-Resource-Policy`: Este encabezado se utiliza para limitar el
  acceso a una API desde otros orígenes. Se debe preferir CORS como un enfoque
  más moderno para el control de acceso, pero este encabezado se puede incluir
  si se espera que un navegador más antiguo acceda a la API.
- `Permissions-Policy`: Este encabezado se utiliza para limitar las
  características y API (por ejemplo, geolocalización, cámara, micrófono) a las
  que se permite acceder al navegador en el contexto de un sitio web. Dado que
  las API JSON puras no se representan directamente en un navegador, este
  encabezado no es relevante.

Para facilitar el uso consistente de estos headers, hay una función
reutilizable `create_response` que se utiliza al crear respuestas:

```rust
fn create_response(status_code: StatusCode, body: Vec<u8>) -> HttpResponse<'static> {
    HttpResponse::builder()
        .with_status_code(status_code)
        .with_headers(vec![
            ("content-type".to_string(), "application/json".to_string()),
            (
                "strict-transport-security".to_string(),
                "max-age=31536000; includeSubDomains".to_string(),
            ),
            ("x-content-type-options".to_string(), "nosniff".to_string()),
            ("referrer-policy".to_string(), "no-referrer".to_string()),
            (
                "cache-control".to_string(),
                "no-store, max-age=0".to_string(),
            ),
            ("pragma".to_string(), "no-cache".to_string()),
        ])
        .with_body(body)
        .build()
}
```

## Respuestas

El árbol de certificación HTTP tiene una estructura de datos dedicada, mientras
que las respuestas se almacenan en un `HashMap`, junto con sus respectivas
certificaciones. Las respuestas y certificaciones se almacenan por separado de
las expresiones CEL porque es probable que cambien a lo largo del ciclo de vida
del contenedor, mientras que las expresiones CEL se configuran solo una vez.
También se podrían almacenar todos en la misma estructura si así lo desea el
desarrollador.

Respuestas de respaldo (como la respuesta "no encontrada") se almacenan por separado de otras respuestas. Esto se hace para permitir una lógica de enrutamiento más simple para las respuestas, lo cual se describirá con más detalle más adelante en esta guía.

```rust
struct CertifiedHttpResponse<'a> {
    response: HttpResponse<'a>,
    certification: HttpCertification,
}

thread_local! {
    static FALLBACK_RESPONSES: RefCell<HashMap<String, CertifiedHttpResponse<'static>>> = RefCell::new(HashMap::new());
    static RESPONSES: RefCell<HashMap<(String, String), CertifiedHttpResponse<'static>>> = RefCell::new(HashMap::new());

    static HTTP_TREE: RefCell<HttpCertificationTree> = RefCell::new(HttpCertificationTree::default());
}
```

Las respuestas se certifican con varios pasos, que se encapsulan en una función
reutilizable:

- Eliminar cualquier respuesta y certificación existente para la ruta de la
  solicitud.
  - Esto se hace para evitar que se certifiquen múltiples respuestas para una
    ruta de solicitud determinada.
- Recuperar la expresión CEL precalculada para la ruta de la solicitud.
- Insertar el encabezado `Ic-CertificationExpression` para la respuesta dada,
  con la expresión CEL correspondiente convertida a cadena como su valor.
- Calcular la certificación para la respuesta dada y la expresión CEL.
- Almacenar la respuesta junto con su certificación.
- Insertar la certificación en el árbol de certificación en la ruta
  correspondiente.
- Actualizar los datos certificados del contenedor.

Para obtener más información sobre cómo crear certificaciones, consulte la
sección relevante en la documentación de
[`ic-http-certification`](https://docs.rs/ic-http-certification/latest/ic_http_certification/#creating-certifications).

```rust
fn certify_response(
    request: HttpRequest,
    response: &mut HttpResponse<'static>,
    tree_path: &HttpCertificationPath,
) {
    let request_path = request.get_path().unwrap();

    // recuperar y eliminar cualquier respuesta existente para el método y la ruta de la solicitud
    let existing_response = RESPONSES.with_borrow_mut(|responses| {
        responses.remove(&(request.method().to_string(), request_path.clone()))
    });

    // si hay una respuesta existente, eliminar su certificación del árbol de certificación
    if let Some(existing_response) = existing_response {
        HTTP_TREE.with_borrow_mut(|http_tree| {
            http_tree.delete(&HttpCertificationTreeEntry::new(
                tree_path.clone(),
                &existing_response.certification,
            ));
        })
    }

    // insertar el encabezado `Ic-CertificationExpression` con la expresión CEL convertida a cadena como su valor
    response.add_header((
        CERTIFICATE_EXPRESSION_HEADER_NAME.to_string(),
        TODO_CEL_EXPR.clone(),
    ));

    // crear la certificación para esta respuesta y par de expresión CEL
    let certification =
        HttpCertification::full(&TODO_CEL_EXPR_DEF, &request, &response, None).unwrap();

    RESPONSES.with_borrow_mut(|responses| {
        // almacenar la respuesta para su posterior recuperación
        responses.insert(
            (request.method().to_string(), request_path),
            CertifiedHttpResponse {
                response: response.clone(),
                certification: certification.clone(),
            },
        );
    });

    HTTP_TREE.with_borrow_mut(|http_tree| {
        // insertar la certificación en el árbol de certificación
        http_tree.insert(&HttpCertificationTreeEntry::new(tree_path, &certification));

        // establecer los datos certificados del contenedor
        set_certified_data(&http_tree.root_hash());
    });
}
```

Estos pasos ahora se pueden reutilizar para cada respuesta que necesita ser
certificada:

```rust
fn certify_list_todos_response() {
    let request = HttpRequest::get(TODOS_PATH).build();

    let body = TODO_ITEMS.with_borrow(|items| {
        ListTodosResponse::ok(
            &items
                .iter()
                .map(|(_id, item)| item.clone())
                .collect::<Vec<_>>(),
        )
        .encode()
    });
    let mut response = create_response(StatusCode::OK, body);

    certify_response(request, &mut response, &TODOS_TREE_PATH);
}

fn certify_not_allowed_todo_responses() {
    [
        Method::HEAD,
        Method::PUT,
        Method::PATCH,
        Method::OPTIONS,
        Method::TRACE,
        Method::CONNECT,
    ]
    .into_iter()
    .for_each(|method| {
        let request = HttpRequest::builder()
            .with_method(method)
            .with_url(TODOS_PATH)
            .build();

        let body = ErrorResponse::not_allowed().encode();
        let mut response = create_response(StatusCode::METHOD_NOT_ALLOWED, body);

        certify_response(request, &mut response, &TODOS_TREE_PATH);
    });
}
```

Certificar la respuesta "No encontrada" requiere un procedimiento ligeramente
diferente. Esto es muy similar a la función reutilizable `certify_response`,
pero con las siguientes diferencias:

- La variante `HttpCertificationPath` utilizada es `wildcard` en lugar de
  `exact`.
- Se utiliza una `DefaultResponseOnlyCelExpression` en lugar de una
  `DefaultFullCelExpression`.
- La respuesta se almacena en `FALLBACK_RESPONSES` en lugar de `RESPONSES`.

```rust
fn certify_not_found_response() {
    let body = ErrorResponse::not_found().encode();
    let mut response = create_response(StatusCode::NOT_FOUND, body);

    let tree_path = HttpCertificationPath::wildcard(NOT_FOUND_PATH);

    // insertar el encabezado `Ic-CertificationExpression` con la expresión CEL convertida a cadena como su valor
    response.add_header((
        CERTIFICATE_EXPRESSION_HEADER_NAME.to_string(),
        NOT_FOUND_CEL_EXPR.clone(),
    ));

    // create the certification for this response and CEL expression pair
    let certification =
        HttpCertification::response_only(&NOT_FOUND_CEL_EXPR_DEF, &response, None).unwrap();

    FALLBACK_RESPONSES.with_borrow_mut(|responses| {
        responses.insert(
            NOT_FOUND_PATH.to_string(),
            CertifiedHttpResponse {
                response,
                certification,
            },
        );
    });

    HTTP_TREE.with_borrow_mut(|http_tree| {
        // insert the certification into the certification tree
        http_tree.insert(&HttpCertificationTreeEntry::new(tree_path, &certification));

        // set the canister's certified data
        set_certified_data(&http_tree.root_hash());
    });
}
```

## Sirviendo respuestas

Al servir una respuesta certificada, se debe agregar un encabezado adicional a
la respuesta que actuará como prueba de certificación para la
[puerta de enlace HTTP](https://internetcomputer.org/docs/current/references/http-gateway-protocol-spec)
que realizará la validación. Agregar este encabezado a la respuesta se ha
abstraído en una función separada:

Con esta función reutilizable, servir respuestas certificadas es relativamente
sencillo:

- Primero, verificar si existe una respuesta para la URL y el método de
  solicitud actual.
- Si existe una respuesta, servirla.
- De lo contrario, servir la respuesta de "No encontrada" por defecto.
- Agregar el encabezado de respuesta `IC-Certificate`.

```rust
fn query_handler(request: &HttpRequest, _params: &Params) -> HttpResponse<'static> {
    let request_path = request.get_path().expect("No se pudo obtener la ruta de la solicitud");

    // primero verificar si hay una respuesta certificada para el método y la ruta de la solicitud
    let (tree_path, certified_response) = RESPONSES
        .with_borrow(|responses| {
            responses
                .get(&(request.method().to_string(), request_path.clone()))
                .map(|response| {
                    (
                        HttpCertificationPath::exact(&request_path),
                        response.clone(),
                    )
                })
        })
        // si no hay una respuesta certificada, usar la respuesta de "No encontrada" por defecto
        .unwrap_or_else(|| {
            FALLBACK_RESPONSES.with_borrow(|fallback_responses| {
                fallback_responses
                    .get(NOT_FOUND_PATH)
                    .clone()
                    .map(|response| (NOT_FOUND_TREE_PATH.to_owned(), response.clone()))
                    .unwrap()
            })
        });

    let mut response = certified_response.response;

    HTTP_TREE.with_borrow(|http_tree| {
        add_v2_certificate_header(
            &data_certificate().expect("No hay un certificado de datos disponible"),
            &mut response,
            &http_tree
                .witness(
                    &HttpCertificationTreeEntry::new(&tree_path, certified_response.certification),
                    &request_path,
                )
                .unwrap(),
            &tree_path.to_expr_path(),
        );
    });

    response
}
```

Cuando se realizan llamadas de actualización a puntos finales que no actualizan
el estado, se devuelve un error para evitar costos adicionales de ciclo para
estos puntos finales:

```rust
fn no_update_call_handler(_http_request: &HttpRequest, _params: &Params) -> HttpResponse<'static> {
    create_response(StatusCode::BAD_REQUEST, vec![])
}
```

## Actualizando el estado

La lista de tareas pendientes se puede actualizar mediante solicitudes `POST`,
`PATCH` y `DELETE`. Estas llamadas se recibirán inicialmente como
[llamadas de consulta](https://internetcomputer.org/docs/current/references/ic-interface-spec/#http-query)
que no permiten actualizar el estado del canister, por lo que la llamada de
consulta se
[actualiza a una llamada de actualización](https://internetcomputer.org/docs/current/references/http-gateway-protocol-spec#upgrade-to-update-calls)
para permitir que el estado del canister cambie.

```rust
fn upgrade_to_update_call_handler(
    _http_request: &HttpRequest,
    _params: &Params,
) -> HttpResponse<'static> {
    HttpResponse::builder().with_upgrade(true).build()
}
```

La actualización a una llamada de `update` instruirá a la puerta de enlace HTTP
a volver a realizar la solicitud como una
[llamada de actualización](https://internetcomputer.org/docs/current/references/ic-interface-spec/#http-call).
Como llamada de actualización, la respuesta a esta solicitud no necesita estar
certificada. Sin embargo, dado que el estado del canister ha cambiado, las
respuestas estáticas de llamadas de consulta deberán volver a certificarse. Las
mismas funciones que certificaron estas respuestas en primer lugar se pueden
reutilizar para lograr esto.

Para crear elementos de la lista de tareas pendientes:

```rust
fn create_todo_item_handler(req: &HttpRequest, _params: &Params) -> HttpResponse<'static> {
    let req_body: CreateTodoItemRequest = json_decode(req.body());

    let id = NEXT_TODO_ID.with_borrow_mut(|f| {
        let id = *f;
        *f += 1;
        id
    });

    let todo_item = TODO_ITEMS.with_borrow_mut(|items| {
        let todo_item = TodoItem {
            id,
            title: req_body.title,
            completed: false,
        };

        items.insert(id, todo_item.clone());

        todo_item
    });

    certify_list_todos_response();

    let body = CreateTodoItemResponse::ok(&todo_item).encode();
    create_response(StatusCode::CREATED, body)
}
```

Para actualizar elementos de la lista de tareas pendientes:

```rust
fn update_todo_item_handler(req: &HttpRequest, params: &Params) -> HttpResponse<'static> {
    let req_body: UpdateTodoItemRequest = json_decode(req.body());
    let id: u32 = params.get("id").unwrap().parse().unwrap();

    TODO_ITEMS.with_borrow_mut(|items| {
        let item = items.get_mut(&id).unwrap();

        if let Some(title) = req_body.title {
            item.title = title;
        }

        if let Some(completed) = req_body.completed {
            item.completed = completed;
        }
    });

    certify_list_todos_response();

    let body = UpdateTodoItemResponse::ok(&()).encode();
    create_response(StatusCode::OK, body)
}
```

Y, finalmente, para eliminar elementos de la lista de tareas pendientes:

```rust
fn delete_todo_item_handler(_req: &HttpRequest, params: &Params) -> HttpResponse<'static> {
    let id: u32 = params.get("id").unwrap().parse().unwrap();

    TODO_ITEMS.with_borrow_mut(|items| {
        items.remove(&id);
    });

    certify_list_todos_response();

    let body = DeleteTodoItemResponse::ok(&()).encode();
    create_response(StatusCode::NO_CONTENT, body)
}
```

## Enrutamiento

Para configurar el enrutamiento, se utiliza la biblioteca
[`matchit`](https://docs.rs/matchit/latest/matchit/). Se crea un enrutador para
cada método de solicitud admitido y se crea una colección de enrutadores por
separado para llamadas de consulta y actualización. Estos enrutadores se
almacenan en `HashMap`s:

```rust
thread_local! {
    static QUERY_ROUTER: RefCell<HashMap<String, Router<RouteHandler>>> = RefCell::new(HashMap::new());
    static UPDATE_ROUTER: RefCell<HashMap<String, Router<RouteHandler>>> = RefCell::new(HashMap::new());
}
```

Los controladores de ruta se vinculan a los enrutadores. Para las llamadas de
consulta, solo se utilizan dos controladores de ruta:

- `upgrade_to_update_call_handler` para los métodos de solicitud que modificarán
  el estado del canister.
- `query_handler` para todo lo demás.

```rust
fn prepare_query_handlers() {
    insert_query_route("POST", "/todos", upgrade_to_update_call_handler);
    insert_query_route("PATCH", "/todos/{id}", upgrade_to_update_call_handler);
    insert_query_route("DELETE", "/todos/{id}", upgrade_to_update_call_handler);

    insert_query_route("GET", "/{*p}", query_handler);
    ["HEAD", "PUT", "OPTIONS", "TRACE", "CONNECT"]
        .iter()
        .for_each(|method| {
            insert_query_route(method, "/{*p}", query_handler);
        });
}

fn insert_query_route(method: &str, path: &str, route_handler: RouteHandler) {
    QUERY_ROUTER.with_borrow_mut(|query_router| {
        let router = query_router.entry(method.to_string()).or_default();

        router.insert(path, route_handler).unwrap();
    });
}
```

Para las llamadas de actualización, hay más controladores de ruta:

- `create_todo_item_handler` para las solicitudes POST.
- `update_todo_item_handler` para las solicitudes PATCH.
- `delete_todo_item_handler` para las solicitudes DELETE.
- `no_update_call_handler` para todo lo demás.

```rust
fn prepare_update_handlers() {
    insert_update_route("POST", TODOS_PATH, create_todo_item_handler);
    insert_update_route("PATCH", "/todos/{id}", update_todo_item_handler);
    insert_update_route("DELETE", "/todos/{id}", delete_todo_item_handler);

    ["GET", "HEAD", "PUT", "OPTIONS", "TRACE", "CONNECT"]
        .iter()
        .for_each(|method| {
            insert_update_route(method, "/{*p}", no_update_call_handler);
        });
}

fn insert_update_route(method: &str, path: &str, route_handler: RouteHandler) {
    UPDATE_ROUTER.with_borrow_mut(|update_router| {
        let router = update_router.entry(method.to_string()).or_default();

        router.insert(path, route_handler).unwrap();
    });
}
```

## Probando el canister

Este ejemplo utiliza un canister llamado `http_certification_json_api_backend`.

Para probar el canister, puedes usar
[`dfx`](https://internetcomputer.org/docs/current/developer-docs/getting-started/install)
para iniciar una instancia local del replica:

```shell
dfx start --background --clean
```

Luego, implementa el canister:

```shell
dfx deploy http_certification_json_api_backend
```

Para obtener los elementos de la lista de tareas pendientes:

```shell
curl -s \
    "http://$(dfx canister id http_certification_json_api_backend).localhost:$(dfx info webserver-port)/todos" \
    --resolve "$(dfx canister id http_certification_json_api_backend).localhost:$(dfx info webserver-port):127.0.0.1" | jq
```

Para agregar un elemento a la lista de tareas pendientes:

```shell
curl -s -X POST \
    "http://$(dfx canister id http_certification_json_api_backend).localhost:$(dfx info webserver-port)/todos" \
    --resolve "$(dfx canister id http_certification_json_api_backend).localhost:$(dfx info webserver-port):127.0.0.1" \
    -H "Content-Type: application/json" \
    -d '{ "title": "Aprender Motoko" }' | jq
```

Para actualizar un elemento de la lista de tareas pendientes:

```shell
curl -s -X PATCH \
    "http://$(dfx canister id http_certification_json_api_backend).localhost:$(dfx info webserver-port)/todos/0" \
    --resolve "$(dfx canister id http_certification_json_api_backend).localhost:$(dfx info webserver-port):127.0.0.1" \
    -H "Content-Type: application/json" \
    -d '{ "completed": true }' | jq
```

Para eliminar un elemento de la lista de tareas pendientes:

```shell
curl -s -X DELETE \
    "http://$(dfx canister id http_certification_json_api_backend).localhost:$(dfx info webserver-port)/todos/0" \
    --resolve "$(dfx canister id http_certification_json_api_backend).localhost:$(dfx info webserver-port):127.0.0.1" | jq
```

## Recursos

- [Código fuente de ejemplo](https://github.com/dfinity/response-verification/tree/main/examples/http-certification/json-api).
- Paquete `ic-http-certification` en
  [crates.io](https://crates.io/crates/ic-http-certification).
- Documentación de `ic-http-certification` en
  [docs.rs](https://docs.rs/ic-http-certification/latest/ic_http_certification).
- Código fuente de `ic-http-certification` en
  [GitHub](https://github.com/dfinity/response-verification/tree/main/packages/ic-http-certification).
- Proyectos de
  [OWASP Secure Headers](https://owasp.org/www-project-secure-headers/index.html).
