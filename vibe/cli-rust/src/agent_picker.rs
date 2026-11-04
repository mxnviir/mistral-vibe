//! `/agent` picker state: two labeled sections — the main agent (Default row
//! plus the primaries) and the subagent role (an Inherit row plus one role) —
//! and a Suggested row. The sections are chosen separately: Enter on a main
//! row switches the session live (the Shift+Tab path); Enter on a subagent row
//! sets the session's default subagent. `s` saves both defaults to the user's
//! config, as the rows display them.

use std::sync::Arc;

use serde_json::{json, Value};

use crate::app::App;
use crate::config_write::{self, Scope};
use crate::model_picker::FAILED_TOAST_SECS;
use crate::server::{method, AgentType, Client};

/// The subagent role a bare spawn runs; `""` keeps the generic child.
pub const UNSET_DEFAULT_SUBAGENT: &str = "";

/// A picker row, in list order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    /// The main-agent section's caption.
    MainHeader,
    /// The main agent the server starts with (`defaultAgent`).
    MainDefault,
    /// A primary agent, one per `AgentType::Agent` summary.
    MainAgent(usize),
    /// The subagent section's caption.
    SubHeader,
    /// The generic child: bare spawns inherit the session's model and tools.
    SubagentDefault,
    /// A subagent role, one per `AgentType::Subagent` summary.
    Subagent(usize),
    /// `/agent`'s recommendation for this session, with its reason.
    Suggested,
}

/// The recommendation the Suggested row applies: a main agent and a subagent
/// role, both optional so the row only offers what this session has.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Suggestion {
    pub main: Option<String>,
    pub subagent: Option<String>,
    pub reason: &'static str,
}

/// `/agent` picker state (a bottom-app).
#[derive(Default)]
pub struct AgentPicker {
    /// While set the picker replaces the input box: title, option list, hint.
    pub open: bool,
    /// Highlighted row and the viewport.
    pub list: crate::option_list::OptionList<crate::option_list::MultiAction>,
    /// The recommendation the Suggested row resolves to, computed at open.
    pub suggestion: Option<Suggestion>,
}

/// Open the picker highlighted on the row matching what is live now.
pub fn open(app: &mut App) {
    app.agent_picker.suggestion = Some(suggest(app));
    app.agent_picker.list = crate::option_list::OptionList::new(current_index(app));
    app.agent_picker.open = true;
}

/// Close the picker without persisting (Esc / cancel).
pub fn cancel(app: &mut App) {
    app.agent_picker.open = false;
}

/// Row count: the two section captions, the Default row, one per agent, the
/// Inherit row, and Suggested.
pub fn option_count(app: &App) -> usize {
    app.agents.all.len() + 5
}

/// The row `i` stands for. Index order: the main caption, Default, the
/// primaries, the subagent caption, Inherit, the subagent roles, Suggested.
pub fn row_of(app: &App, i: usize) -> Option<Row> {
    let primaries = primary_count(app);
    let subagents = subagent_count(app);
    let last = option_count(app) - 1;
    match i {
        0 => Some(Row::MainHeader),
        1 => Some(Row::MainDefault),
        i if i <= primaries + 1 => Some(Row::MainAgent(i - 2)),
        i if i == primaries + 2 => Some(Row::SubHeader),
        i if i == primaries + 3 => Some(Row::SubagentDefault),
        i if i <= primaries + 3 + subagents => Some(Row::Subagent(i - primaries - 4)),
        i if i == last => Some(Row::Suggested),
        _ => None,
    }
}

/// Count of `AgentType::Subagent` summaries.
fn subagent_count(app: &App) -> usize {
    app.agents
        .all
        .iter()
        .filter(|agent| agent.agent_type == AgentType::Subagent)
        .count()
}

/// Whether row `i` is a section caption: painted dim, never highlighted,
/// never selectable.
pub fn is_header(app: &App, i: usize) -> bool {
    matches!(row_of(app, i), Some(Row::MainHeader) | Some(Row::SubHeader))
}

/// Count of `AgentType::Agent` summaries.
fn primary_count(app: &App) -> usize {
    app.agents
        .all
        .iter()
        .filter(|agent| agent.agent_type == AgentType::Agent)
        .count()
}

/// The `j`-th primary agent.
fn primary(app: &App, j: usize) -> Option<crate::server::AgentSummary> {
    app.agents
        .all
        .iter()
        .filter(|agent| agent.agent_type == AgentType::Agent)
        .nth(j)
        .cloned()
}

/// The `j`-th subagent role.
fn subagent(app: &App, j: usize) -> Option<crate::server::AgentSummary> {
    app.agents
        .all
        .iter()
        .filter(|agent| agent.agent_type == AgentType::Subagent)
        .nth(j)
        .cloned()
}

/// The configured default main agent's name (`defaultAgent`).
fn default_main(app: &App) -> String {
    app.session.startup_config.default_agent.clone()
}

