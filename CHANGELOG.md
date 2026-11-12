# Changelog

Bu proje [Semantic Versioning](https://semver.org/lang/tr/) kullanır.

## 0.1.0 — 2026-11-12

İlk sürüm.

- `/polacode` slash komutu: dosya veya satır aralığından kod kartı üretir.
- `--lines`, `--theme`, `--bg`, `--title`, `--lang`, `--font-size`, `--padding`,
  `--radius`, `--no-line-numbers`, `--no-window`, `--no-shadow`, `--out`,
  `--format` seçenekleri ve slash komutu tamamlamaları.
- 8 tema: dracula, one-dark, nord, tokyo-night, github-dark, catppuccin-mocha,
  github-light, solarized-light.
- 23 dil için bağımlılıksız renklendirme (HTML/CSS/JSON/YAML/TOML için özel
  kurallar).
- Çıktı: `@2x` PNG (Chrome/Edge/Chromium/Brave ile) + kendi kendine yeten HTML.
  Şeffaf arka plan, gradyan arka planlar ve hex renk desteği.
- PNG üretilemediğinde komut hata vermez; HTML yazılıp nedeni açıklanır.
- Zed'in WASI kum havuzu nedeniyle kart, eklenti dizininde üretilip host
  tarafında Node ile projeye kopyalanır.
