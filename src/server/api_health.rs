//! The Documentation health route: one list of the pages that need
//! attention, from the page index.

use std::sync::Arc;

use axum::extract::State;

use super::AppState;
use super::api::{ApiResult, blocking, today_for};
use crate::error::CairnError;
use crate::health;

pub async fn health(State(state): State<Arc<AppState>>) -> ApiResult {
    blocking(state, |st| {
        let ws = st.workspace()?;
        // Look at the folder now, not a few seconds ago: this screen is
        // about what is true at the moment.
        ws.refresh_index(true);
        let report = {
            let index = ws.index.lock().expect("index lock");
            health::report(&index, &today_for(st))
        };
        serde_json::to_value(report)
            .map_err(|e| CairnError::Io(format!("The report could not be made: {e}")))
    })
    .await
}