/// The configured default subagent role's name (`defaultSubagent`).
fn default_subagent(app: &App) -> &str {
    &app.session.startup_config.default_subagent
}

/// The row to open highlighted on: the first row that marks what is live.
fn current_index(app: &App) -> usize {
    (0..option_count(app))
        .find(|&i| is_current(app, i))
        .unwrap_or(0)
}

/// Whether row `i` names what is live now: a main row the session runs, a
/// subagent row set as the default, the Default/Inherit rows when their
/// underlying live value matches, and never Suggested.
pub fn is_current(app: &App, i: usize) -> bool {
    match row_of(app, i) {
        Some(Row::MainHeader) | Some(Row::SubHeader) => false,
        Some(Row::MainDefault) => default_main(app) == agents_active(app),
        Some(Row::MainAgent(j)) => match primary(app, j) {
            Some(agent) => agent.name == agents_active(app) && agent.name != default_main(app),
            None => false,
        },
        Some(Row::SubagentDefault) => default_subagent(app).is_empty(),
        Some(Row::Subagent(j)) => match subagent(app, j) {
            Some(agent) => default_subagent(app) == agent.name,
            None => false,
        },
        Some(Row::Suggested) | None => false,
    }
}

/// The agent the session runs: a switch in flight's target, else the active one.
fn agents_active(app: &App) -> String {
    crate::agents::displayed(app).name.clone()
}

/// The picker's title.
pub fn title(_app: &App) -> &'static str {
    "Select Agent"
}

/// A row's label and, when set, its dim hint beside it.
pub fn option_label(app: &App, i: usize) -> (String, Option<String>) {
    match row_of(app, i) {
        Some(Row::MainHeader) => ("MAIN AGENT".to_owned(), None),
        Some(Row::SubHeader) => ("SUBAGENT ROLE".to_owned(), None),
        Some(Row::MainDefault) => (
            "Default".to_owned(),
            Some(format!("(currently {})", default_main_display(app))),
        ),
        Some(Row::MainAgent(j)) => match primary(app, j) {
            Some(agent) => (agent.display_name, Some(agent.description)),
            None => (String::new(), None),
        },
        Some(Row::SubagentDefault) => (
            "Inherit session".to_owned(),
            Some("(bare subagent spawns run a generic child on the session's model)".to_owned()),
        ),
        Some(Row::Subagent(j)) => match subagent(app, j) {
            Some(agent) => (agent.display_name, Some(agent.description)),
            None => (String::new(), None),
        },
        Some(Row::Suggested) => match &app.agent_picker.suggestion {
            Some(suggestion) => {
                let (main, subagent) = suggest_labels(app, suggestion);
                (
                    format!("Suggested: {main} + {subagent}"),
                    Some(suggestion.reason.to_owned()),
                )
            }
            None => ("Suggested".to_owned(), None),
        },
        None => (String::new(), None),
    }
}

/// The Default row's hint name: the configured default agent's display name.
fn default_main_display(app: &App) -> String {
    let default = default_main(app);
    app.agents
        .all
        .iter()
        .find(|agent| agent.name == default)
        .map_or_else(|| default.clone(), |agent| agent.display_name.clone())
}

/// The suggestion's two names in display form, falling back to the alias.
fn suggest_labels(app: &App, suggestion: &Suggestion) -> (String, String) {
    let display = |name: &Option<String>, agent_type: AgentType| {
        name.as_ref()
            .and_then(|name| {
                app.agents
                    .all
                    .iter()
                    .find(|agent| agent.name == *name && agent.agent_type == agent_type)
                    .cloned()
            })
            .map_or_else(
                || name.clone().unwrap_or_default(),
                |agent| agent.display_name,
            )
    };
    (
        display(&suggestion.main, AgentType::Agent),
        display(&suggestion.subagent, AgentType::Subagent),
    )
}

/// What this session wants: smart-approve for the main agent (auto-runs safe
/// tool calls, prompts for risky ones), Explore for the subagent role. Only
/// rows this session's snapshot offers; never one it cannot switch to.
fn suggest(app: &App) -> Suggestion {
    let main = app
        .agents
        .all
        .iter()
        .find(|agent| agent.agent_type == AgentType::Agent && agent.name == "smart-approve")
        .map(|agent| agent.name.clone());
    let subagent = app
        .agents
        .all
        .iter()
        .find(|agent| agent.agent_type == AgentType::Subagent && agent.name == "explore")
        .map(|agent| agent.name.clone());
    Suggestion {
        main,
        subagent,
        reason: "auto-runs safe tool calls, prompts for risky ones",
    }
}

/// Move the highlight up/down by one, wrapping at the ends; the section
/// captions are skipped, never highlighted.
pub fn navigate(app: &mut App, down: bool) {
    let count = option_count(app);
    let headers: Vec<usize> = (0..count).filter(|&i| is_header(app, i)).collect();
    app.agent_picker
        .list
        .step(down, count, true, |i| !headers.contains(&i));
}

