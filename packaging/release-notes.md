## Install

Download the file for your Ubuntu version (64-bit Intel/AMD PCs):

| Ubuntu | File |
|---|---|
| 24.04 LTS | `steward_@VERSION@-1.ubuntu24.04_amd64.deb` |
| 26.04 LTS | `steward_@VERSION@-1.ubuntu26.04_amd64.deb` |
| 26.10 | `steward_@VERSION@-1.ubuntu26.10_amd64.deb` |

Then install it from a terminal in the download folder:

```
sudo apt install ./steward_@VERSION@-1.ubuntu24.04_amd64.deb
```

Use `apt`, not `dpkg -i`, so the libraries Steward needs are installed too.
Not sure which Ubuntu you have? Run `lsb_release -rs`.

Check a download with `sha256sum -c SHA256SUMS --ignore-missing`.
