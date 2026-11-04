//! Where the user is in the app. Every screen is a [`Route`]; the sidebar, links inside screens
//! and keyboard shortcuts all go through [`Router::go`], and [`Router::back`] retraces the steps.
//! docs/VISION.md lists the routes to come and what each screen holds.

use crate::workspace::TableKey;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// Opening files: the drop zone and the recent files.
    Home,
    /// The tables open, and one of them in the grid.
    Data(Option<TableKey>),
    /// Joining tables into files: a workbook, a file each, or one stacked table.
    Combine,
    Settings,
}

impl Route {
    /// The sidebar's entry for this route: two routes are in the same section when it is the same.
    pub fn section(self) -> Section {
        match self {
            Route::Home => Section::Home,
            Route::Data(_) => Section::Data,
            Route::Combine => Section::Combine,
            Route::Settings => Section::Settings,
        }
    }
}

/// A sidebar entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Home,
    Data,
    Combine,
    Settings,
}

/// How far back [`Router::back`] reaches.
const HISTORY: usize = 50;

#[derive(Debug)]
pub struct Router {
    current: Route,
    history: Vec<Route>,
}

impl Default for Router {
    fn default() -> Self {
        Self { current: Route::Home, history: Vec::new() }
    }
}

impl Router {
    pub fn current(&self) -> Route {
        self.current
    }

    pub fn go(&mut self, route: Route) {
        if route == self.current {
            return;
        }
        self.history.push(self.current);
        if self.history.len() > HISTORY {
            self.history.remove(0);
        }
        self.current = route;
    }

    /// Goes to a section from the sidebar: Data comes back to the table it last showed.
    pub fn go_to_section(&mut self, section: Section, last_table: Option<TableKey>) {
        let route = match section {
            Section::Home => Route::Home,
            Section::Data => Route::Data(last_table),
            Section::Combine => Route::Combine,
            Section::Settings => Route::Settings,
        };
        self.go(route);
    }

    pub fn can_go_back(&self) -> bool {
        !self.history.is_empty()
    }

    pub fn back(&mut self) {
        if let Some(route) = self.history.pop() {
            self.current = route;
        }
    }

    /// Forgets steps that lead to a table no longer open, and leaves it if it is on screen.
    pub fn forget(&mut self, gone: impl Fn(TableKey) -> bool) {
        let stale = |r: &Route| matches!(r, Route::Data(Some(key)) if gone(*key));
        self.history.retain(|r| !stale(r));
        self.history.dedup();
        if stale(&self.current) {
            self.current = Route::Data(None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T1: TableKey = TableKey { source: 1, index: 0 };
    const T2: TableKey = TableKey { source: 2, index: 0 };

    #[test]
    fn back_retraces_the_steps() {
        let mut router = Router::default();
        router.go(Route::Data(Some(T1)));
        router.go(Route::Data(Some(T1)));
        router.go(Route::Combine);
        router.back();
        assert_eq!(router.current(), Route::Data(Some(T1)));
        router.back();
        assert_eq!(router.current(), Route::Home);
        assert!(!router.can_go_back());
    }

    #[test]
    fn closed_tables_leave_the_history() {
        let mut router = Router::default();
        router.go(Route::Data(Some(T1)));
        router.go(Route::Data(Some(T2)));
        router.forget(|k| k.source == 2);
        assert_eq!(router.current(), Route::Data(None));
        router.back();
        assert_eq!(router.current(), Route::Data(Some(T1)));
    }

    #[test]
    fn sections_group_routes() {
        assert_eq!(Route::Data(Some(T1)).section(), Route::Data(None).section());
        let mut router = Router::default();
        router.go_to_section(Section::Data, Some(T2));
        assert_eq!(router.current(), Route::Data(Some(T2)));
    }
}
