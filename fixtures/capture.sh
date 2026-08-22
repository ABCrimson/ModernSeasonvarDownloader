#!/usr/bin/env bash
# Re-record the seasonvar fixtures. Review the diff before committing.
set -euo pipefail
cd "$(dirname "$0")/seasonvar"
UA="Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36"
for f in serials/serial-*.html; do
  href=$(grep -o -E '<link rel="canonical" href="[^"]+"|<meta property="og:url" content="[^"]+"' "$f" | head -1 | grep -o -E 'https://[^"]+')
  echo "GET $href"; curl -fsS -m 30 -A "$UA" -o "$f" "$href"
  id=$(basename "$f" .html | sed 's/serial-//')
  grep -o -E "(var pl = \{'0': |pl\[[0-9]+\] = )\"[^\"]+\"" "$f" | while read -r line; do
    tid=$(echo "$line" | grep -o -E 'pl\[[0-9]+\]' | grep -o -E '[0-9]+' || echo 0)
    p=$(echo "$line" | grep -o -E '"[^"]+"' | tr -d '"')
    echo "  GET $p -> playlists/plist-$id-$tid.json"; curl -fsS -m 30 -A "$UA" -o "playlists/plist-$id-$tid.json" "https://seasonvar.ru$p"
  done
done
curl -fsS -m 30 -A "$UA" -o misc/autocomplete-naruto.json "https://seasonvar.ru/autocomplete.php?query=naruto"
echo "done — run: git diff --stat fixtures/"
