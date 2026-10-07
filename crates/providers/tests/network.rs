use axum::{
    body::Body,
    http::{Request, Response},
    routing::get,
    Router,
};
use video_providers::Session;
#[tokio::test]
async fn cookies_are_scoped_and_ranges_are_required() {
    let app = Router::new().fallback(get(|request: Request<Body>| async move {
        match request.uri().path() {
            "/init" => Response::builder()
                .header("Set-Cookie", "private=yes; Path=/protected; HttpOnly")
                .body(Body::empty())
                .unwrap(),
            "/protected/media" => {
                let cookie = request
                    .headers()
                    .get("cookie")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                Response::builder()
                    .status(if cookie.contains("private=yes") {
                        200
                    } else {
                        403
                    })
                    .body(Body::from("ok"))
                    .unwrap()
            }
            "/public" => Response::builder()
                .status(if request.headers().contains_key("cookie") {
                    403
                } else {
                    200
                })
                .body(Body::from("ok"))
                .unwrap(),
            _ => Response::builder()
                .body(Body::from("ignores range"))
                .unwrap(),
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let root = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let session = Session::new(&format!("{root}/init")).unwrap();
    session.text(&format!("{root}/init")).await.unwrap();
    assert!(session
        .response(&format!("{root}/protected/media"), None)
        .await
        .is_ok());
    assert!(session
        .response(&format!("{root}/public"), None)
        .await
        .is_ok());
    assert!(session
        .response(&format!("{root}/range"), Some((0, 10)))
        .await
        .is_err());
    task.abort();
}
