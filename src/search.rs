use crate::*;
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Clear, Wrap},
};

pub(super) fn matching(
    targets: &[Target],
    query: &str,
    hints: &HashMap<String, String>,
) -> Vec<usize> {
    let terms: Vec<_> = query
        .to_lowercase()
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    let mut matches: Vec<_> = targets
        .iter()
        .enumerate()
        .filter_map(|(i, target)| {
            let label = target.label().to_lowercase();
            let hint = hints.get(&target.host).map(|text| text.to_lowercase());
            let mut hint_terms = 0;
            let mut score = 0;
            let mut fuzzy = 0;
            for term in &terms {
                let (is_fuzzy, distance) = if let Some(found) = term_score(&label, term) {
                    found
                } else {
                    hint_terms += 1;
                    // Match within a line, not across unrelated directives/comments.
                    hint.as_ref()?
                        .lines()
                        .filter_map(|line| term_score(line, term))
                        .min()?
                };
                fuzzy += is_fuzzy;
                score += distance;
            }
            Some((hint_terms, fuzzy, score, i))
        })
        .collect();
    matches.sort_unstable();
    matches.into_iter().map(|(_, _, _, i)| i).collect()
}

fn term_score(text: &str, term: &str) -> Option<(usize, usize)> {
    if let Some(start) = text.find(term) {
        return Some((0, start));
    }
    let mut chars = text.chars().enumerate();
    let mut first = None;
    let mut last = 0;
    for wanted in term.chars() {
        let (position, _) = chars.find(|(_, c)| *c == wanted)?;
        first.get_or_insert(position);
        last = position;
    }
    Some((1, last - first.unwrap_or(0)))
}

#[cfg(test)]
fn parse_host_config(text: &str, user: &str) -> HashMap<String, String> {
    ssh_config::parse(text, user).hints
}

pub(super) fn hint(frame: &mut Frame, area: Rect, text: &str) {
    if area.width < 45 || area.height < 5 {
        return;
    }
    let width = (area.width / 2).min(80);
    let paragraph = Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(theme::USERNAME).bg(theme::SURFACE));
    let height = paragraph
        .line_count(width)
        .min(usize::from(area.height.saturating_sub(2)))
        .max(1) as u16;
    let rect = Rect::new(area.right() - width, area.bottom() - height, width, height);
    frame.render_widget(Clear, rect);
    frame.render_widget(paragraph, rect);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fuzzy_terms_and_stable_ranking() {
        let targets: Vec<_> = [
            "bob@production-west",
            "alice@prod",
            "root@pxrxyod",
            "alice@production-east",
        ]
        .into_iter()
        .map(|s| target(s, "").unwrap())
        .collect();
        assert_eq!(matching(&targets, "ALICE east", &HashMap::new()), vec![3]);
        assert_eq!(matching(&targets, "prd", &HashMap::new()), vec![0, 1, 3, 2]);
        assert_eq!(
            matching(&targets, "prod", &HashMap::new()),
            vec![0, 1, 3, 2]
        );
        assert_eq!(matching(&targets, "", &HashMap::new()), vec![0, 1, 2, 3]);
        assert!(matching(&targets, "zz", &HashMap::new()).is_empty());
    }
    #[test]
    fn fuzzy_search_matches_hints_and_ranks_hostnames_first() {
        let targets: Vec<_> = ["alice@box", "alice@production", "root@box"]
            .into_iter()
            .map(|name| target(name, "").unwrap())
            .collect();
        let hints = parse_host_config(
            "# Production database\nHost box\n User alice\n HostName 192.0.2.10\n Port 2202\n IdentityFile ~/.ssh/work-key\n",
            "",
        );
        assert_eq!(matching(&targets, "production", &hints), [1, 0]);
        assert_eq!(matching(&targets, "prd", &hints), [1, 0]);
        for query in [
            "DTBSE",
            "192.0.2",
            "2202",
            "work-key",
            "box database",
            "alice 2202",
            "production work-key",
        ] {
            assert_eq!(matching(&targets, query, &hints), [0], "{query}");
        }
        assert!(matching(&targets[2..], "database", &hints).is_empty());
        assert!(matching(&targets, "database missing", &hints).is_empty());
        assert_eq!(matching(&targets, "", &hints), [0, 1, 2]);
        let separate = HashMap::from([("alice@box".into(), "abc\ndef".into())]);
        assert!(matching(&targets, "abcdef", &separate).is_empty());
    }

    #[test]
    fn host_config_preserves_settings_and_comments_for_user_and_alias() {
        let parsed = parse_host_config(
            "# Production\nHost prod p\n User alice # Deploy user\n # Back up first\n# Staging\nHost stage\n User bob\nHost implicit\n # Not for root\n",
            "bob",
        );
        assert_eq!(
            parsed["alice@prod"],
            "# Production\nHost prod p\n User alice # Deploy user\n # Back up first"
        );
        assert_eq!(parsed["alice@p"], parsed["alice@prod"]);
        assert_eq!(parsed["stage"], "# Staging\nHost stage\n User bob");
        assert!(!parsed.contains_key("root@prod"));
        assert!(parsed["implicit"].contains("# Not for root"));
    }
    #[test]
    fn config_hint_handles_leading_comments_quotes_repeated_hosts_and_match() {
        let parsed = parse_host_config(
            "Host first\n User alice\n Port 2201\n\n  # Before second\n  # More context\nHost=second alias # Host note\n User = \"bob\"\n IdentityFile \"~/.ssh/key#work\" # Key note\n # Internal note\n ProxyCommand ssh jump -W %h:%p\n\nHost second\n User bob\n ServerAliveInterval 30\n # Trailing note\nMatch user bob\n # Not part of a Host block\n Port 9999\n",
            "bob",
        );
        assert_eq!(parsed["alice@first"], "Host first\n User alice\n Port 2201");
        let hint = &parsed["second"];
        assert!(
            hint.starts_with("  # Before second\n  # More context\nHost=second alias # Host note")
        );
        assert!(hint.contains("IdentityFile \"~/.ssh/key#work\" # Key note"));
        assert!(hint.contains("# Internal note\n ProxyCommand ssh jump -W %h:%p"));
        assert!(hint.contains("\n\nHost second\n User bob\n ServerAliveInterval 30"));
        assert!(!hint.contains("Match"));
        assert!(!hint.contains("9999"));
        assert!(!parsed["alias"].contains("ServerAliveInterval"));
        let eof = parse_host_config("# Before\nHost end\n User bob\n # Last comment\n", "bob");
        assert!(eof["end"].ends_with(" # Last comment"));
    }
}
