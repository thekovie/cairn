//! The Documentation health route: one list of the pages that need
//! attention, from the page index.

use std::sync::Arc;

use axum::extract::State;

use super::AppState;
use super::api::{ApiResult, blocking, today_for};
use crate::error::CairnError;
use crate::{health, linkcheck};

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

/// Look through every page for links and pictures that lead nowhere. Done
/// only when asked: it reads every page once.
pub async fn check(State(state): State<Arc<AppState>>) -> ApiResult {
    blocking(state, |st| {
        let ws = st.workspace()?;
        ws.refresh_index(true);
        // Take the list and let go of the index: the check can take a while
        // on a slow share, and other screens shouldn't wait for it.
        let pages = ws.index.lock().expect("index lock").all();
        let report = linkcheck::check_workspace(&ws.root, &pages);
        serde_json::to_value(report)
            .map_err(|e| CairnError::Io(format!("The result could not be made: {e}")))
    })
    .await
}
