pub fn normalize_title(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 4);
    let chars: Vec<char> = input.chars().collect();

    for (i, &c) in chars.iter().enumerate() {
        if c == '—' {
            if !out.ends_with(' ') && !out.is_empty() {
                out.push(' ');
            }
            out.push('—');
            let next = chars.get(i + 1).copied();
            if next.is_some() && next != Some(' ') {
                out.push(' ');
            }
        } else {
            out.push(c);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_unspaced_em_dash() {
        assert_eq!(
            normalize_title("Energy—Mind and Matter"),
            "Energy — Mind and Matter"
        );
    }

    #[test]
    fn leaves_already_spaced_em_dash() {
        assert_eq!(
            normalize_title("Energy — Mind and Matter"),
            "Energy — Mind and Matter"
        );
    }

    #[test]
    fn handles_multiple_em_dashes() {
        assert_eq!(normalize_title("A—B—C"), "A — B — C");
    }

    #[test]
    fn preserves_clean_title() {
        assert_eq!(
            normalize_title("The Universal Father"),
            "The Universal Father"
        );
    }

    #[test]
    fn handles_leading_or_trailing_space_on_one_side() {
        assert_eq!(normalize_title("word— other"), "word — other");
        assert_eq!(normalize_title("word —other"), "word — other");
    }

    #[test]
    fn preserves_double_hyphen_untouched() {
        assert_eq!(normalize_title("A--B"), "A--B");
    }

    #[test]
    fn real_paper_42() {
        assert_eq!(
            normalize_title("Energy—Mind and Matter"),
            "Energy — Mind and Matter"
        );
    }

    #[test]
    fn real_paper_118() {
        assert_eq!(
            normalize_title("Supreme and Ultimate—Time and Space"),
            "Supreme and Ultimate — Time and Space"
        );
    }
}
