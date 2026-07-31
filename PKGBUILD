# Maintainer: scubba
#
# Tauri v2 has no pacman bundler, so this PKGBUILD — not `tauri build` — is how
# Komble is packaged for Arch. It also installs the polkit policy and the
# privileged helper, which on the Debian build were carried inside the .deb.

pkgname=komble-arch
pkgver=0.1.0
pkgrel=2
pkgdesc="App store for Arch — pacman, the AUR and AppImages"
arch=('x86_64' 'aarch64')
url="https://github.com/prj786/komble-arch"
license=('MIT')

depends=(
  'webkit2gtk-4.1'
  'gtk3'
  'libayatana-appindicator'   # tray icon
  'polkit'                    # pkexec + the helper policy
  'pacman'
)
optdepends=(
  'pacman-contrib: safe update checking via checkupdates (strongly recommended)'
  'expac: much faster package index — without it descriptions are unavailable'
  'base-devel: required to build AUR packages'
  'git: required to build AUR packages'
  'fuse2: needed to LAUNCH AppImages (integration works without it)'
)
makedepends=('rust' 'cargo' 'nodejs' 'npm')

# Arch turns LTO on by default in makepkg.conf, which injects -flto into CFLAGS
# and LDFLAGS. The `ring` crate (via reqwest/rustls) builds hand-written x86-64
# assembly, and that does not survive -flto: its objects become bitcode and the
# link then fails with a wall of
#   ld.lld: error: undefined symbol: ring_core_0_17_14__p256_mul_mont
# which reads like a missing dependency rather than a flags problem.
#
# Rust-level LTO is still on via [profile.release] in Cargo.toml, so this only
# disables makepkg's *C* LTO — no loss in the resulting binary.
options=(!lto)

source=("$pkgname-$pkgver.tar.gz::$url/archive/refs/tags/v$pkgver.tar.gz")
sha256sums=('8bbf8d551060cab8958e611fa0effd87865f851be15213fd52e7d0df15800ac1')

build() {
  cd "$srcdir/$pkgname-$pkgver"
  npm ci
  npm run build
  cd src-tauri
  cargo build --release --locked
}

package() {
  cd "$srcdir/$pkgname-$pkgver"

  install -Dm755 src-tauri/target/release/komble-arch "$pkgdir/usr/bin/komble"

  # The privileged helper. 0755 and root-owned: polkit authorises this exact
  # path, so it must not be writable by anyone who could then run it as root.
  install -Dm755 packaging/komble-helper "$pkgdir/usr/lib/komble/komble-helper"
  install -Dm644 packaging/io.github.komble.arch.policy \
    "$pkgdir/usr/share/polkit-1/actions/io.github.komble.arch.policy"

  install -Dm644 packaging/komble.desktop "$pkgdir/usr/share/applications/komble.desktop"
  install -Dm644 src-tauri/icons/128x128.png \
    "$pkgdir/usr/share/icons/hicolor/128x128/apps/komble.png"
  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
