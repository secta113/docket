//! `rotproof stop-hook`: the hook an agent runs when it stops (Claude Code's `Stop`, Gemini CLI's `AfterAgent`). It
//! sends the agent back once when its last message leaves something open and nothing in `docs/` changed.
//!
//! - An agent's findings are written somewhere before they are lost: in its report ("not checked", "out of scope").
//!   The hook reads that report when the agent stops, and when it holds one of [`PHRASES`] while `docs/` has no
//!   change (`git status --porcelain -- docs`), it asks the agent to record the finding or to say where it already is.
//! - The agent decides what the phrase meant; the hook only makes the moment. It answers each agent in its own form
//!   ([`Agent`]): Claude Code with `additionalContext`, which it shows as feedback rather than an error; Gemini CLI
//!   with `decision: "deny"`, whose `reason` it sends the agent as the next prompt.
//! - It sends the agent back at most once per stop: while the agent is already continuing because of a stop hook
//!   (`stop_hook_active`, in both), it lets the agent stop, so a phrase quoted in the answer cannot loop.
//! - The project is the nearest directory, from where the hook runs upwards, that holds `.config/rotproof.toml`.
//!   Outside one, the hook lets the agent stop and says nothing: it may be configured for every repository.
//!
//! An input from neither hook, or a `git` that fails, is an error: the agent shows it, and stops.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Value, json};

use crate::layers::DECLARATION;

/// The agents whose stop hook this answers, by the `hook_event_name` of their input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Agent {
    /// Claude Code's `Stop`: the last message in `last_assistant_message`
    Claude,
    /// Gemini CLI's `AfterAgent`: the last message in `prompt_response`
    Gemini,
}

impl Agent {
    fn of(event: &str) -> Option<Agent> {
        match event {
            "Stop" => Some(Agent::Claude),
            "AfterAgent" => Some(Agent::Gemini),
            _ => None,
        }
    }

    /// The field that holds the agent's last message.
    fn message_field(self) -> &'static str {
        match self {
            Agent::Claude => "last_assistant_message",
            Agent::Gemini => "prompt_response",
        }
    }

    /// What sends the agent back with `text`.
    fn send_back(self, text: String) -> Value {
        match self {
            Agent::Claude => json!({
                "hookSpecificOutput": {
                    "hookEventName": "Stop",
                    "additionalContext": text,
                }
            }),
            Agent::Gemini => json!({ "decision": "deny", "reason": text }),
        }
    }
}

/// The settings that run the hook, and where `rotproof create` writes each, once: the project's files from then on.
/// `rotproof` has to be on the `PATH` the agent runs hooks with.
pub const SETTINGS: [(&str, &str); 2] = [
    (
        ".claude/settings.json",
        r#"{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "rotproof stop-hook"
          }
        ]
      }
    ]
  }
}
"#,
    ),
    (
        ".gemini/settings.json",
        r#"{
  "hooks": {
    "AfterAgent": [
      {
        "hooks": [
          {
            "name": "rotproof-stop-hook",
            "type": "command",
            "command": "rotproof stop-hook"
          }
        ]
      }
    ]
  }
}
"#,
    ),
];

// A line that points at the records: the word spec, specs or backlog standing alone in ASCII (so `spec に`,
// `docs/specs/x.md` and `Backlog` count, and `specific` or `inspect` do not), or a closed spec in docs/done/. Matched
// against the line in lower case
static RECORDS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|[^a-z0-9_])(?:specs?|backlog)(?:[^a-z0-9_]|$)|docs/done/").unwrap()
});

/// What in a report leaves something open, matched in any case. Words common in plain prose ("later") are left out:
/// a hook that fires on every message is answered without reading.
pub const PHRASES: [&str; 15] = [
    "未確認",
    "後で",
    "あとで",
    "別途",
    "対象外",
    "未決",
    "見送",
    "未着手",
    "保留",
    "not checked",
    "not verified",
    "unverified",
    "out of scope",
    "follow-up",
    "todo",
];

/// What the hook prints for `input` (the JSON the agent writes on stdin), run from `start`. `None` lets the agent
/// stop, with nothing printed.
pub fn run(start: &Path, input: &str) -> Result<Option<String>, String> {
    decide(input, || match project_root(start) {
        Some(root) => docs_changed(&root),
        // Outside a project there is nothing to record into
        None => Ok(true),
    })
}

/// The decision, with `recorded` saying whether `docs/` changed. It is asked only when a phrase is found.
fn decide(
    input: &str,
    recorded: impl FnOnce() -> Result<bool, String>,
) -> Result<Option<String>, String> {
    let input: Value =
        serde_json::from_str(input).map_err(|e| format!("the hook input is not JSON: {e}"))?;
    let event = input
        .get("hook_event_name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let Some(agent) = Agent::of(event) else {
        return Err(format!(
            "the hook input is from {event:?}: run `rotproof stop-hook` as Claude Code's Stop hook or Gemini CLI's \
             AfterAgent hook"
        ));
    };
    if input.get("stop_hook_active").and_then(Value::as_bool) == Some(true) {
        return Ok(None);
    }
    let field = agent.message_field();
    let message = match input.get(field) {
        Some(Value::String(message)) => message.as_str(),
        // A turn that ended without text
        Some(Value::Null) => "",
        _ => return Err(format!("the {event} hook input has no {field}")),
    };
    let found = open_phrases(message);
    if found.is_empty() || recorded()? {
        return Ok(None);
    }
    let quoted: Vec<String> = found.iter().map(|phrase| format!("\"{phrase}\"")).collect();
    let text = format!(
        "Your last message says {} and nothing in docs/ changed. If it leaves a finding open, record it now: an \
         item in docs/backlog/ (docs/backlog/rules.md), or the spec it belongs to. If it is already recorded, or is not \
         a finding, say where or why in one line, then stop.",
        quoted.join(", ")
    );
    Ok(Some(agent.send_back(text).to_string()))
}

