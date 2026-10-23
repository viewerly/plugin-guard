//! Guard: answers the permission requests nobody is looking at, by two lists the person sets in
//! Settings › Plugins › Guard, keeps a panel and a status item up to date, and says when a
//! session ends.
//!
//! The host only acts on Guard's answer when no window, phone or dashboard has the session open,
//! so Guard never overrides a person. Returning `None` leaves the request to them.

use std::cell::RefCell;

wit_bindgen::generate!({
    world: "plugin",
    path: "wit",
});

use viewerly::plugin::host;
use viewerly::plugin::types::{Button, Field, Permission, Status, Widget};

/// The settings fields, as declared in ui.toml.
const ALLOW: &str = "allow";
const DENY: &str = "deny";

/// What a field means before anyone has typed in it.
const DEFAULT_ALLOW: &str = "Read, Glob, Grep, LS";
const DEFAULT_DENY: &str = "rm -rf";

/// How many recent decisions the panel lists.
const RECENT: usize = 5;

/// The two lists, read from the settings.
#[derive(Default)]
struct Rules {
    /// Tool names, compared without regard to case.
    allow: Vec<String>,
    /// Phrases, compared with runs of whitespace collapsed to one space.
    deny: Vec<String>,
}

impl Rules {
    /// Reads both fields. An empty field has its default (the host does not keep empty values);
    /// `none` turns that rule off.
    fn read() -> Rules {
        let list = |id: &str, default: &str| -> Vec<String> {
            let text = host::setting(id).filter(|v| !v.trim().is_empty()).unwrap_or_else(|| default.to_owned());
            if text.trim().eq_ignore_ascii_case("none") {
                return Vec::new();
            }
            text.split(',')
                .map(|s| squash(s.trim()))
                .filter(|s| !s.is_empty())
                .collect()
        };
        Rules { allow: list(ALLOW, DEFAULT_ALLOW), deny: list(DENY, DEFAULT_DENY) }
    }

    /// Deny first, so allowing Bash outright still leaves the dangerous commands denied.
    fn decide(&self, p: &Permission) -> Option<Decision> {
        if p.tool == "Bash" {
            let command = squash(&bash_command(&p.input));
            if self.deny.iter().any(|phrase| command.contains(phrase.as_str())) {
                return Some(Decision::Deny);
            }
        }
        if self.allow.iter().any(|tool| tool.eq_ignore_ascii_case(&p.tool)) {
            return Some(Decision::Allow);
        }
        None
    }
}

/// The command a Bash request would run: the input's `command`, or the whole input if it is not
/// the JSON object Claude Code sends.
fn bash_command(input: &str) -> String {
    serde_json::from_str::<serde_json::Value>(input)
        .ok()
        .and_then(|v| v.get("command").and_then(|c| c.as_str()).map(str::to_owned))
        .unwrap_or_else(|| input.to_owned())
}

/// Runs of whitespace become one space, so `rm  -rf` and `rm\t-rf` match `rm -rf`.
fn squash(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Default)]
struct Counts {
    allowed: u32,
    denied: u32,
    passed: u32,
    recent: Vec<String>,
}

