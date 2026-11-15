//! Natural ordering of file names: case-insensitive, and runs of digits compare
//! as numbers, so "file2" sorts before "file10".

use std::cmp::Ordering;

pub fn compare(a: &str, b: &str) -> Ordering {
    let mut left = a.chars().peekable();
    let mut right = b.chars().peekable();
    loop {
        match (left.peek().copied(), right.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let n = take_number(&mut left);
                let m = take_number(&mut right);
                // Compare by value: strip leading zeros, then length, then digits.
                let (n_trim, m_trim) = (n.trim_start_matches('0'), m.trim_start_matches('0'));
                let ord = n_trim.len().cmp(&m_trim.len()).then_with(|| n_trim.cmp(m_trim));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(x), Some(y)) => {
                let ord = fold(x).cmp(&fold(y));
                if ord != Ordering::Equal {
                    return ord;
                }
                left.next();
                right.next();
            }
        }
    }
}

fn take_number(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut out = String::new();
    while let Some(c) = chars.peek().copied().filter(char::is_ascii_digit) {
        out.push(c);
        chars.next();
    }
    out
}

fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(mut names: Vec<&str>) -> Vec<&str> {
        names.sort_by(|a, b| compare(a, b));
        names
    }

    #[test]
    fn numbers_sort_by_value() {
        assert_eq!(
            sorted(vec!["file10", "file2", "file1"]),
            vec!["file1", "file2", "file10"]
        );
    }

    #[test]
    fn case_is_ignored_but_ties_are_stable() {
        assert_eq!(
            sorted(vec!["banana", "Apple", "apple"]),
            vec!["Apple", "apple", "banana"]
        );
    }

    #[test]
    fn leading_zeros_compare_equal_in_value() {
        assert_eq!(compare("img007", "img7"), "img007".cmp("img7"));
        assert_eq!(compare("img007", "img8"), Ordering::Less);
    }

    #[test]
    fn prefixes_come_first() {
        assert_eq!(compare("note", "notes"), Ordering::Less);
    }
}