/// The phrases of [`PHRASES`] that `message` holds, in the order of the list. A line that points at the records is
/// not read: what it leaves open is recorded where it points.
fn open_phrases(message: &str) -> Vec<&'static str> {
    let message: String = message
        .to_lowercase()
        .lines()
        .filter(|line| !RECORDS.is_match(line))
        .collect::<Vec<_>>()
        .join("\n");
    PHRASES
        .into_iter()
        .filter(|phrase| message.contains(phrase))
        .collect()
}

/// The nearest directory from `start` upwards that holds the declaration.
fn project_root(start: &Path) -> Option<PathBuf> {
    let start = start.canonicalize().ok()?;
    start
        .ancestors()
        .find(|dir| dir.join(DECLARATION).is_file())
        .map(Path::to_path_buf)
}

/// Whether `git status` shows any change in `docs/` of `root`: changed, staged or new.
fn docs_changed(root: &Path) -> Result<bool, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["status", "--porcelain", "--", "docs"])
        .output()
        .map_err(|e| format!("git could not run: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git status failed in {}: {}",
            root.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(!out.stdout.iter().all(u8::is_ascii_whitespace))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(message: &str, active: bool) -> String {
        json!({
            "hook_event_name": "Stop",
            "stop_hook_active": active,
            "last_assistant_message": message,
        })
        .to_string()
    }

    fn gemini(message: &str, active: bool) -> String {
        json!({
            "hook_event_name": "AfterAgent",
            "prompt": "fix it",
            "stop_hook_active": active,
            "prompt_response": message,
        })
        .to_string()
    }

    #[test]
    fn a_phrase_with_no_change_in_docs_sends_the_agent_back() {
        let out = decide(
            &input("Done. The Windows path is not checked.", false),
            || Ok(false),
        )
        .unwrap()
        .expect("sent back");
        let out: Value = serde_json::from_str(&out).unwrap();
        let context = out["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap();
        assert!(context.contains("\"not checked\""), "{context}");
        assert_eq!(out["hookSpecificOutput"]["hookEventName"], "Stop");
    }

    #[test]
    fn gemini_cli_is_sent_back_in_its_own_form() {
        let out = decide(&gemini("Linux は未確認です", false), || Ok(false))
            .unwrap()
            .expect("sent back");
        let out: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(out["decision"], "deny");
        assert!(
            out["reason"].as_str().unwrap().contains("\"未確認\""),
            "{out}"
        );
        assert_eq!(
            decide(&gemini("未確認", true), || panic!("not asked")),
            Ok(None)
        );
        // Each agent's message is read from its own field only
        let crossed = json!({"hook_event_name": "AfterAgent", "last_assistant_message": "未確認"});
        assert!(decide(&crossed.to_string(), || Ok(false)).is_err());
    }

    #[test]
    fn every_phrase_is_found_in_any_case() {
        for phrase in PHRASES {
            let upper = phrase.to_uppercase();
            assert_eq!(open_phrases(&format!("x {upper} y")), [phrase], "{phrase}");
        }
    }

    #[test]
    fn a_line_that_points_at_the_records_is_not_read() {
        for line in [
            "未決の問いは spec に書いた",
            "未決の問いはspecに書いた",
            "Backlog: the PATH is not checked",
            "- [x.md](docs/specs/x.md): 未確認",
            "閉じた [y.md](/backlog/y.md) は未着手のまま",
            "[z.md](docs/done/z.md) で見送った",
        ] {
            assert_eq!(open_phrases(line), Vec::<&str>::new(), "{line}");
        }
        // Only that line: the next one is read
        assert_eq!(open_phrases("spec に書いた\nLinux は未確認"), ["未確認"]);
        // A word that only contains spec is not a pointer
        for line in ["the specific path is not checked", "inspect: not checked"] {
            assert_eq!(open_phrases(line), ["not checked"], "{line}");
        }
    }

    #[test]
    fn the_agent_stops_when_docs_changed_or_no_phrase_or_already_sent_back() {
        assert_eq!(decide(&input("未確認のまま", false), || Ok(true)), Ok(None));
        assert_eq!(
            decide(&input("All done.", false), || panic!("not asked")),
            Ok(None)
        );
        assert_eq!(
            decide(&input("未確認のまま", true), || panic!("not asked")),
            Ok(None)
        );
        let null = json!({"hook_event_name": "Stop", "stop_hook_active": false, "last_assistant_message": null});
        assert_eq!(decide(&null.to_string(), || panic!("not asked")), Ok(None));
    }

    #[test]
    fn an_input_from_neither_hook_fails() {
        assert!(decide("not json", || Ok(false)).is_err());
        assert!(decide("{}", || Ok(false)).is_err());
        let other = json!({"hook_event_name": "PreToolUse", "last_assistant_message": "未確認"});
        assert!(decide(&other.to_string(), || Ok(false)).is_err());
        let empty = json!({"hook_event_name": "Stop"});
        assert!(decide(&empty.to_string(), || Ok(false)).is_err());
    }

    #[test]
    fn a_failing_git_is_an_error() {
        assert!(decide(&input("未決", false), || Err("git".into())).is_err());
    }
}
