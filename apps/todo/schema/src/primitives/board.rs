//! The kanban board: which columns a view has, and which tasks sit in each.

use super::status::{Status, Statuses};
use super::task::Task;

/// Which columns the board shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BoardView {
    /// The open statuses only: finished work is out of sight.
    #[default]
    Open,
    /// Every status, finished ones too.
    All,
}

impl BoardView {
    /// The columns, left to right.
    pub fn columns(self, statuses: &Statuses) -> Vec<&Status> {
        match self {
            BoardView::Open => statuses.open().collect(),
            // Finished work after open work, whatever order they were made in.
            BoardView::All => statuses.open().chain(statuses.closed()).collect(),
        }
    }

    /// The column next to `status` in this view: `step` is -1 for left, 1 for right. `None` at an
    /// edge, or when the status is not a column here.
    pub fn neighbor(self, statuses: &Statuses, status: i64, step: i32) -> Option<i64> {
        let cols = self.columns(statuses);
        let at = cols.iter().position(|s| s.id == status)? as i32;
        usize::try_from(at + step)
            .ok()
            .and_then(|i| cols.get(i))
            .map(|s| s.id)
    }
}

/// The tasks in each column, in the order given. A task whose status has no column in this view
/// is left out; one without a status is in the first open column.
pub fn columns<'a>(
    tasks: &'a [Task],
    view: BoardView,
    statuses: &'a Statuses,
) -> Vec<(&'a Status, Vec<&'a Task>)> {
    view.columns(statuses)
        .into_iter()
        .map(|s| {
            let here = tasks
                .iter()
                .filter(|t| statuses.resolve(t.status).is_some_and(|r| r.id == s.id))
                .collect();
            (s, here)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(id: i64, closed: bool) -> Status {
        Status {
            id,
            name: format!("s{id}"),
            color: None,
            closed,
            position: id,
        }
    }

    /// 1 and 2 open, 3 and 4 closed.
    fn set() -> Statuses {
        Statuses::new(vec![st(1, false), st(2, false), st(3, true), st(4, true)])
    }

    fn ids(v: Vec<&Status>) -> Vec<i64> {
        v.into_iter().map(|s| s.id).collect()
    }

    #[test]
    fn the_board_hides_finished_statuses_until_asked_to_show_everything() {
        assert_eq!(ids(BoardView::Open.columns(&set())), [1, 2]);
        assert_eq!(ids(BoardView::All.columns(&set())), [1, 2, 3, 4]);
    }

    #[test]
    fn everything_lists_open_columns_before_finished_ones_whatever_the_order() {
        // A finished status made before an open one still comes after it.
        let s = Statuses::new(vec![st(1, false), st(2, true), st(3, false)]);
        assert_eq!(ids(BoardView::All.columns(&s)), [1, 3, 2]);
    }

    #[test]
    fn neighbors_stop_at_the_edges() {
        let s = set();
        let open = BoardView::Open;
        assert_eq!(open.neighbor(&s, 1, 1), Some(2));
        assert_eq!(open.neighbor(&s, 2, -1), Some(1));
        assert_eq!(open.neighbor(&s, 1, -1), None);
        assert_eq!(
            open.neighbor(&s, 2, 1),
            None,
            "the edge of the open columns"
        );
        assert_eq!(open.neighbor(&s, 3, 1), None, "not a column in this view");
        let all = BoardView::All;
        assert_eq!(all.neighbor(&s, 2, 1), Some(3), "into the finished columns");
        assert_eq!(all.neighbor(&s, 3, 1), Some(4));
        assert_eq!(all.neighbor(&s, 4, 1), None);
    }

    #[test]
    fn a_task_without_a_status_sits_in_the_first_open_column() {
        let s = set();
        let mut t = crate::primitives::task::Task::from(&organiser_engine::Item {
            id: 1,
            kind: "task".into(),
            parent_id: None,
            title: "t".into(),
            fields: Default::default(),
            position: 0,
            archived: false,
            created: 0,
            updated: 0,
            tags: vec![],
        });
        let tasks = [t.clone()];
        let cols = columns(&tasks, BoardView::Open, &s);
        assert_eq!(cols[0].1.len(), 1);
        t.status = Some(4);
        let tasks = [t];
        assert!(
            columns(&tasks, BoardView::Open, &s)
                .iter()
                .all(|c| c.1.is_empty())
        );
        // Closed, it has no column unless the board shows everything.
        assert_eq!(
            columns(&tasks, BoardView::All, &s)
                .iter()
                .find(|c| c.0.id == 4)
                .map(|c| c.1.len()),
            Some(1)
        );
    }
}
