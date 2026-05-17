# jieba-rs POS Tagging Notes

## Source

- Local crate source: `C:\Users\10455\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\jieba-rs-0.9.0\src\lib.rs`

## Findings

- `jieba-rs` exposes `pub fn tag<'a>(&'a self, sentence: &'a str, hmm: bool) -> Vec<Tag<'a>>`.
- `Tag` contains `word: &str` and `tag: &str`; it does not include byte offsets.
- `tag()` internally calls `cut(sentence, hmm)`, then looks up each word in the dictionary records to recover the POS tag.
- `add_word(word, freq, tag)` stores the supplied tag in dictionary records, so user terms added with tag `nz` can be detected through `tag()`.
- Unknown non-dictionary tokens fall back to coarse tags: `x`, `m`, or `eng`.

## Implication For This Repo

- `TechSpanDetector` can replace the current `jieba.cut(text, false)` loop with `jieba.tag(text, false)`. HMM tagging was tested as a candidate, but it regressed existing TNL hot-path latency thresholds.
- Existing cursor-based `text[cursor..].find(word)` offset recovery is still needed.
- Named-entity tags should remain conservative: only `nr`, `ns`, `nt`, and `nz`.
- Since `NamedEntity` has lower priority than URL/email/path/identifier, POS tagging can safely add weak protection spans without overriding stronger technical detections.
