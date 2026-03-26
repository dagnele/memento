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

    let mut actions = Vec::new();

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

            if let Some(action) = item
                .live_state
                .as_deref()
                .and_then(|s| render_action(s, Some(source_path), &item.uri))
            {
                actions.push(action);
            }
        }

        if let Some(preview) = &item.preview {
            lines.push(format!("{} {}", "preview".dimmed(), preview));
        }

        lines.push(String::new());
    }

    if !actions.is_empty() {
        lines.push(format!("{}", "actions".dimmed()));
        for (i, action) in actions.iter().enumerate() {
            lines.push(format!("  {}. {}", i + 1, action.yellow()));
        }
    }

    lines.join("\n")
}

fn render_live_state(state: &str) -> String {
    match state {
        "ok" => "ok".green().to_string(),
        "modified" => "modified needs_reindex".yellow().to_string(),
        "deleted" => "deleted needs_removal".red().to_string(),
        _ => "unreadable".red().to_string(),
    }
}

fn render_action(state: &str, source_path: Option<&str>, uri: &str) -> Option<String> {
    match state {
        "modified" => source_path.map(|p| format!("run `memento reindex {p}`")),
        "deleted" => Some(format!("run `memento rm {uri}`")),
        _ => None,
    }
}
