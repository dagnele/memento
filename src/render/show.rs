use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Attribute, Cell, Color, ContentArrangement, Table};
use owo_colors::OwoColorize;

use crate::service::show::{ShowItem, ShowResult};

pub fn render(result: &ShowResult) -> String {
    let mut lines = vec![
        format!(
            "{} {} {}",
            "memento".bold(),
            "show".cyan().bold(),
            "entry found".green()
        ),
        String::new(),
    ];

    if let Some(item) = &result.item {
        lines.push(format!("{}", render_item_table(item)));
        if let Some(action) = render_action(&item.live_state, &item.source_path, &item.uri) {
            lines.push(String::new());
            lines.push(format!("{}", "actions".dimmed()));
            lines.push(format!("  {}. {}", 1, action.yellow()));
        }
        return lines.join("\n");
    }

    if let Some(entry) = &result.virtual_entry {
        lines.push(format!("{}", render_virtual_entry_table(entry)));
    }

    lines.join("\n")
}

fn render_item_table(item: &ShowItem) -> Table {
    let mut table = base_table();

    table.add_row(vec!["uri", item.uri.as_str()]);
    table.add_row(vec!["namespace", item.namespace.as_str()]);
    table.add_row(vec!["kind", item.kind.as_str()]);
    table.add_row(vec!["live_state", item.live_state.as_str()]);
    table.add_row(vec![
        Cell::new("source_path"),
        Cell::new(item.source_path.as_str()),
    ]);
    table.add_row(vec![
        Cell::new("file_size_bytes"),
        Cell::new(item.file_size_bytes.as_str()),
    ]);
    table.add_row(vec![
        Cell::new("modified_at"),
        Cell::new(item.modified_at.as_str()),
    ]);
    table.add_row(vec![Cell::new("layers"), Cell::new(render_layers(item))]);
    table.add_row(vec!["created_at", item.created_at.as_str()]);
    table.add_row(vec!["updated_at", item.updated_at.as_str()]);

    table
}

fn render_virtual_entry_table(entry: &crate::service::show::VirtualEntry) -> Table {
    let mut table = base_table();
    table.add_row(vec!["uri", entry.uri.as_str()]);
    table.add_row(vec!["entry_type", entry.entry_type.as_str()]);
    table
}

fn render_layers(item: &ShowItem) -> String {
    if item.layers.is_empty() {
        return "-".to_string();
    }

    item.layers
        .iter()
        .map(|layer| format!("{}:{}", layer.layer, layer.storage_kind))
        .collect::<Vec<_>>()
        .join(", ")
}

fn base_table() -> Table {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("field")
                .fg(Color::Cyan)
                .add_attribute(Attribute::Bold),
            Cell::new("value")
                .fg(Color::Green)
                .add_attribute(Attribute::Bold),
        ]);
    table
}

fn render_action(state: &str, source_path: &str, uri: &str) -> Option<String> {
    match state {
        "modified" => Some(format!("run `memento reindex {source_path}`")),
        "deleted" => Some(format!("run `memento rm {uri}`")),
        _ => None,
    }
}
