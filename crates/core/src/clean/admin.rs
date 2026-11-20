//! "Needs administrator" (P5-T07): system clean-ups the app measures but
//! never runs. Each row shows a size (or "size unknown", ADR 0011) and the
//! command to copy. Only read-only queries run here.

use std::path::Path;

use crate::tools::{Ran, Runner, parse_size};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminRow {
    /// "apt-cache", "snap-revisions", "journal", "old-kernels", "docker".
    pub id: &'static str,
    /// None: could not be measured without root.
    pub bytes: Option<u64>,
    /// How many things the command would remove (revisions, kernels …).
    pub count: usize,
    /// Exactly what to paste into a terminal.
    pub command: String,
}

const APT_ARCHIVES: &str = "/var/cache/apt/archives";
const SNAPS: &str = "/var/lib/snapd/snaps";

/// Disabled revisions in `snap list --all` output: (name, revision).
pub fn parse_disabled_snaps(text: &str) -> Vec<(String, String)> {
    let mut lines = text.lines();
    let Some(header) = lines.next() else {
        return Vec::new();
    };
    let notes_at = header.find("Notes");
    lines
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let notes = notes_at
                .and_then(|at| line.get(at..))
                .unwrap_or_else(|| fields.last().copied().unwrap_or(""));
            if !notes.split(',').any(|n| n.trim() == "disabled") {
                return None;
            }
            Some((fields.first()?.to_string(), fields.get(2)?.to_string()))
        })
        .collect()
}

/// The size in "Archived and active journals take up 1.2G in the file system."
pub fn parse_journal_usage(text: &str) -> Option<u64> {
    let rest = text.split("take up ").nth(1)?;
    parse_size(rest.split_whitespace().next()?)
}

/// Installed kernel images that are neither running nor the newest:
/// `dpkg-query -W -f='${Package}\t${Version}\t${Installed-Size}\t${db:Status-Status}\n' 'linux-image-[0-9]*'`.
pub fn parse_old_kernels(text: &str, running: &str) -> Vec<(String, u64)> {
    let mut installed: Vec<(String, String, u64)> = text
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            let [package, version, size, status] = fields.as_slice() else {
                return None;
            };
            (*status == "installed").then(|| {
                (
                    package.to_string(),
                    version.to_string(),
                    size.trim().parse::<u64>().unwrap_or(0) * 1024,
                )
            })
        })
        .collect();
    installed.sort_by(|a, b| natural_version(&a.1).cmp(&natural_version(&b.1)));
    let newest = installed.last().map(|k| k.0.clone());
    installed
        .into_iter()
        .filter(|(package, _, _)| {
            Some(package) != newest.as_ref() && package != &format!("linux-image-{running}")
        })
        .map(|(package, _, size)| (package, size))
        .collect()
}

/// Orders "7.3.0-9" before "7.3.0-10": numbers compare as numbers.
fn natural_version(version: &str) -> Vec<u64> {
    version
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().unwrap_or(0))
        .collect()
}

/// Reclaimable bytes of images in `docker system df --format '{{.Type}}\t{{.Reclaimable}}'`.
pub fn parse_docker_reclaimable(text: &str) -> Option<u64> {
    text.lines().find_map(|line| {
        let (kind, value) = line.split_once('\t')?;
        (kind.trim() == "Images").then(|| parse_size(value.split_whitespace().next()?))?
    })
}

fn apt_cache(archives: &Path) -> AdminRow {
    let files: Option<Vec<u64>> = std::fs::read_dir(archives).ok().map(|entries| {
        entries
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".deb"))
            .filter_map(|e| e.metadata().ok().map(|m| m.len()))
            .collect()
    });
    AdminRow {
        id: "apt-cache",
        count: files.as_ref().map_or(0, Vec::len),
        bytes: files.map(|f| f.iter().sum()),
        command: "sudo apt clean".into(),
    }
}

