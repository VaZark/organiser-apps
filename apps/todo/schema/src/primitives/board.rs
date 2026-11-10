//! The kanban board: which columns a view has, and which tasks sit in each.

use super::status::Status;
use super::task::Task;

/// Which columns the board shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BoardView {
    /// To do, in progress and done.
    #[default]
    All,
    /// Only what is finished: done and won't do.
    Closed,
}

impl BoardView {
    /// The columns, left to right.
    pub fn columns(self) -> &'static [Status] {
        match self {
            BoardView::All => &[Status::Open, Status::InProgress, Status::Done],
            BoardView::Closed => &[Status::Done, Status::Dismissed],
        }
    }

    /// The column next to `status` in this view: `step` is -1 for left, 1 for right. `None` at an
    /// edge, or when the status is not a column here.
    pub fn neighbor(self, status: Status, step: i32) -> Option<Status> {
        let cols = self.columns();
        let at = cols.iter().position(|s| *s == status)? as i32;
        usize::try_from(at + step)
            .ok()
            .and_then(|i| cols.get(i))
            .copied()
    }
}

/// The tasks in each column, in the order given. A task whose status has no column in this view
/// is left out.
pub fn columns(tasks: &[Task], view: BoardView) -> Vec<(Status, Vec<&Task>)> {
    view.columns()
        .iter()
        .map(|s| (*s, tasks.iter().filter(|t| t.status == *s).collect()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighbors_stop_at_the_edges() {
        let v = BoardView::All;
        assert_eq!(v.neighbor(Status::Open, 1), Some(Status::InProgress));
        assert_eq!(v.neighbor(Status::InProgress, -1), Some(Status::Open));
        assert_eq!(v.neighbor(Status::Open, -1), None);
        assert_eq!(v.neighbor(Status::Done, 1), None);
        assert_eq!(
            v.neighbor(Status::Dismissed, 1),
            None,
            "not a column in this view"
        );
        assert_eq!(
            BoardView::Closed.neighbor(Status::Done, 1),
            Some(Status::Dismissed)
        );
    }
}
