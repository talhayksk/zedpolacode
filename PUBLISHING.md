# Yayınlama

Bu belge, Zed Polacode'u Zed eklenti mağazasına gönderme adımlarını anlatır.

## 0. Ön koşullar

- Depo public olmalı: <https://github.com/talhayksk/zedpolacode>
- `extension.toml` içindeki `id` tekil olmalı (`zedpolacode`).
- `version` üç yerde aynı olmalı: `extension.toml`, `Cargo.toml`, `CHANGELOG.md`.

## 1. Sürüm çıkar

```bash
# sürüm numaralarını güncelle, sonra:
git commit -am "Bump version to 0.1.0"
git tag -a v0.1.0 -m "Zed Polacode 0.1.0"
git push origin main --tags
```

CI yeşil olmalı ve `zedpolacode.wasm` artifact'ı üretilmeli.

## 2. Zed mağaza kaydı (PR)

Zed mağazası, [zed-industries/extensions](https://github.com/zed-industries/extensions)
deposundaki bir **alt modül** listesiyle çalışır. Süreç:

1. `zed-industries/extensions` deposunu fork'la.
2. Fork'u klonla ve alt modülü ekle:

   ```bash
   git clone https://github.com/<kullanici>/extensions.git
   cd extensions
   git submodule add https://github.com/talhayksk/zedpolacode.git extensions/zedpolacode
   ```

3. `extensions.toml` dosyasına kaydı ekle:

   ```toml
   [zedpolacode]
   submodule = "extensions/zedpolacode"
   version = "0.1.0"
   ```

4. Değişiklikleri commit'le ve `zed-industries/extensions` deposuna PR aç.
   PR şablonundaki kontroller (manifest doğrulama, wasm derleme, isim
   çakışması) otomatik çalışır.

> Aynı `id` ile başka bir eklenti varsa PR reddedilir. `zedpolacode` şu an
> boş görünüyor, ama göndermeden önce
> <https://github.com/zed-industries/extensions/blob/main/extensions.toml>
> içinde tekrar kontrol et.

## 3. Yeni sürüm yayınlama

Zed, eklentileri alt modülün **tag**'inden okur. Yeni sürüm için:

1. Sürümü artır (`extension.toml` + `Cargo.toml` + `CHANGELOG.md`).
2. `git tag -a vX.Y.Z -m "..."` ve `git push --tags`.
3. `zed-industries/extensions` fork'unda alt modülü güncelle:

   ```bash
   cd extensions/zedpolacode
   git fetch --tags
   git checkout vX.Y.Z
   cd ../..
   git add extensions/zedpolacode
   git commit -m "Update zedpolacode to vX.Y.Z"
   ```

4. `extensions.toml` içindeki `version` alanını güncelle ve PR aç.

## 4. Kontrol listesi (göndermeden önce)

- [ ] `cargo fmt --check` temiz
- [ ] `cargo test` yeşil
- [ ] `cargo build --release --target wasm32-wasip1` başarılı
- [ ] `extension.toml` içinde `id`, `name`, `version`, `schema_version`,
      `authors`, `description`, `repository` dolu
- [ ] README'de kurulum ve kullanım anlaşılır
- [ ] `LICENSE` var
- [ ] PNG çıktısı gerçekten üretiliyor (Chrome/Edge kurulu bir makinede elle dene)
