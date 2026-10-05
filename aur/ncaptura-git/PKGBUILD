pkgname=ncaptura-git
_pkgname=ncaptura
_pkgbasever=0.1.0
pkgver=${_pkgbasever}.r0.g0000000
pkgrel=1
pkgdesc="GTK4 + Libadwaita screenshot, recording, OCR and translation tool"
arch=('x86_64')
options=('!lto')
url="https://github.com/arcat0v0/ncaptura"
license=('MIT' 'Apache-2.0')
depends=('gcc-libs' 'glibc' 'gtk4' 'libadwaita' 'gtk4-layer-shell' 'grim' 'slurp' 'wf-recorder' 'wl-clipboard')
makedepends=('cargo' 'git' 'pkgconf')
optdepends=(
  'libpulse: pactl support for --audio auto device selection'
  'python310: system interpreter used by `ncaptura ocr setup` (any of python310-313 works)'
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

prepare() {
  cd "${srcdir}/${_pkgname}"
  export RUSTUP_TOOLCHAIN=stable
  cargo fetch --locked
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

  install -Dm644 README.md "${pkgdir}/usr/share/doc/${_pkgname}/README.md"
  install -Dm644 docs/README_EN.md "${pkgdir}/usr/share/doc/${_pkgname}/README_EN.md"
  install -Dm644 LICENSE-MIT "${pkgdir}/usr/share/licenses/${_pkgname}/LICENSE-MIT"
  install -Dm644 LICENSE-APACHE "${pkgdir}/usr/share/licenses/${_pkgname}/LICENSE-APACHE"
}
