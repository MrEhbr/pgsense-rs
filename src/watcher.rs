use std::path::Path;

use anyhow::{Context, Result};
use notify::{EventKind, RecommendedWatcher, Watcher, event::ModifyKind};
use tokio::sync::mpsc;
use tracing::warn;

/// Events are debounced via `try_send` — if the channel already has a pending
/// notification, duplicates are silently dropped.
pub fn watch_file(path: &Path) -> Result<(mpsc::Receiver<()>, RecommendedWatcher)> {
    let (tx, rx) = mpsc::channel(1);

    // Canonicalize so the path matches what notify reports (e.g. /var →
    // /private/var on macOS)
    let canonical = path
        .canonicalize()
        .with_context(|| format!("failed to canonicalize rules path: {}", path.display()))?;
    let watched_path = canonical.clone();

    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| match res {
        Ok(event) => {
            let is_data_change = matches!(
                event.kind,
                EventKind::Modify(ModifyKind::Data(_) | ModifyKind::Any) | EventKind::Create(_)
            );
            if is_data_change && event.paths.iter().any(|p| p == &watched_path) {
                let _ = tx.try_send(());
            }
        },
        Err(e) => warn!(error = %e, "file watcher error"),
    })
    .context("failed to create file watcher")?;

    let parent = canonical
        .parent()
        .context("rules file has no parent directory")?;
    watcher
        .watch(parent, notify::RecursiveMode::NonRecursive)
        .context("failed to watch rules file directory")?;

    Ok((rx, watcher))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn detects_file_modification() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rules.toml");
        std::fs::write(&path, "initial").unwrap();

        let (mut rx, _watcher) = watch_file(&path).unwrap();

        // FSEvents on macOS needs time to register the watch
        tokio::time::sleep(Duration::from_millis(500)).await;
        std::fs::write(&path, "modified").unwrap();

        let result = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await;
        assert!(result.is_ok(), "should receive a change notification");
    }

    #[tokio::test]
    async fn no_event_without_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rules.toml");
        std::fs::write(&path, "stable").unwrap();

        let (mut rx, _watcher) = watch_file(&path).unwrap();

        let result = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await;
        assert!(result.is_err(), "should timeout with no change event");
    }
}
