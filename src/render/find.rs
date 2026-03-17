use owo_colors::OwoColorize;

use crate::service::find::FindResult;

pub fn render(result: &FindResult) -> String {
    let mut lines = vec![format!(
        "{} {} {}",
        "memento".bold(),
        "find".cyan().bold(),
        format!("searching for {}", result.query).green()
    )];

    if result.matches.is_empty() {
        lines.push(format!(
            "{} {}",
            "status".dimmed(),
            "no matches found".yellow()
        ));
        return lines.join("\n");
    }

    for item in &result.matches {
        lines.push(format!(
            "{} {} {} {}",
            item.uri.cyan(),
            format!("distance={:.6}", item.distance).dimmed(),
            format!("{}:{}", item.layer, item.scope).dimmed(),
            item.locator.dimmed()
        ));
        lines.push(format!(
            "{} {} {}",
            "kind".dimmed(),
            item.kind,
            format!("namespace={}", item.namespace).dimmed()
        ));

        if let Some(live_state) = &item.live_state {
            lines.push(format!(
                "{} {}",
                "state".dimmed(),
                render_live_state(live_state)
            ));
        }

        if let Some(source_path) = &item.source_path {
            lines.push(format!("{} {}", "path".dimmed(), source_path));

            if item.live_state.as_deref().is_some_and(needs_reindex) {
                lines.push(format!(
                    "{} {}",
                    "action".dimmed(),
                    format!("run `memento reindex {source_path}`").yellow()
                ));
            }
        }

        if let Some(preview) = &item.preview {
            lines.push(format!("{} {}", "preview".dimmed(), preview));
        }

        lines.push(String::new());
    }

    lines.join("\n")
}

fn render_live_state(state: &str) -> String {
    match state {
        "ok" => "ok".green().to_string(),
        "modified" => "modified needs_reindex".yellow().to_string(),
        "deleted" => "deleted needs_reindex".red().to_string(),
        _ => "unreadable".red().to_string(),
    }
}

fn needs_reindex(state: &str) -> bool {
    matches!(state, "modified" | "deleted")
}
