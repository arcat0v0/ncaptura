pkgname=ncaptura-git
_pkgname=ncaptura
_pkgbasever=0.1.0
pkgver=${_pkgbasever}.r0.g0000000
pkgrel=1
pkgdesc="GTK4 + Libadwaita screenshot and recording tool"
arch=('x86_64')
url="https://github.com/arcat0v0/ncaptura"
license=('unknown')
depends=('gcc-libs' 'glibc' 'gtk4' 'libadwaita' 'grim' 'slurp' 'wf-recorder')
makedepends=('cargo' 'git' 'pkgconf')
optdepends=(
  'libpulse: pactl support for --audio auto device selection'
  'niri: focused output detection in fullscreen mode'
)
provides=("${_pkgname}")
conflicts=("${_pkgname}")
source=("${_pkgname}::git+${url}.git")
sha256sums=('SKIP')

pkgver() {
  cd "${srcdir}/${_pkgname}"
  printf "%s.r%s.g%s" \
    "${_pkgbasever}" \
    "$(git rev-list --count HEAD)" \
    "$(git rev-parse --short HEAD)"
}

build() {
  cd "${srcdir}/${_pkgname}"
  export RUSTUP_TOOLCHAIN=stable
  export CARGO_TARGET_DIR="target"
  cargo build --frozen --release
}

check() {
  cd "${srcdir}/${_pkgname}"
  export RUSTUP_TOOLCHAIN=stable
  export CARGO_TARGET_DIR="target"
  cargo test --frozen
}

package() {
  cd "${srcdir}/${_pkgname}"
  install -Dm755 "target/release/${_pkgname}" "${pkgdir}/usr/bin/${_pkgname}"

  if [[ -f README.md ]]; then
    install -Dm644 README.md "${pkgdir}/usr/share/doc/${_pkgname}/README.md"
  fi
}
