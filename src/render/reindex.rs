use owo_colors::OwoColorize;

use crate::service::reindex::ReindexResult;

pub fn render(result: &ReindexResult) -> String {
    let mut lines = vec![format!(
        "{} {} {}",
        "memento".bold(),
        "reindex".cyan().bold(),
        format!(
            "refreshed {} resource(s)",
            result.indexed_paths.len() + result.metadata_only_paths.len()
        )
        .green()
    )];

    for path in &result.indexed_paths {
        lines.push(format!("{} {}", "reindexed".dimmed(), path.cyan()));
    }

    for path in &result.metadata_only_paths {
        lines.push(format!(
            "{} {} {}",
            "reindexed".dimmed(),
            path.cyan(),
            "metadata only".dimmed()
        ));
    }

    for path in &result.deleted_paths {
        lines.push(format!("{} {}", "removed".dimmed(), path.red()));
    }

    lines.join("\n")
}
