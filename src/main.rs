use std::{
    collections::HashMap,
    error::Error,
    sync::{Arc, Mutex},
};

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Response,
    routing::get,
    Router,
};
use reqwest::Client;
use serde::Deserialize;

#[derive(Clone)]
struct AppState {
    lists: HashMap<String, String>,
    client: Client,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let state = AppState {
        lists: HashMap::new(),
        client: Client::new(),
    };
    let router = Router::new()
        .route("/lists/:name", get(smart_list))
        .with_state(Arc::new(Mutex::new(state)));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, router).await?;
    Ok(())
}

async fn smart_list(
    Path(name): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<Arc<Mutex<AppState>>>,
) -> Result<Response, (StatusCode, String)> {
    // getting list data...
    let cloned_state;
    {
        cloned_state = state.lock().unwrap().clone();
    }
    let list_id;
    if let Some(x) = cloned_state.lists.get_key_value(&name) {
        // we know what list this is...
        list_id = x.1.clone()
    } else {
        // we need to figure this out...
        list_id = match get_list(&name, cloned_state.client.clone()).await {
            Ok(Some(x)) => {
                let mut state = state.lock().unwrap();
                state.lists.insert(name, x.clone());
                x
            }
            Ok(None) => return Err((StatusCode::NOT_FOUND, "".into())),
            Err(x) => {
                println!("{x:?}");
                return Err((StatusCode::NOT_FOUND, x.to_string()));
            }
        };
    }
    let request_url = format!("https://moth.social/api/v1/timelines/list/{}", list_id);
    let response = match cloned_state
        .client
        .get(request_url)
        .header("Authorization", dotenv::var("RELAY_TOKEN").unwrap())
        .query(&params)
        .send()
        .await
    {
        Ok(x) => x,
        Err(x) => return Err((StatusCode::INTERNAL_SERVER_ERROR, x.to_string())),
    };
    let response_body = match response.text().await {
        Ok(x) => x,
        Err(x) => return Err((StatusCode::INTERNAL_SERVER_ERROR, x.to_string())),
    };
    let response = Response::builder()
        .status(200)
        .header("Content-Type", "application/json")
        .body(response_body.into())
        .unwrap();
    return Ok(response);
}

#[derive(Deserialize)]
struct List {
    id: String,
    title: String,
}

async fn get_list(list_name: &str, client: Client) -> Result<Option<String>, Box<dyn Error>> {
    let all_lists: Vec<List> = client
        .get("https://moth.social/api/v1/lists")
        .header("Authorization", dotenv::var("RELAY_TOKEN").unwrap())
        .send()
        .await?
        .json()
        .await?;
    for list in all_lists {
        if list.title == list_name {
            return Ok(Some(list.id));
        }
    }
    Ok(None)
}
