use crate::*;
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Clear, Wrap},
};

pub(super) fn matching(targets: &[Target], query: &str) -> Vec<usize> {
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
            let mut score = 0;
            let mut fuzzy = 0;
            for term in &terms {
                if let Some(start) = label.find(term) {
                    score += start;
                    continue;
                }
                let mut chars = label.chars().enumerate();
                let mut first = None;
                let mut last = 0;
                for wanted in term.chars() {
                    let (position, _) = chars.find(|(_, c)| *c == wanted)?;
                    first.get_or_insert(position);
                    last = position;
                }
                fuzzy += 1;
                score += last - first.unwrap_or(0);
            }
            Some((fuzzy, score, i))
        })
        .collect();
    matches.sort_unstable();
    matches.into_iter().map(|(_, _, i)| i).collect()
}

pub(super) fn comments(home: &Path, user: &str) -> HashMap<String, String> {
    parse_comments(
        &fs::read_to_string(home.join(".ssh/config")).unwrap_or_default(),
        user,
    )
}

fn parse_comments(text: &str, user: &str) -> HashMap<String, String> {
    let mut result = HashMap::new();
    let mut hosts = Vec::new();
    let mut login = None;
    let mut notes = Vec::new();
    let mut pending = Vec::new();
    let flush = |hosts: &Vec<String>,
                 login: &Option<String>,
                 notes: &Vec<String>,
                 result: &mut HashMap<String, String>| {
        if let Some(login) = login {
            for host in hosts {
                if !host.contains(['*', '?', '!'])
                    && !notes.is_empty()
                    && let Some(target) = target(&format!("{login}@{host}"), user)
                {
                    result.insert(target.host, notes.join("\n"));
                }
            }
        }
    };
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(comment) = trimmed.strip_prefix('#') {
            let comment = comment
                .trim()
                .chars()
                .filter(|c| !c.is_control())
                .collect::<String>();
            if line.starts_with(char::is_whitespace) && !hosts.is_empty() {
                notes.push(comment);
            } else {
                pending.push(comment);
            }
            continue;
        }
        let (code, inline) = trimmed
            .split_once('#')
            .map_or((trimmed, None), |(a, b)| (a, Some(b.trim())));
        let words: Vec<_> = code.split_whitespace().collect();
        if words
            .first()
            .is_some_and(|w| w.eq_ignore_ascii_case("host") || w.eq_ignore_ascii_case("match"))
        {
            flush(&hosts, &login, &notes, &mut result);
            hosts = if words[0].eq_ignore_ascii_case("host") {
                words[1..].iter().map(|s| (*s).into()).collect()
            } else {
                Vec::new()
            };
            login = None;
            notes = std::mem::take(&mut pending);
        } else if words
            .first()
            .is_some_and(|w| w.eq_ignore_ascii_case("user"))
            && words.len() > 1
        {
            login = Some(words[1].to_owned());
        }
        if !words.is_empty() {
            notes.append(&mut pending);
            if let Some(comment) = inline {
                notes.push(comment.chars().filter(|c| !c.is_control()).collect());
            }
        }
    }
    flush(&hosts, &login, &notes, &mut result);
    result
}

pub(super) fn hint(frame: &mut Frame, area: Rect, text: &str) {
    if area.width < 45 || area.height < 5 {
        return;
    }
    let width = (area.width / 2).min(60);
    let paragraph = Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(theme::USERNAME).bg(theme::SURFACE));
    let height = paragraph
        .line_count(width)
        .min(usize::from(area.height / 3))
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
        assert_eq!(matching(&targets, "ALICE east"), vec![3]);
        assert_eq!(matching(&targets, "prd"), vec![0, 1, 3, 2]);
        assert_eq!(matching(&targets, "prod"), vec![0, 1, 3, 2]);
        assert_eq!(matching(&targets, ""), vec![0, 1, 2, 3]);
        assert!(matching(&targets, "zz").is_empty());
    }
    #[test]
    fn comments_follow_explicit_user_and_alias() {
        let parsed = parse_comments(
            "# Production\nHost prod p\n User alice # Deploy user\n # Back up first\n# Staging\nHost stage\n User bob\nHost implicit\n # Not for root\nHost *\n User wildcard\n",
            "bob",
        );
        assert_eq!(
            parsed["alice@prod"],
            "Production\nDeploy user\nBack up first"
        );
        assert_eq!(parsed["alice@p"], parsed["alice@prod"]);
        assert_eq!(parsed["stage"], "Staging");
        assert!(!parsed.contains_key("root@prod"));
        assert!(!parsed.contains_key("implicit"));
    }
}
