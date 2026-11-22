# New-version path live check (2026-11-22)

Goal: a “new version” reply must write a versioned sibling and leave the
active artifact byte-identical, instead of overwriting the original.

Environment: installed binary
`7d4c2b854883805d740daef17d46df4c470364a05536b67ae10755b18e46418b`.
Page test workspace `/tmp/tui-cont-test`; note test workspace
`/tmp/tui-newver-test`; real DeepSeek; TUI driven through tmux.

## Implementation

- `new_version_request` detects 新版本 / 再来一版 / 新的一版 / 另一版 /
  new version (and the reply right after the already-done gate question),
  unless the same text negates it or asks to verify without changes.
- The policy is active only when the ledger has unchanged active artifacts.
- `versioned_path(page.html)` → `page-v2.html`; if that exists →
  `page-v3.html`; an existing `page-v2.html` also advances to `page-v3.html`.
- A `NEW VERSION` system message lists active → versioned paths; write/edit
  calls to the active path are denied with `new_version_target` before
  execution, so the model retries at the versioned path.
- Browser actions no longer clear an accepted receipt; they do not modify the
  artifact file.

## Page test (debug binary)

Prompt: “做一个新版本，把 h1 改成 Gamma，其他不变。”

Run `d8ac4cfc`: 19 models, 0 writes to `page.html`, 1 write to
`page-v2.html`.

| File | Result |
|---|---|
| `page.html` | unchanged, 1097 bytes, sha256 `2b1b6b18…`, `<h1>Alpha</h1>` |
| `page-v2.html` | new, 1097 bytes, sha256 `81a93954…`, `<h1>Gamma</h1>` |

The answer was conditional because the final acceptance receipt was lost to a
browser audit that ran after acceptance; the browser exception was added
after this run. The overwrite protection itself passed: the original never
changed and the new file was the only write target.

## Note test (installed binary)

Session `ses_newver_20261122`, `--continue` for the second turn.

| Run | Task | Models | Result |
|---|---|---|---|
| `9dd5018c` | create `note.txt` = `first` | 7 | `note.txt` = `first` |
| `dfa0cd9d` | “做一个新版本，把内容改成 second，其他不变。” | 12 | `note-v2.txt` = `second`, **completed**, accepted |

`note.txt` stayed 5 bytes `first`; `note-v2.txt` is 6 bytes `second`. There
were 0 writes to the active path, 1 write to the versioned path, and no
`new_version_target` denial because the model followed the guidance directly.

## Deterministic test

`new_version_write_is_redirected_to_a_versioned_path` seeds an active
`page.html`, has the scripted model try to overwrite it, then write
`page-v2.html`. It asserts the original is unchanged, the versioned file is
written, and the journal records a `new_version_target` denial.

## Limits

- Only `write_file`/`edit_file` to the active path are blocked before
  execution; `terminal_exec` could still write it.
- Detection is phrase-based; paraphrases such as “下一版” are not matched yet.
- The policy requires unchanged active artifacts from the newest Act entry.
- Version names are `-vN` siblings; custom naming is not supported.
