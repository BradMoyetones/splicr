# Inter, cut down

Splicr's typeface, as Remapr's, is [Inter](https://rsms.me/inter/) by Rasmus Andersson, under the SIL Open Font
License 1.1 ([OFL-inter.txt](OFL-inter.txt)). It is the logo's typeface: the S on the sheet is
Inter SemiBold.

The two files here are Inter 4.1's static Regular and SemiBold (`extras/ttf/` in
[Inter-4.1.zip](https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip)), cut down to
the characters Splicr needs: Latin with its accents, punctuation, currency, arrows, math signs and
the keyboard symbols (⌘ ⌥ ⇧ ⌃ ⏎ ⌫ ⎋). That takes each file from about 410 KB to 60 KB.

| File in the zip | SHA-256 |
|---|---|
| `Inter-Regular.ttf` | `40d692fce188e4471e2b3cba937be967878f631ad3ebbbdcd587687c7ebe0c82` |
| `Inter-SemiBold.ttf` | `78a843fade9d4612a5567302fb595b56976eb5fcebf4fea5a5912d638bafcde3` |

To cut them again, for instance to add a script for a new language, use
[fontTools](https://github.com/fonttools/fonttools) 4.60:

```sh
RANGES="U+0020-007E,U+00A0-00FF,U+0100-017F,U+0192,U+02C6,U+02C7,U+02DA,U+02DC,U+2000-206F,\
U+20A0-20CF,U+2100-214F,U+2190-21FF,U+2200-22FF,U+2300-23FF,U+25A0-25FF,U+2713,U+2715"
for w in Regular SemiBold; do
  pyftsubset "Inter-$w.ttf" --unicodes="$RANGES" --layout-features=kern --no-hinting \
    --output-file="Inter-$w.ttf"
done
```

Characters outside the cut fall back to egui's bundled fonts.