/// Enter on the highlighted row:
/// - a main row is a live switch via the Shift+Tab path (`session/agent/update`);
/// - a subagent row writes `/default_subagent` to the session layer, which the
///   next bare spawn picks up;
/// - the Suggested row applies both of its names.
pub fn select(app: &mut App, client: &Arc<Client>) {
    let Some(row) = row_of(app, app.agent_picker.list.selected()) else {
        return;
    };
    app.agent_picker.open = false;
    apply_row(app, client, row);
}

/// Apply a confirmed row; the section captions are scenery, never a choice.
fn apply_row(app: &mut App, client: &Arc<Client>, row: Row) {
    let session_subagent = default_subagent(app).to_owned();
    match row {
        Row::MainHeader | Row::SubHeader => {}
        Row::MainDefault => {
            let default = default_main(app);
            if default != agents_active(app) {
                crate::agents::pick(app, client, default);
            }
        }
        Row::MainAgent(j) => {
            if let Some(agent) = primary(app, j) {
                if agent.name != agents_active(app) {
                    crate::agents::pick(app, client, agent.name);
                }
            }
        }
        Row::SubagentDefault => {
            if !session_subagent.is_empty() {
                commit_subagent(app, client, UNSET_DEFAULT_SUBAGENT, Scope::Session);
            }
        }
        Row::Subagent(j) => {
            if let Some(agent) = subagent(app, j) {
                if agent.name != session_subagent {
                    commit_subagent(app, client, &agent.name, Scope::Session);
                }
            }
        }
        Row::Suggested => {
            if let Some(suggestion) = app.agent_picker.suggestion.clone() {
                if let Some(main) = suggestion.main {
                    if main != agents_active(app) {
                        crate::agents::pick(app, client, main);
                    }
                }
                let target = suggestion
                    .subagent
                    .unwrap_or_else(|| UNSET_DEFAULT_SUBAGENT.to_owned());
                if target != session_subagent {
                    commit_subagent(app, client, &target, Scope::Saved);
                }
            }
        }
    }
}

/// `s`: save both defaults as the rows display them, in one write batch.
pub fn save(app: &mut App, client: &Arc<Client>) {
    let main = default_main(app);
    let subagent = default_subagent(app).to_owned();
    let mut ops = config_write::pick_ops("/default_agent", &main, Scope::Saved);
    ops.extend(config_write::pick_ops(
        "/default_subagent",
        &subagent,
        Scope::Saved,
    ));
    commit(app, client, ops, "agent picker");
    app.agent_picker.open = false;
}

/// Write the session's default subagent via the idle-only `config/write`, then
/// reload the runtime so the next spawn reads it.
fn commit_subagent(app: &mut App, client: &Arc<Client>, name: &str, scope: Scope) {
    let ops = config_write::pick_ops("/default_subagent", name, scope);
    commit(app, client, ops, "default subagent pick");
}

/// Commit `ops` through `config/write`, then `config/reload`; one event reaches
/// the UI, which toasts the outcome.
fn commit(app: &mut App, client: &Arc<Client>, ops: Vec<Value>, reason: &str) {
    if ops.is_empty() {
        return;
    }
    let Some(session_id) = app.session.session_id.clone() else {
        return;
    };
    let client = client.clone();
    let reason = reason.to_owned();
    crate::ui_event::spawn(app, async move {
        let written = match config_write::write(&client, &session_id, ops, &reason).await {
            Err(error) => {
                return Event::Failed(format!("Failed to apply: {reason} — {error}"));
            }
            Ok(written) => written,
        };
        match crate::config_write::mutation_error(&written) {
            Some(error) => Event::Failed(format!("Failed to apply: {reason} — {error}")),
            // Only a clean, applied write is worth reloading: `config/reload`
            // is idle-only, and a summary write is only committed while idle.
            None => match client
                .request(
                    method::CONFIG_RELOAD,
                    json!({"sessionId": session_id, "reloadRuntime": true}),
                )
                .await
            {
                Err(error) => Event::Failed(format!(
                    "Failed to apply: {reason} — {}",
                    crate::server::request_error::describe(&error)
                )),
                Ok(_) => Event::Written,
            },
        }
    });
}

/// A settled config write's answer, applied on the main thread.
pub enum Event {
    /// The write and the reload both landed.
    Written,
    /// The write or the reload failed; the message names the pick and why.
    Failed(String),
}

/// Apply a settled write: a failure is a toast; a success needs nothing (the
/// runtime refreshing reaches the UI through `runtime/updated`).
pub fn apply_event(app: &mut App, event: Event) {
    if let Event::Failed(message) = event {
        app.show_toast(message, crate::app::ToastSeverity::Error, FAILED_TOAST_SECS);
    }
}
