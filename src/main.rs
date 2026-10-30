//! Number games for a room of people.
//!
//! The host picks a rule and a range, everyone picks a number on their
//! phone, and the rule decides who won. No number is shown to anybody
//! until the host reveals the round.
//!
//! There are three kinds of link, and the key in each is the only
//! authority there is: `/h/<key>` runs the room, `/j/<key>` joins it, and
//! `/p/<key>` belongs to one player.

mod game;
mod view;

use std::{path::PathBuf, sync::Arc};

use axum::{
    Json, Router,
    extract::{Path, State},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use rooms::{Rooms, http, now_ms, site};
use serde::Deserialize;

use game::{Game, Rule};

/// The page and its files. The script and stylesheet are compiled from
/// the TypeScript and CSS in `client/` by the build script.
const FILES: site::Files = site::Files {
    page: include_str!("../static/index.html"),
    icon: include_str!("../static/icon.svg"),
    script: include_str!(concat!(env!("OUT_DIR"), "/app.js")),
    stylesheet: include_str!(concat!(env!("OUT_DIR"), "/app.css")),
};

/// How many rooms at once. Limited because anyone can open one.
const MAX_ROOMS: usize = 2000;

type Shared = Arc<Rooms<Game>>;

/// Applies a host action to the room and returns the host's view, so the
/// screen that pressed the button does not wait for its own stream.
async fn act(
    rooms: &Rooms<Game>,
    key: &str,
    action: impl FnOnce(&mut Game, u64) -> Result<(), &'static str>,
) -> Response {
    site::act(rooms, key, action, |game, now| {
        http::envelope(now, &view::host(game))
    })
    .await
}

#[derive(Deserialize)]
struct Started {
    #[serde(flatten)]
    rule: Rule,
    low: f64,
    high: f64,
}

#[derive(Deserialize)]
struct Opened {
    open: bool,
}

#[derive(Deserialize)]
struct Named {
    name: String,
}

#[derive(Deserialize)]
struct Picked {
    value: f64,
}

async fn open_room(State(rooms): State<Shared>) -> Response {
    let now = now_ms();
    site::open(&rooms, |_| Game::new(now), |game| game.join.clone()).await
}

async fn host_state(State(rooms): State<Shared>, Path(key): Path<String>) -> Response {
    match rooms
        .read(&key, |game, now| http::envelope(now, &view::host(game)))
        .await
    {
        Some(body) => http::json(body),
        None => http::gone(),
    }
}

async fn host_events(State(rooms): State<Shared>, Path(key): Path<String>) -> Response {
    rooms::one(rooms, key, |game| {
        serde_json::to_string(&view::host(game)).ok()
    })
}

async fn start(
    State(rooms): State<Shared>,
    Path(key): Path<String>,
    Json(started): Json<Started>,
) -> Response {
    act(&rooms, &key, |game, now| {
        game.start(started.rule, started.low, started.high, now)
    })
    .await
}

async fn set_open(
    State(rooms): State<Shared>,
    Path(key): Path<String>,
    Json(opened): Json<Opened>,
) -> Response {
    act(&rooms, &key, |game, _| game.set_open(opened.open)).await
}

async fn reveal(State(rooms): State<Shared>, Path(key): Path<String>) -> Response {
    act(&rooms, &key, |game, _| game.reveal()).await
}

async fn join(
    State(rooms): State<Shared>,
    Path(join): Path<String>,
    Json(named): Json<Named>,
) -> Response {
    let Some(key) = rooms.key_where(|game| game.join == join).await else {
        return http::gone();
    };
    match rooms
        .with(&key, |game, now| game.join(&named.name, now))
        .await
    {
        None => http::gone(),
        Some(Err(message)) => http::refused(message),
        Some(Ok(player)) => Json(serde_json::json!({ "player": player })).into_response(),
    }
}

/// How many people are already in, which is what the join page shows
/// before anyone gives a name.
async fn how_many(State(rooms): State<Shared>, Path(join): Path<String>) -> Response {
    let Some(key) = rooms.key_where(|game| game.join == join).await else {
        return http::gone();
    };
    match rooms.read(&key, |game, _| game.players.len()).await {
        Some(players) => Json(serde_json::json!({ "players": players })).into_response(),
        None => http::gone(),
    }
}

async fn player_state(State(rooms): State<Shared>, Path(key): Path<String>) -> Response {
    let found = rooms
        .find(|_, game| view::player(game, &key).map(|view| http::envelope(now_ms(), &view)))
        .await;
    match found {
        Some(body) => http::json(body),
        None => http::gone(),
    }
}

async fn player_events(State(rooms): State<Shared>, Path(key): Path<String>) -> Response {
    rooms::stream(rooms, move |held, now| {
        let game = held.values_mut().find(|game| game.player(&key).is_some())?;
        game.last_seen = now;
        serde_json::to_string(&view::player(game, &key)?).ok()
    })
}

async fn pick(
    State(rooms): State<Shared>,
    Path(key): Path<String>,
    Json(picked): Json<Picked>,
) -> Response {
    let Some(room) = rooms
        .find(|room, game| game.player(&key).map(|_| room.to_string()))
        .await
    else {
        return http::gone();
    };
    let done = rooms
        .with(&room, |game, now| {
            game.pick(&key, picked.value, now)
                .map(|()| http::envelope(now, &view::player(game, &key)))
        })
        .await;
    match done {
        None => http::gone(),
        Some(Err(message)) => http::refused(message),
        Some(Ok(body)) => http::json(body),
    }
}

fn router(rooms: Shared) -> Router {
    Router::new()
        .merge(site::routes(FILES))
        .route("/api/rooms", post(open_room))
        .route("/api/h/{key}", get(host_state))
        .route("/api/h/{key}/events", get(host_events))
        .route("/api/h/{key}/start", post(start))
        .route("/api/h/{key}/open", put(set_open))
        .route("/api/h/{key}/reveal", post(reveal))
        .route("/api/j/{key}", get(how_many).post(join))
        .route("/api/p/{key}", get(player_state))
        .route("/api/p/{key}/events", get(player_events))
        .route("/api/p/{key}/pick", put(pick))
        .with_state(rooms)
}

#[tokio::main]
async fn main() {
    let addr = std::env::var("NUMBERS_ADDR").unwrap_or_else(|_| "127.0.0.1:8098".into());
    let state = std::env::var("NUMBERS_STATE").unwrap_or_else(|_| "rooms.json".into());

    let rooms: Shared = Rooms::load(PathBuf::from(state), MAX_ROOMS);
    tokio::spawn(rooms.clone().sweep(rooms::SWEEP));

    rooms::serve(&addr, router(rooms)).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode, header},
    };
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    fn app(name: &str) -> (Router, PathBuf) {
        let path =
            std::env::temp_dir().join(format!("numbers-test-{name}-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        (router(Rooms::load(&path, 4)), path)
    }

    async fn call(
        app: &Router,
        method: &str,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let request = Request::builder().method(method).uri(uri);
        let request = match body {
            Some(body) => request
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        }
        .expect("the request is well formed");
        let response = app
            .clone()
            .oneshot(request)
            .await
            .expect("the router answers");
        let status = response.status();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("a body")
            .to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    async fn ok(app: &Router, method: &str, uri: &str, body: Option<Value>) -> Value {
        let (status, body) = call(app, method, uri, body).await;
        assert!(status.is_success(), "{method} {uri}: {status} {body}");
        body
    }

    /// A room with two players and a round of nought to ten, a third of
    /// the average.
    async fn ready(app: &Router) -> (String, String, String) {
        let keys = ok(app, "POST", "/api/rooms", None).await;
        let host = keys["host"].as_str().unwrap().to_string();
        let join = keys["join"].as_str().unwrap().to_string();
        let mut players = Vec::new();
        for name in ["Ada", "Bo"] {
            let body = ok(
                app,
                "POST",
                &format!("/api/j/{join}"),
                Some(json!({ "name": name })),
            )
            .await;
            players.push(body["player"].as_str().unwrap().to_string());
        }
        ok(
            app,
            "POST",
            &format!("/api/h/{host}/start"),
            Some(json!({"rule": "of-the-mean", "num": 1, "den": 3, "low": 0, "high": 10})),
        )
        .await;
        (host, players.remove(0), players.remove(0))
    }

    #[tokio::test]
    async fn a_round_is_played_and_comes_out() {
        let (app, path) = app("round");
        let (host, ada, bo) = ready(&app).await;

        let view = ok(
            &app,
            "PUT",
            &format!("/api/p/{ada}/pick"),
            Some(json!({"value": 9})),
        )
        .await["view"]
            .clone();
        assert_eq!(view["mine"], 9.0);
        assert_eq!(view["round"]["label"], "closest to 1/3 of the average");
        ok(
            &app,
            "PUT",
            &format!("/api/p/{bo}/pick"),
            Some(json!({"value": 1})),
        )
        .await;

        let view = ok(&app, "GET", &format!("/api/h/{host}"), None).await["view"].clone();
        assert!(view["outcome"].is_null(), "nothing before the reveal");

        let view = ok(&app, "POST", &format!("/api/h/{host}/reveal"), None).await["view"].clone();
        assert_eq!(view["outcome"]["mean"], 5.0);
        assert_eq!(view["outcome"]["winners"][0], "Bo");
        assert_eq!(view["outcome"]["picks"][0]["value"], 1.0);
        assert_eq!(view["scores"][0]["name"], "Bo");
        assert_eq!(view["scores"][0]["wins"], 1);

        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn nobody_is_sent_a_number_before_the_reveal() {
        let (app, path) = app("quiet");
        let (host, ada, bo) = ready(&app).await;
        ok(
            &app,
            "PUT",
            &format!("/api/p/{ada}/pick"),
            Some(json!({"value": 7})),
        )
        .await;

        let host_view = ok(&app, "GET", &format!("/api/h/{host}"), None)
            .await
            .to_string();
        assert!(!host_view.contains("\"value\""), "{host_view}");
        let other = ok(&app, "GET", &format!("/api/p/{bo}"), None).await;
        assert!(other["view"]["mine"].is_null());
        assert_eq!(other["view"]["picked"], 1, "a count and no more");

        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn a_number_outside_the_range_is_refused() {
        let (app, path) = app("range");
        let (_, ada, _) = ready(&app).await;
        let (status, body) = call(
            &app,
            "PUT",
            &format!("/api/p/{ada}/pick"),
            Some(json!({"value": 11})),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(
            body["error"],
            "That number is outside the range for this round."
        );

        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn every_rule_can_be_started() {
        let (app, path) = app("rules");
        let keys = ok(&app, "POST", "/api/rooms", None).await;
        let host = keys["host"].as_str().unwrap().to_string();
        for rule in [
            json!({"rule": "of-the-mean", "num": 2, "den": 3, "low": 0, "high": 100}),
            json!({"rule": "of-the-median", "num": 1, "den": 2, "low": 0, "high": 100}),
            json!({"rule": "lowest-unique", "low": 1, "high": 20}),
            json!({"rule": "highest-unique", "low": 1, "high": 20}),
            json!({"rule": "under-the-mean", "low": 0, "high": 100}),
        ] {
            let view =
                ok(&app, "POST", &format!("/api/h/{host}/start"), Some(rule)).await["view"].clone();
            assert!(view["round"]["label"].is_string());
        }
        let (status, _) = call(
            &app,
            "POST",
            &format!("/api/h/{host}/start"),
            Some(json!({"rule": "of-the-mean", "num": 5, "den": 3, "low": 0, "high": 10})),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "5/3 of the average runs away");

        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn one_kind_of_key_does_not_work_as_another() {
        let (app, path) = app("keys");
        let (host, ada, _) = ready(&app).await;
        for uri in [
            format!("/api/h/{ada}"),
            format!("/api/p/{host}"),
            "/api/h/notakey".to_string(),
            "/api/p/notakey".to_string(),
            "/api/j/notakey".to_string(),
        ] {
            let (status, _) = call(&app, "GET", &uri, None).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
        }
        let (status, _) = call(&app, "POST", &format!("/api/h/{ada}/reveal"), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "a player cannot reveal");

        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn a_room_survives_the_server_restarting() {
        let (app, path) = app("restart");
        let (host, ada, _) = ready(&app).await;
        ok(
            &app,
            "PUT",
            &format!("/api/p/{ada}/pick"),
            Some(json!({"value": 4})),
        )
        .await;
        ok(&app, "POST", &format!("/api/h/{host}/reveal"), None).await;

        let again = router(Rooms::load(&path, 4));
        let view = ok(&again, "GET", &format!("/api/h/{host}"), None).await["view"].clone();
        assert_eq!(view["outcome"]["picks"][0]["value"], 4.0);

        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn the_page_is_served_at_every_link_a_phone_might_be_handed() {
        let (app, path) = app("pages");
        for uri in ["/", "/h/abc", "/j/abc", "/p/abc"] {
            let request = Request::builder().uri(uri).body(Body::empty()).unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{uri}");
        }
        let _ = std::fs::remove_file(&path);
    }
}
