# The snap (Snap Store and Ubuntu App Center)

The snap is defined in `snap/snapcraft.yaml`: strictly confined, on core24,
with Qt 6 from the kde-neon-6 extension. The name `steward` is registered
to the product owner's Snap Store account.

## Build

```sh
sudo snap install snapcraft --classic
sudo snap install lxd && sudo lxd init --auto && sudo usermod -aG lxd $USER
snapcraft pack --use-lxd
```

### Docker and LXD on the same machine

Docker sets the host's forward policy to drop, so LXD build instances get
no IPv4 internet (IPv6 still works, which hides the problem until a
source is IPv4-only, such as GitHub). Allow LXD's bridge in Docker's
`DOCKER-USER` chain (LXD's documented fix; lost on reboot or a Docker
restart):

```sh
sudo iptables -I DOCKER-USER -i lxdbr0 -j ACCEPT
sudo iptables -I DOCKER-USER -o lxdbr0 -m conntrack --ctstate RELATED,ESTABLISHED -j ACCEPT
```

### Qt from the kde-neon-6 SDK

Three things in the build steps exist because of the SDK snaps, not Steward:

- cxx-qt runs qmake, moc and qmlcachegen with an empty environment, so
  `LD_LIBRARY_PATH` cannot point them at the SDK's own Qt. The build links
  just the libraries the build machine lacks into `/opt/qtsdk-libs` and
  tells the linker about that folder. Adding the SDK folders whole puts
  their copies of system libraries (libudev) ahead of the real ones and
  breaks `snap-confine`.
- The Qt SDK carries dangling `libglib-2.0.so`/`libgio-2.0.so` links. The
  gold linker stops at them and leaves GLib unresolved, so the snap links
  with the standard linker (`-fuse-ld=bfd`).
- snapcraft's output must be piped (`snapcraft pack ... | cat`) when it
  runs without a terminal, or it exits with code 120.

## Test locally

```sh
sudo snap install --dangerous ./steward_*_amd64.snap
snap run steward
```

If the .deb is also installed, quit its background copy first
(`killall steward steward-indexer`); otherwise the snap hands over to it.

## Publish

```sh
snapcraft upload --release=edge steward_*_amd64.snap   # testers: snap install steward --edge
snapcraft release steward <revision> stable           # App Center
```

The two `dbus` slots (Steward's own bus names) always send an upload to
manual review ("human review required due to 'deny-connection'
constraint"). Ask for it on forum.snapcraft.io in the store-requests
category; once approved, an upload made with `--release=edge` goes to
edge on its own.

Inside the snap the host's package tools are out of reach, so the
Installed page and three Free Up Space features say so (P9-T01). Some
interfaces (removable-media, system-observe) may need a store request on
forum.snapcraft.io to connect automatically.

## From CI

When a merged pull request raises the version, the Release workflow
(`.github/workflows/release.yml`) builds the snap and uploads it to edge
after the GitHub Release is out. It needs a store login limited to this
snap, saved as the repository secret `SNAPCRAFT_STORE_CREDENTIALS`:

```sh
snapcraft export-login --snaps=steward --channels=edge \
  --acls package_access,package_push,package_update,package_release - \
  | gh secret set SNAPCRAFT_STORE_CREDENTIALS --repo m-umair-khan-dev/steward
```

Without the secret the snap is still built, and the workflow warns that it
was not uploaded. Moving a revision to stable stays a manual step.
