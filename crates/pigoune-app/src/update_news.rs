#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateNews {
    Available,
    Installing(u32),
    Installed,
    Failed,
}

const PROGRESS_RUNNING: u32 = 0;
const PROGRESS_DONE: u32 = 2;
const PROGRESS_FAILED: u32 = 3;
const FULL_PROGRESS: u32 = 100;

#[must_use]
pub fn news_from_commits(running: &str, local: &str, remote: &str) -> Option<UpdateNews> {
    if !local.is_empty() && local != running {
        Some(UpdateNews::Installed)
    } else if !remote.is_empty() && remote != local {
        Some(UpdateNews::Available)
    } else {
        None
    }
}

#[must_use]
pub fn news_from_progress(status: u32, progress: u32) -> Option<UpdateNews> {
    match status {
        PROGRESS_RUNNING => Some(UpdateNews::Installing(progress.min(FULL_PROGRESS))),
        PROGRESS_DONE => Some(UpdateNews::Installed),
        PROGRESS_FAILED => Some(UpdateNews::Failed),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{UpdateNews, news_from_commits, news_from_progress};

    #[test]
    fn a_newer_remote_version_is_available() {
        assert_eq!(
            news_from_commits("a", "a", "b"),
            Some(UpdateNews::Available)
        );
    }

    #[test]
    fn an_up_to_date_version_has_no_news() {
        assert_eq!(news_from_commits("a", "a", "a"), None);
        assert_eq!(news_from_commits("a", "a", ""), None);
    }

    #[test]
    fn a_version_installed_elsewhere_waits_for_a_restart() {
        assert_eq!(
            news_from_commits("a", "b", "b"),
            Some(UpdateNews::Installed)
        );
    }

    #[test]
    fn progress_follows_the_installation() {
        assert_eq!(news_from_progress(0, 40), Some(UpdateNews::Installing(40)));
        assert_eq!(
            news_from_progress(0, 250),
            Some(UpdateNews::Installing(100))
        );
        assert_eq!(news_from_progress(2, 100), Some(UpdateNews::Installed));
        assert_eq!(news_from_progress(3, 10), Some(UpdateNews::Failed));
    }

    #[test]
    fn nothing_to_install_has_no_news() {
        assert_eq!(news_from_progress(1, 0), None);
    }
}
