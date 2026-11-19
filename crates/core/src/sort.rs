//! Sorting and grouping rules for listings (DESIGN.md P1-T07, P1-T10).

use std::cmp::Ordering;

use crate::entry::EntryData;
use crate::kind::Kind;
use crate::natural;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortKey {
    #[default]
    Name,
    Size,
    Modified,
    Created,
    Kind,
}

impl SortKey {
    pub const ALL: [SortKey; 5] = [
        SortKey::Name,
        SortKey::Size,
        SortKey::Modified,
        SortKey::Created,
        SortKey::Kind,
    ];

    pub fn id(self) -> &'static str {
        match self {
            SortKey::Name => "name",
            SortKey::Size => "size",
            SortKey::Modified => "modified",
            SortKey::Created => "created",
            SortKey::Kind => "kind",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|key| key.id() == id)
    }
}

/// Orders two entries. Folders always come before files; within each, by
/// `key`, then by name so the order is stable.
pub fn compare(a: &EntryData, b: &EntryData, key: SortKey, descending: bool) -> Ordering {
    let folders_first = b.is_dir.cmp(&a.is_dir);
    if folders_first != Ordering::Equal {
        return folders_first;
    }
    let by_name = || natural::compare(&a.display_name, &b.display_name);
    let ord = match key {
        SortKey::Name => by_name(),
        SortKey::Size => a.size.cmp(&b.size).then_with(by_name),
        SortKey::Modified => a.modified.cmp(&b.modified).then_with(by_name),
        SortKey::Created => a.created.cmp(&b.created).then_with(by_name),
        SortKey::Kind => a.kind.cmp(&b.kind).then_with(by_name),
    };
    if descending { ord.reverse() } else { ord }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GroupBy {
    #[default]
    None,
    Kind,
    Created,
}

impl GroupBy {
    pub const ALL: [GroupBy; 3] = [GroupBy::None, GroupBy::Kind, GroupBy::Created];

    pub fn id(self) -> &'static str {
        match self {
            GroupBy::None => "none",
            GroupBy::Kind => "kind",
            GroupBy::Created => "created",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|g| g.id() == id)
    }
}

/// The section an entry belongs to when grouping. `order` sorts sections;
/// `label` is English and is translated by the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub order: u32,
    pub id: String,
    pub label: &'static str,
}

/// When grouping by creation date, the age buckets, relative to "now".
const DAY: i64 = 86_400;

pub fn section_of(entry: &EntryData, group: GroupBy, now: i64) -> Option<Section> {
    match group {
        GroupBy::None => None,
        GroupBy::Kind => {
            let order = Kind::ALL.iter().position(|k| *k == entry.kind).unwrap_or(0) as u32;
            Some(Section {
                order,
                id: entry.kind.id().into(),
                label: entry.kind.label(),
            })
        }
        GroupBy::Created => {
            let (order, id, label) = if entry.created <= 0 {
                (6, "unknown", "Date Unknown")
            } else {
                let age = now - entry.created;
                match age {
                    a if a < DAY => (0, "today", "Today"),
                    a if a < 2 * DAY => (1, "yesterday", "Yesterday"),
                    a if a < 7 * DAY => (2, "this-week", "Earlier This Week"),
                    a if a < 30 * DAY => (3, "this-month", "Earlier This Month"),
                    a if a < 365 * DAY => (4, "this-year", "Earlier This Year"),
                    _ => (5, "older", "Older"),
                }
            };
            Some(Section {
                order,
                id: id.into(),
                label,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, is_dir: bool, size: u64, kind: Kind, created: i64) -> EntryData {
        EntryData {
            file: gio::File::for_path(format!("/tmp/{name}")),
            name: name.into(),
            display_name: name.into(),
            is_dir,
            is_hidden: false,
            is_symlink: false,
            size,
            modified: created,
            created,
            content_type: String::new(),
            kind,
            icon: None,
            symbolic_icon: None,
            can_read: true,
        }
    }

    #[test]
    fn folders_come_first_even_when_descending() {
        let folder = entry("zz", true, 0, Kind::Folder, 0);
        let file = entry("aa", false, 9, Kind::Other, 0);
        assert_eq!(
            compare(&folder, &file, SortKey::Name, false),
            Ordering::Less
        );
        assert_eq!(compare(&folder, &file, SortKey::Size, true), Ordering::Less);
    }

    #[test]
    fn size_ties_fall_back_to_name() {
        let a = entry("a", false, 5, Kind::Other, 0);
        let b = entry("b", false, 5, Kind::Other, 0);
        assert_eq!(compare(&a, &b, SortKey::Size, false), Ordering::Less);
    }

    #[test]
    fn kind_sections_follow_the_kind_order() {
        let image = entry("x.png", false, 1, Kind::Images, 0);
        let pdf = entry("x.pdf", false, 1, Kind::Pdfs, 0);
        let a = section_of(&image, GroupBy::Kind, 0).unwrap();
        let b = section_of(&pdf, GroupBy::Kind, 0).unwrap();
        assert!(a.order < b.order);
        assert_eq!(a.label, "Images");
    }

    #[test]
    fn created_buckets() {
        let now = 1_000 * DAY;
        let bucket = |age: i64| {
            section_of(
                &entry("f", false, 0, Kind::Other, now - age),
                GroupBy::Created,
                now,
            )
            .unwrap()
            .id
        };
        assert_eq!(bucket(10), "today");
        assert_eq!(bucket(DAY + 1), "yesterday");
        assert_eq!(bucket(3 * DAY), "this-week");
        assert_eq!(bucket(10 * DAY), "this-month");
        assert_eq!(bucket(100 * DAY), "this-year");
        assert_eq!(bucket(400 * DAY), "older");
        let unknown = entry("f", false, 0, Kind::Other, 0);
        assert_eq!(
            section_of(&unknown, GroupBy::Created, now).unwrap().id,
            "unknown"
        );
        assert_eq!(section_of(&unknown, GroupBy::None, now), None);
    }
}
