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

## Publish

```sh
snapcraft upload --release=edge steward_*_amd64.snap   # testers: snap install steward --edge
snapcraft release steward <revision> stable           # App Center
```

Inside the snap the host's package tools are out of reach, so the
Installed page and three Free Up Space features say so (P9-T01). Some
interfaces (removable-media, system-observe) may need a store request on
forum.snapcraft.io to connect automatically.
