use owo_colors::OwoColorize;

use crate::service::ls::LsResult;

pub fn render(result: &LsResult) -> String {
    let mut lines = vec![format!(
        "{} {} {}",
        "memento".bold(),
        "ls".cyan().bold(),
        format!("listing {}", result.target).green()
    )];

    if result.entries.is_empty() {
        lines.push(format!(
            "{} {}",
            "status".dimmed(),
            "no indexed resources".yellow()
        ));
        return lines.join("\n");
    }

    for entry in &result.entries {
        if let Some(live_state) = &entry.live_state {
            lines.push(format!(
                "{} {} {}",
                entry.uri.cyan(),
                entry.kind.dimmed(),
                render_live_state(live_state)
            ));

            if needs_reindex(live_state) {
                if let Some(source_path) = &entry.source_path {
                    lines.push(format!(
                        "{} {}",
                        "action".dimmed(),
                        format!("run `memento reindex {source_path}`").yellow()
                    ));
                }
            }
        } else {
            lines.push(format!("{} {}", entry.uri.cyan(), entry.kind.dimmed()));
        }
    }

    lines.join("\n")
}

fn render_live_state(state: &str) -> String {
    match state {
        "ok" => "ok".green().to_string(),
        "modified" => "modified".yellow().to_string(),
        "deleted" => "deleted".red().to_string(),
        _ => "unreadable".red().to_string(),
    }
}

fn needs_reindex(state: &str) -> bool {
    matches!(state, "modified" | "deleted")
}
