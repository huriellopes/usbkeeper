#!/usr/bin/env bash
# Gera/atualiza o repositório APT assinado do usbkeeper.
#   scripts/apt-repo.sh <dir-do-site> <dir-com-.deb> <id-da-chave-gpg>
# Copia os .deb novos para <site>/apt/pool, recria os índices e assina
# (InRelease + Release.gpg). Requer apt-ftparchive (apt-utils) e gpg.
set -euo pipefail
SITE=$1 DEBS=$2 KEY=$3
APT="$SITE/apt"
POOL="$APT/pool/main/u/usbkeeper"
DIST="$APT/dists/stable"
mkdir -p "$POOL" "$DIST/main/binary-amd64"

shopt -s nullglob
for deb in "$DEBS"/*_amd64.deb; do
  [[ -e "$POOL/$(basename "$deb")" ]] || cp -v "$deb" "$POOL/"
done

cd "$APT"
apt-ftparchive packages pool > dists/stable/main/binary-amd64/Packages
gzip -9kf dists/stable/main/binary-amd64/Packages
apt-ftparchive \
  -o APT::FTPArchive::Release::Origin=usbkeeper \
  -o APT::FTPArchive::Release::Label=usbkeeper \
  -o APT::FTPArchive::Release::Suite=stable \
  -o APT::FTPArchive::Release::Codename=stable \
  -o APT::FTPArchive::Release::Architectures=amd64 \
  -o APT::FTPArchive::Release::Components=main \
  -o APT::FTPArchive::Release::Description="usbkeeper: pendrives e HDs externos" \
  release dists/stable > dists/stable/Release.tmp
mv dists/stable/Release.tmp dists/stable/Release
gpg --batch --yes --pinentry-mode loopback -u "$KEY" --clearsign -o dists/stable/InRelease dists/stable/Release
gpg --batch --yes --pinentry-mode loopback -u "$KEY" -abs -o dists/stable/Release.gpg dists/stable/Release
gpg --export "$KEY" > usbkeeper.gpg
echo "repositório pronto em $APT ($(ls pool/main/u/usbkeeper | wc -l) pacote(s))"