/// The rows for this system. Tools that are not installed have no row.
pub fn rows(runner: &dyn Runner) -> Vec<AdminRow> {
    let mut out = vec![apt_cache(Path::new(APT_ARCHIVES))];

    match runner.run("snap", &["list", "--all"]) {
        Ran::Missing => {}
        Ran::Failed(_) => out.push(AdminRow {
            id: "snap-revisions",
            bytes: None,
            count: 0,
            command: "snap list --all | awk '/disabled/{print $1, $3}' | while read name rev; do sudo snap remove \"$name\" --revision=\"$rev\"; done".into(),
        }),
        Ran::Output(text) => {
            let disabled = parse_disabled_snaps(&text);
            if !disabled.is_empty() {
                let sizes: Vec<Option<u64>> = disabled
                    .iter()
                    .map(|(name, rev)| {
                        Path::new(SNAPS)
                            .join(format!("{name}_{rev}.snap"))
                            .metadata()
                            .ok()
                            .map(|m| m.len())
                    })
                    .collect();
                out.push(AdminRow {
                    id: "snap-revisions",
                    bytes: sizes.iter().copied().sum(),
                    count: disabled.len(),
                    command: disabled
                        .iter()
                        .map(|(name, rev)| format!("sudo snap remove {name} --revision={rev}"))
                        .collect::<Vec<_>>()
                        .join(" && "),
                });
            }
        }
    }

    match runner.run("journalctl", &["--disk-usage"]) {
        Ran::Missing => {}
        ran => out.push(AdminRow {
            id: "journal",
            bytes: ran.output().and_then(|t| parse_journal_usage(&t)),
            count: 0,
            command: "sudo journalctl --vacuum-size=200M".into(),
        }),
    }

    let running = rustix::system::uname().release().to_string_lossy().into_owned();
    if let Some(text) = runner
        .run(
            "dpkg-query",
            &[
                "-W",
                "-f=${Package}\t${Version}\t${Installed-Size}\t${db:Status-Status}\n",
                "linux-image-[0-9]*",
            ],
        )
        .output()
    {
        let old = parse_old_kernels(&text, &running);
        if !old.is_empty() {
            out.push(AdminRow {
                id: "old-kernels",
                bytes: Some(old.iter().map(|k| k.1).sum()),
                count: old.len(),
                command: "sudo apt autoremove --purge".into(),
            });
        }
    }

    match runner.run(
        "docker",
        &["system", "df", "--format", "{{.Type}}\t{{.Reclaimable}}"],
    ) {
        Ran::Missing => {}
        ran => out.push(AdminRow {
            id: "docker",
            bytes: ran.output().and_then(|t| parse_docker_reclaimable(&t)),
            count: 0,
            command: "docker image prune --all".into(),
        }),
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_snap_revisions() {
        let text = "Name      Version   Rev    Tracking       Publisher   Notes\n\
                    core22    20250110  1748   latest/stable  canonical✓  base,disabled\n\
                    core22    20250315  1802   latest/stable  canonical✓  base\n\
                    painter   2.1       13     latest/stable  example     disabled\n\
                    painter   2.2       15     latest/stable  example     -\n";
        assert_eq!(
            parse_disabled_snaps(text),
            vec![("core22".into(), "1748".into()), ("painter".into(), "13".into())]
        );
    }

    #[test]
    fn journal_usage() {
        assert_eq!(
            parse_journal_usage("Archived and active journals take up 1.2G in the file system."),
            Some(1_288_490_189)
        );
        assert_eq!(parse_journal_usage("No journal files were found."), None);
    }

    #[test]
    fn old_kernels_keep_the_running_and_the_newest() {
        let text = "linux-image-7.0.0-38-generic\t7.0.0-38.38\t15000\tinstalled\n\
                    linux-image-7.3.0-9-generic\t7.3.0-9.9\t16000\tinstalled\n\
                    linux-image-7.3.0-10-generic\t7.3.0-10.10\t16100\tinstalled\n\
                    linux-image-6.8.0-1-generic\t6.8.0-1.1\t14000\tconfig-files\n";
        let old = parse_old_kernels(text, "7.3.0-9-generic");
        assert_eq!(old, vec![("linux-image-7.0.0-38-generic".into(), 15000 * 1024)]);
    }

    #[test]
    fn docker_images() {
        let text = "Images\t2.5GB (61%)\nContainers\t0B (0%)\n";
        assert_eq!(parse_docker_reclaimable(text), Some(2_500_000_000));
    }
}
