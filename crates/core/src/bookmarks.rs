//! Bookmarks (P7-T08; DESIGN.md §4.2, §7.1): links with pictures, in
//! collections, with tags and favourites. Kept in `app.db`.
//!
//! Fetching a link's title and picture contacts that site, and nothing
//! else does: it is the app's only outbound request (DESIGN.md §8.5), and
//! Preferences can turn it off. The fetch is bounded in time and size.

use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{OptionalExtension, params};

use crate::store::Store;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Bookmark {
    pub id: i64,
    pub url: String,
    pub title: Option<String>,
    pub site: Option<String>,
    /// "article", "video", "image", "file" or "other".
    pub kind: String,
    pub image: Option<PathBuf>,
    pub favourite: bool,
    pub collection: Option<i64>,
    pub tags: Vec<String>,
    pub created: i64,
    /// True once a fetch was tried (whether or not it found anything).
    pub fetched: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collection {
    pub id: i64,
    pub name: String,
}

/// A link as typed or pasted, cleaned up: "example.com/a" becomes
/// "https://example.com/a". Anything that is not a web link is refused.
pub fn normalise(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() || text.contains(char::is_whitespace) {
        return None;
    }
    let url = if text.starts_with("http://") || text.starts_with("https://") {
        text.to_string()
    } else if !text.contains("://")
        && host_of(&format!("https://{text}")).is_some_and(|h| h.contains('.'))
    {
        format!("https://{text}")
    } else {
        return None;
    };
    host_of(&url).filter(|h| !h.is_empty())?;
    Some(url)
}

/// "https://www.example.com:8080/a" → "www.example.com:8080".
pub fn host_of(url: &str) -> Option<&str> {
    let rest = url.split_once("://")?.1;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let host = &rest[..end];
    let host = host.rsplit_once('@').map(|(_, h)| h).unwrap_or(host);
    Some(host)
}

/// The site as shown on a card: the host without "www." or a port.
pub fn site_of(url: &str) -> String {
    let host = host_of(url).unwrap_or("");
    let host = host.strip_prefix("www.").unwrap_or(host);
    host.split(':').next().unwrap_or(host).to_string()
}

// --- the database -------------------------------------------------------------------------------

fn row(r: &rusqlite::Row) -> rusqlite::Result<Bookmark> {
    Ok(Bookmark {
        id: r.get(0)?,
        url: r.get(1)?,
        title: r.get(2)?,
        site: r.get(3)?,
        kind: r.get(4)?,
        image: r.get::<_, Option<String>>(5)?.map(PathBuf::from),
        favourite: r.get(6)?,
        collection: r.get(7)?,
        created: r.get(8)?,
        fetched: r.get(9)?,
        tags: Vec::new(),
    })
}

const COLUMNS: &str =
    "id, url, title, site, kind, image_path, favourite, collection_id, created_at, fetched";

impl Store {
    /// Adds a link; returns its id and whether it was new. Adding a link
    /// that is already saved keeps the saved one.
    pub fn bookmark_add(&self, url: &str, now: i64) -> rusqlite::Result<(i64, bool)> {
        if let Some(id) = self
            .conn()
            .query_row("SELECT id FROM bookmark WHERE url = ?1", [url], |r| {
                r.get(0)
            })
            .optional()?
        {
            return Ok((id, false));
        }
        self.conn().execute(
            "INSERT INTO bookmark (url, site, created_at) VALUES (?1, ?2, ?3)",
            params![url, site_of(url), now],
        )?;
        Ok((self.conn().last_insert_rowid(), true))
    }

    pub fn bookmarks(&self) -> rusqlite::Result<Vec<Bookmark>> {
        let mut statement = self.conn().prepare(&format!(
            "SELECT {COLUMNS} FROM bookmark ORDER BY created_at DESC, id DESC"
        ))?;
        let mut all: Vec<Bookmark> = statement.query_map([], row)?.collect::<Result<_, _>>()?;
        let mut tags = self
            .conn()
            .prepare("SELECT bookmark_id, tag FROM bookmark_tag ORDER BY tag")?;
        let pairs: Vec<(i64, String)> = tags
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        for (id, tag) in pairs {
            if let Some(b) = all.iter_mut().find(|b| b.id == id) {
                b.tags.push(tag);
            }
        }
        Ok(all)
    }

    pub fn bookmark(&self, id: i64) -> rusqlite::Result<Option<Bookmark>> {
        Ok(self.bookmarks()?.into_iter().find(|b| b.id == id))
    }

    /// What a fetch found. Fields it did not find stay as they were.
    pub fn bookmark_set_meta(&self, id: i64, meta: &Meta) -> rusqlite::Result<()> {
        self.conn().execute(
            "UPDATE bookmark SET title = COALESCE(?2, title), site = COALESCE(?3, site),
                kind = ?4, image_path = COALESCE(?5, image_path), fetched = 1 WHERE id = ?1",
            params![
                id,
                meta.title,
                meta.site,
                meta.kind,
                meta.image_file.as_ref().map(|p| p.display().to_string()),
            ],
        )?;
        Ok(())
    }

    pub fn bookmark_set_title(&self, id: i64, title: &str) -> rusqlite::Result<()> {
        let title = title.trim();
        self.conn().execute(
            "UPDATE bookmark SET title = ?2 WHERE id = ?1",
            params![id, (!title.is_empty()).then_some(title)],
        )?;
        Ok(())
    }

    pub fn bookmark_set_favourite(&self, id: i64, favourite: bool) -> rusqlite::Result<()> {
        self.conn().execute(
            "UPDATE bookmark SET favourite = ?2 WHERE id = ?1",
            params![id, favourite],
        )?;
        Ok(())
    }

    pub fn bookmark_set_collection(
        &self,
        id: i64,
        collection: Option<i64>,
    ) -> rusqlite::Result<()> {
        self.conn().execute(
            "UPDATE bookmark SET collection_id = ?2 WHERE id = ?1",
            params![id, collection],
        )?;
        Ok(())
    }

    /// Replaces a bookmark's tags. Tags are trimmed, lower-case and unique.
    pub fn bookmark_set_tags(&mut self, id: i64, tags: &[String]) -> rusqlite::Result<()> {
        let tx = self.conn_mut().transaction()?;
        tx.execute("DELETE FROM bookmark_tag WHERE bookmark_id = ?1", [id])?;
        for tag in tags {
            let tag = tag.trim().trim_start_matches('#').to_lowercase();
            if !tag.is_empty() {
                tx.execute(
                    "INSERT OR IGNORE INTO bookmark_tag (bookmark_id, tag) VALUES (?1, ?2)",
                    params![id, tag],
                )?;
            }
        }
        tx.commit()
    }

    /// Removes a bookmark; returns its picture so the caller can delete it.
    pub fn bookmark_delete(&self, id: i64) -> rusqlite::Result<Option<PathBuf>> {
        let image: Option<String> = self
            .conn()
            .query_row("SELECT image_path FROM bookmark WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .optional()?
            .flatten();
        self.conn()
            .execute("DELETE FROM bookmark WHERE id = ?1", [id])?;
        Ok(image.map(PathBuf::from))
    }

    pub fn collections(&self) -> rusqlite::Result<Vec<Collection>> {
        let mut statement = self
            .conn()
            .prepare("SELECT id, name FROM collection ORDER BY position, id")?;
        statement
            .query_map([], |r| {
                Ok(Collection {
                    id: r.get(0)?,
                    name: r.get(1)?,
                })
            })?
            .collect()
    }

    /// Adds a collection, or returns the one with that name.
    pub fn collection_add(&self, name: &str) -> rusqlite::Result<i64> {
        let name = name.trim();
        if let Some(id) = self
            .conn()
            .query_row("SELECT id FROM collection WHERE name = ?1", [name], |r| {
                r.get(0)
            })
            .optional()?
        {
            return Ok(id);
        }
        self.conn().execute(
            "INSERT INTO collection (name, position)
             VALUES (?1, (SELECT COALESCE(MAX(position), 0) + 1 FROM collection))",
            [name],
        )?;
        Ok(self.conn().last_insert_rowid())
    }

    pub fn collection_rename(&self, id: i64, name: &str) -> rusqlite::Result<()> {
        self.conn().execute(
            "UPDATE collection SET name = ?2 WHERE id = ?1",
            params![id, name.trim()],
        )?;
        Ok(())
    }

    /// Deletes a collection; its bookmarks stay, outside any collection.
    pub fn collection_delete(&self, id: i64) -> rusqlite::Result<()> {
        self.conn()
            .execute("DELETE FROM collection WHERE id = ?1", [id])?;
        Ok(())
    }
}

// --- what a page says about itself ----------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Meta {
    pub title: Option<String>,
    pub site: Option<String>,
    pub kind: String,
    /// The picture's address, as found in the page.
    pub image_url: Option<String>,
    /// Where the picture was saved.
    pub image_file: Option<PathBuf>,
}

fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let end = after.find(';').filter(|e| *e <= 8);
        let decoded = end.and_then(|e| {
            let name = &after[..e];
            let c = match name {
                "amp" => '&',
                "lt" => '<',
                "gt" => '>',
                "quot" => '"',
                "apos" => '\'',
                "nbsp" => ' ',
                _ => {
                    let code =
                        if let Some(hex) = name.strip_prefix("#x").or(name.strip_prefix("#X")) {
                            u32::from_str_radix(hex, 16).ok()?
                        } else {
                            name.strip_prefix('#')?.parse().ok()?
                        };
                    char::from_u32(code)?
                }
            };
            Some((c, e))
        });
        match decoded {
            Some((c, e)) => {
                out.push(c);
                rest = &after[e + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn clean_text(text: &str) -> Option<String> {
    let text = decode_entities(text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    (!text.is_empty()).then(|| text.chars().take(300).collect())
}

/// The attributes of one tag (`<meta property="og:title" content="…">`).
fn attributes(tag: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let bytes = tag.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && !bytes[i].is_ascii_alphabetic() {
            i += 1;
        }
        let start = i;
        while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || b"-_:".contains(&bytes[i])) {
            i += 1;
        }
        let name = tag[start..i].to_ascii_lowercase();
        while i < bytes.len() && bytes[i] == b' ' {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'=' {
            i += 1;
            while i < bytes.len() && bytes[i] == b' ' {
                i += 1;
            }
            let value = if i < bytes.len() && (bytes[i] == b'"' || bytes[i] == b'\'') {
                let quote = bytes[i];
                let s = i + 1;
                let e = tag[s..]
                    .find(quote as char)
                    .map(|e| s + e)
                    .unwrap_or(tag.len());
                i = e + 1;
                &tag[s..e.min(tag.len())]
            } else {
                let s = i;
                while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                &tag[s..i]
            };
            out.push((name, value.to_string()));
        } else if !name.is_empty() {
            out.push((name, String::new()));
        }
    }
    out
}

/// `href` resolved against the page's address.
pub fn resolve(base: &str, href: &str) -> Option<String> {
    let href = href.trim();
    if href.starts_with("http://") || href.starts_with("https://") {
        return Some(href.to_string());
    }
    if href.contains("://") || href.starts_with("data:") || href.starts_with("javascript:") {
        return None;
    }
    let (scheme, rest) = base.split_once("://")?;
    let host_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let origin = format!("{scheme}://{}", &rest[..host_end]);
    if let Some(protocol_relative) = href.strip_prefix("//") {
        return Some(format!("{scheme}://{protocol_relative}"));
    }
    if href.starts_with('/') {
        return Some(format!("{origin}{href}"));
    }
    let path = &rest[host_end..];
    let path = path.split(['?', '#']).next().unwrap_or("");
    let folder = path.rfind('/').map(|i| &path[..=i]).unwrap_or("/");
    Some(format!("{origin}{folder}{href}"))
}

/// The title, site name, kind and picture a page declares (Open Graph,
/// Twitter cards, `<title>`).
pub fn parse_html(html: &str, url: &str) -> Meta {
    let lower = html.to_ascii_lowercase();
    let mut meta = Meta {
        kind: "other".into(),
        ..Meta::default()
    };
    let mut og_type = None;
    let mut og_title = None;
    let mut images: Vec<(u8, String)> = Vec::new();
    let mut from = 0;
    while let Some(found) = lower[from..].find("<meta") {
        let start = from + found;
        let end = lower[start..]
            .find('>')
            .map(|e| start + e)
            .unwrap_or(lower.len());
        let attrs = attributes(&html[start + 5..end]);
        from = end;
        let get = |key: &str| {
            attrs
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str())
        };
        let key = get("property")
            .or(get("name"))
            .unwrap_or("")
            .to_ascii_lowercase();
        let Some(content) = get("content") else {
            continue;
        };
        match key.as_str() {
            "og:title" | "twitter:title" if og_title.is_none() => og_title = clean_text(content),
            "og:site_name" => meta.site = clean_text(content),
            "og:type" => og_type = Some(content.to_ascii_lowercase()),
            "og:image" | "og:image:url" | "og:image:secure_url" => {
                images.push((0, content.to_string()))
            }
            "twitter:image" | "twitter:image:src" => images.push((1, content.to_string())),
            _ => {}
        }
    }
    if let Some(start) = lower.find("<title")
        && let Some(open_end) = lower[start..].find('>')
    {
        let s = start + open_end + 1;
        if let Some(close) = lower[s..].find("</title") {
            meta.title = clean_text(&html[s..s + close]);
        }
    }
    if og_title.is_some() {
        meta.title = og_title;
    }
    images.sort_by_key(|(rank, _)| *rank);
    meta.image_url = images
        .into_iter()
        .find_map(|(_, href)| resolve(url, &decode_entities(&href)));
    let host = site_of(url);
    meta.kind = match og_type.as_deref() {
        Some(t) if t.starts_with("video") => "video",
        Some("article") => "article",
        _ if ["youtube.com", "youtu.be", "vimeo.com"]
            .iter()
            .any(|h| host.ends_with(h)) =>
        {
            "video"
        }
        _ => "other",
    }
    .into();
    meta
}

// --- fetching -------------------------------------------------------------------------------------

const PAGE_LIMIT: u64 = 2 * 1024 * 1024;
const IMAGE_LIMIT: u64 = 5 * 1024 * 1024;

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .max_redirects(5)
        .user_agent(concat!("Cairn/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn content_type(response: &ureq::http::Response<ureq::Body>) -> String {
    response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
}

fn picture_extension(mime: &str) -> Option<&'static str> {
    // Raster pictures only: an SVG could refer to other addresses.
    match mime {
        "image/png" => Some("png"),
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        _ => None,
    }
}

/// The folder bookmark pictures are kept in: `$XDG_DATA_HOME/cairn/bookmarks`.
pub fn pictures_dir() -> PathBuf {
    glib::user_data_dir().join("cairn").join("bookmarks")
}

fn save_picture(agent: &ureq::Agent, url: &str, folder: &Path) -> Option<PathBuf> {
    let mut response = agent.get(url).call().ok()?;
    let ext = picture_extension(&content_type(&response))?;
    let bytes = response
        .body_mut()
        .with_config()
        .limit(IMAGE_LIMIT)
        .read_to_vec()
        .ok()?;
    let name = format!("{}.{ext}", blake3::hash(url.as_bytes()).to_hex());
    std::fs::create_dir_all(folder).ok()?; // E2-13 allow: the app's own picture cache
    let path = folder.join(name);
    std::fs::write(&path, bytes).ok()?; // E2-13 allow: the app's own picture cache
    Some(path)
}

/// Fetches a link's page and its picture (saved into `folder`). Blocking:
/// call it off the main thread.
pub fn fetch(url: &str, folder: &Path) -> Meta {
    let agent = agent();
    let Ok(mut response) = agent.get(url).call() else {
        return Meta {
            kind: "other".into(),
            ..Meta::default()
        };
    };
    let mime = content_type(&response);
    if mime.starts_with("image/") {
        drop(response);
        let image_file = save_picture(&agent, url, folder);
        return Meta {
            kind: "image".into(),
            image_file,
            ..Meta::default()
        };
    }
    if mime != "text/html" && mime != "application/xhtml+xml" {
        let kind = if mime.starts_with("video/") {
            "video"
        } else {
            "file"
        };
        return Meta {
            kind: kind.into(),
            ..Meta::default()
        };
    }
    let Ok(html) = response
        .body_mut()
        .with_config()
        .limit(PAGE_LIMIT)
        .read_to_string()
    else {
        return Meta {
            kind: "other".into(),
            ..Meta::default()
        };
    };
    let mut meta = parse_html(&html, url);
    if let Some(image) = &meta.image_url {
        meta.image_file = save_picture(&agent, image, folder);
    }
    meta
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_are_normalised() {
        assert_eq!(
            normalise(" example.com/a "),
            Some("https://example.com/a".into())
        );
        assert_eq!(
            normalise("http://127.0.0.1:8000/x"),
            Some("http://127.0.0.1:8000/x".into())
        );
        assert_eq!(normalise("ftp://example.com"), None);
        assert_eq!(normalise("hello world"), None);
        assert_eq!(normalise("notes"), None);
        assert_eq!(site_of("https://www.example.com:8080/a?b"), "example.com");
    }

    #[test]
    fn open_graph_wins_over_the_title() {
        let html = r#"<html><head><title> Plain &amp; simple </title>
            <meta property="og:title" content="Swift &quot;concurrency&quot;">
            <meta content='/img/card.png' property='og:image'>
            <meta name="twitter:image" content="https://cdn.example.com/t.png">
            <meta property="og:site_name" content="Dev Notes">
            <meta property="og:type" content="article"></head></html>"#;
        let meta = parse_html(html, "https://example.com/posts/1?x=2");
        assert_eq!(meta.title.as_deref(), Some("Swift \"concurrency\""));
        assert_eq!(
            meta.image_url.as_deref(),
            Some("https://example.com/img/card.png")
        );
        assert_eq!(meta.site.as_deref(), Some("Dev Notes"));
        assert_eq!(meta.kind, "article");
        let plain = parse_html(
            "<TITLE>Only &#8211; this</TITLE>",
            "https://youtube.com/watch?v=1",
        );
        assert_eq!(plain.title.as_deref(), Some("Only – this"));
        assert_eq!(plain.kind, "video");
        assert_eq!(plain.image_url, None);
    }

    #[test]
    fn relative_addresses() {
        let base = "http://127.0.0.1:9/a/b/page.html?q";
        assert_eq!(
            resolve(base, "pic.png").unwrap(),
            "http://127.0.0.1:9/a/b/pic.png"
        );
        assert_eq!(
            resolve(base, "/pic.png").unwrap(),
            "http://127.0.0.1:9/pic.png"
        );
        assert_eq!(
            resolve(base, "//cdn.x/p.png").unwrap(),
            "http://cdn.x/p.png"
        );
        assert_eq!(resolve(base, "data:image/png;base64,AA"), None);
    }

    #[test]
    fn the_database() {
        let mut store = Store::open_in_memory().unwrap();
        let (id, new) = store.bookmark_add("https://example.com/a", 10).unwrap();
        assert!(new);
        assert_eq!(
            store.bookmark_add("https://example.com/a", 11).unwrap(),
            (id, false)
        );
        let (other, _) = store.bookmark_add("https://www.other.org/", 20).unwrap();
        store
            .bookmark_set_tags(id, &["Design".into(), " #design ".into(), "rust".into()])
            .unwrap();
        store.bookmark_set_favourite(id, true).unwrap();
        let reading = store.collection_add("Reading").unwrap();
        assert_eq!(store.collection_add("Reading").unwrap(), reading);
        store.bookmark_set_collection(id, Some(reading)).unwrap();
        store
            .bookmark_set_meta(
                id,
                &Meta {
                    title: Some("A".into()),
                    kind: "article".into(),
                    ..Meta::default()
                },
            )
            .unwrap();
        let all = store.bookmarks().unwrap();
        assert_eq!(
            all.iter().map(|b| b.id).collect::<Vec<_>>(),
            [other, id],
            "newest first"
        );
        let a = &all[1];
        assert_eq!(a.tags, ["design", "rust"]);
        assert!(a.favourite && a.fetched);
        assert_eq!(
            (a.title.as_deref(), a.site.as_deref(), a.kind.as_str()),
            (Some("A"), Some("example.com"), "article")
        );
        store.collection_delete(reading).unwrap();
        assert_eq!(
            store.bookmark(id).unwrap().unwrap().collection,
            None,
            "kept, outside any collection"
        );
        store.bookmark_delete(id).unwrap();
        assert_eq!(store.bookmarks().unwrap().len(), 1);
    }
}