#[derive(Default)]
struct State {
    rules: Rules,
    counts: Counts,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

struct Guard;

impl Guest for Guard {
    fn on_event(event: Event) -> Option<Decision> {
        match event {
            // On a change, read both again rather than patching one: the host has already stored
            // the value.
            Event::Started | Event::SettingChanged(_) => {
                STATE.with_borrow_mut(|s| s.rules = Rules::read());
                refresh();
                show_rules();
            }
            Event::PermissionRequested(p) => {
                let decision = STATE.with_borrow_mut(|s| {
                    let decision = s.rules.decide(&p);
                    let c = &mut s.counts;
                    let word = match decision {
                        Some(Decision::Allow) => {
                            c.allowed += 1;
                            "allowed"
                        }
                        Some(Decision::Deny) => {
                            c.denied += 1;
                            "denied"
                        }
                        None => {
                            c.passed += 1;
                            "left to you"
                        }
                    };
                    c.recent.insert(0, format!("{}: {} {word}", p.session.title, p.tool));
                    c.recent.truncate(RECENT);
                    decision
                });
                refresh();
                return decision;
            }
            Event::SessionExited(s) => host::notify("Session ended", &format!("{} in {} has ended.", s.title, s.project)),
            Event::StatusChanged(s) if s.status == Status::Error => host::log(&format!("{} stopped with an error", s.title)),
            Event::Invoked(i) => match i.id.as_str() {
                "reset" => {
                    STATE.with_borrow_mut(|s| s.counts = Counts::default());
                    refresh();
                }
                "explain" => {
                    let title = i.session.map(|s| s.title).unwrap_or_else(|| "this session".into());
                    host::notify("Guard", &format!("Guard answers {title}'s requests while nobody is looking at it. Its rules are in Settings › Plugins › Guard."));
                }
                _ => {}
            },
            _ => {}
        }
        None
    }
}

/// Redraws the panel and the status item from the counts.
fn refresh() {
    STATE.with_borrow(|s| {
        let c = &s.counts;
        let mut widgets = vec![Widget::KeyValue(vec![
            ("Allowed".into(), c.allowed.to_string()),
            ("Denied".into(), c.denied.to_string()),
            ("Left to you".into(), c.passed.to_string()),
        ])];
        if c.recent.is_empty() {
            widgets.push(Widget::Text("Nothing decided yet.".into()));
        } else {
            widgets.push(Widget::List(c.recent.clone()));
        }
        widgets.push(Widget::Markdown(summary(&s.rules)));
        widgets.push(Widget::Button(Button { id: "reset".into(), label: "Reset".into() }));
        host::set_panel("activity", &widgets);
        host::set_status("guard", &format!("Guard: {} allowed", c.allowed));
    });
}

/// Redraws the settings pane: the fields, and the rules they make, in words.
fn show_rules() {
    STATE.with_borrow(|s| {
        host::set_panel(
            "rules",
            &[
                Widget::Markdown("Guard answers permission requests **only when nobody is looking** at the session. Separate entries with commas. An empty field uses the default shown in it; type `none` to turn that rule off.".into()),
                field(ALLOW, "Always allow tools", DEFAULT_ALLOW),
                field(DENY, "Deny Bash commands containing", DEFAULT_DENY),
                Widget::Markdown(summary(&s.rules)),
            ],
        );
    });
}

fn field(id: &str, label: &str, placeholder: &str) -> Widget {
    Widget::Field(Field { id: id.into(), label: label.into(), placeholder: placeholder.into(), secret: false })
}

/// The rules in force, in words.
fn summary(rules: &Rules) -> String {
    let words = |list: &[String], code: bool| -> String {
        let shown: Vec<String> = list.iter().map(|s| if code { format!("`{s}`") } else { s.clone() }).collect();
        shown.join(", ")
    };
    let allow = if rules.allow.is_empty() { "nothing".to_owned() } else { words(&rules.allow, false) };
    let deny = if rules.deny.is_empty() { "nothing".to_owned() } else { format!("Bash commands containing {}", words(&rules.deny, true)) };
    format!("**Allows** {allow}.\n\n**Denies** {deny}.\n\nEverything else waits for you.")
}

export!(Guard);

// The pure parts, run natively with `cargo test` (the host functions are only there in WASM).
#[cfg(test)]
mod tests {
    use super::*;

    fn ask(tool: &str, input: &str) -> Permission {
        use viewerly::plugin::types::Session;
        let session = Session { id: "s".into(), project: "p".into(), title: "t".into(), agent: "claude-code".into(), status: Status::WaitingForPermission };
        Permission { id: "r".into(), session, tool: tool.into(), summary: String::new(), input: input.into() }
    }

    fn rules(allow: &[&str], deny: &[&str]) -> Rules {
        Rules { allow: allow.iter().map(|s| s.to_string()).collect(), deny: deny.iter().map(|s| s.to_string()).collect() }
    }

    #[test]
    fn deny_wins_then_allow_then_you() {
        let r = rules(&["Read", "bash"], &["rm -rf"]);
        assert!(matches!(r.decide(&ask("read", "{}")), Some(Decision::Allow)));
        assert!(matches!(r.decide(&ask("Bash", r#"{"command":"rm   -rf /"}"#)), Some(Decision::Deny)));
        assert!(matches!(r.decide(&ask("Bash", r#"{"command":"ls"}"#)), Some(Decision::Allow)));
        assert!(r.decide(&ask("Write", "{}")).is_none());
        assert!(rules(&[], &[]).decide(&ask("Read", "{}")).is_none());
    }

    #[test]
    fn the_command_is_read_from_the_input() {
        assert_eq!(bash_command(r#"{"command":"echo hi","description":"x"}"#), "echo hi");
        assert_eq!(bash_command("not json"), "not json");
        assert_eq!(squash(" rm \t -rf  x "), "rm -rf x");
    }
}
