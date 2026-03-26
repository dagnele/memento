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

    let mut actions = Vec::new();

    for entry in &result.entries {
        if let Some(live_state) = &entry.live_state {
            lines.push(format!(
                "{} {} {}",
                entry.uri.cyan(),
                entry.kind.dimmed(),
                render_live_state(live_state)
            ));

            if let Some(action) =
                render_action(live_state, entry.source_path.as_deref(), &entry.uri)
            {
                actions.push(action);
            }
        } else {
            lines.push(format!(
                "{} {}",
                entry.uri.blue().bold(),
                entry.kind.dimmed()
            ));
        }
    }

    if !actions.is_empty() {
        lines.push(String::new());
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
        "modified" => "modified".yellow().to_string(),
        "deleted" => "deleted".red().to_string(),
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
