use std::sync::Mutex;

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum AppView {
    Convert,
    Presets,
    #[serde(rename = "magic")]
    WatchedFolders,
}

#[derive(Default)]
pub(crate) struct AppNavigationState {
    inner: Mutex<NavigationInner>,
}

#[derive(Default)]
struct NavigationInner {
    pending_view: Option<AppView>,
    frontend_ready: bool,
}

impl AppNavigationState {
    pub(crate) fn take_initial_view(&self) -> Option<AppView> {
        let mut inner = self.inner.lock().expect("navigation state poisoned");
        inner.frontend_ready = true;
        inner.pending_view.take()
    }

    // Return whether the frontend is listening. Before that, retain the latest choice.
    pub(crate) fn request_view(&self, view: AppView) -> bool {
        let mut inner = self.inner.lock().expect("navigation state poisoned");
        if inner.frontend_ready {
            true
        } else {
            inner.pending_view = Some(view);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_the_latest_view_until_the_frontend_is_listening() {
        let state = AppNavigationState::default();
        assert!(!state.request_view(AppView::Presets));
        assert!(!state.request_view(AppView::WatchedFolders));
        assert_eq!(state.take_initial_view(), Some(AppView::WatchedFolders));
        assert_eq!(state.take_initial_view(), None);
        assert!(state.request_view(AppView::Convert));
        assert_eq!(state.take_initial_view(), None);
    }
}
