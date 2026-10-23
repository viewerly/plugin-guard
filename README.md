# Guard

A permission policy for [Viewerly](https://viewerly.io): Guard answers the permission requests
**nobody is looking at**, by two rules you set, and leaves everything else to you.

- **Deny Bash commands containing**: a Bash request whose command contains any of these phrases
  is denied. Default: `rm -rf`. Checked first.
- **Always allow tools**: a request for one of these tools is allowed. Default: `Read, Glob, Grep,
  LS`.
- Anything else waits for you, exactly as without Guard.

Viewerly only lets Guard answer when no window, phone or dashboard has the session open: if you
are looking, you decide. Every answer shows in the session as "Allowed by Guard" or "Denied by
Guard", is listed under Guard in Settings › Plugins, and is written to Viewerly's decision log.

Guard also keeps a **panel** (what it allowed, denied and left to you, the last five decisions and
the rules in force, with a Reset button), a **status item** ("Guard: 3 allowed"), a palette
command **Guard: Reset Counts**, a session action **Guard** that reminds you what it does, and a
local notification when a session ends.

Guard is an automation with a panel, so it needs Viewerly Premium.

## Settings

Settings › Plugins › Guard has the two fields. Separate entries with commas.

- An empty field uses its default; type `none` to turn that rule off.
- Tool names are matched whole, ignoring case (`Read`, `mcp__github__get_issue`).
- Phrases are matched as plain text against the Bash command, with runs of spaces and tabs
  treated as one space, so `rm  -rf` matches `rm -rf`. This is a safety net, not a sandbox:
  `rm -r -f` or a script that deletes files does not match. Keep "Always allow tools" to tools that
  cannot change anything.

Changes apply at once.

## What it asks for

The grants sheet lists each of these; allow them one by one.

| Capability | Grants sheet | Why |
| --- | --- | --- |
| `events = ["permission.requested"]` | See permission requests, with the command or file they are about | To decide them by the rules. |
| `events = ["session.status"]` | See session status changes | To log a session that stops with an error. |
| `events = ["session.exited"]` | See sessions ending | For the "Session ended" notification. |
| `permissions = "decide"` | Answer permission requests nobody is looking at | The point of Guard. Shown with Guard's stated rules (`rules` in the manifest). |
| `notify = true` | Show notifications | "Session ended" and the session action's reminder. |

No network: Guard reaches nothing outside Viewerly. Without the decide grant it still counts
what it would have done, but answers nothing.

## Installing

Download `io.viewerly.guard-<version>.vplugin` from the
[releases](https://github.com/viewerly/plugin-guard/releases), then:

- **Settings › Plugins › Install from File…**, or
- `viewerly plugin install io.viewerly.guard-0.1.0.vplugin`.

Once it is listed in the directory, **Settings › Plugins › Browse Plugins…** installs it without a
download. A plugin starts with nothing allowed; the grants sheet opens so you can choose.

## Building

You need Rust and the WASM target:

```sh
rustup target add wasm32-wasip2
./build.sh     # guard.wasm, then dist/io.viewerly.guard-<version>.vplugin
cargo test     # the rules, natively
```

`src/lib.rs` is built against `wit/viewerly.wit`, Viewerly's plugin world
(`viewerly:plugin@0.1.0`), with [wit-bindgen](https://github.com/bytecodealliance/wit-bindgen). The
result is a WASM component, which is what Viewerly runs; a core module is refused at install. While
working on it, **Install Folder…** installs this folder directly once `guard.wasm` is built.

A `v*` tag tests, builds and attaches the `.vplugin` to a GitHub release
(`.github/workflows/release.yml`).

## Licence

MIT, © 2026 Viewerly. See [LICENSE](LICENSE).
