//! `/model` picker state: a leading Default (unpinned) row, one row per
//! configured model, one per configured main+subagent mode, and the Custom
//! entry (which stages a main-model pick, then a subagent-model pick), current
//! row marked, no live preview. A pick chains straight into `/thinking`, which
//! writes both at once.

use std::sync::Arc;

use serde_json::Value;

use crate::app::{App, CustomStage, ModelOption, ToastSeverity};
use crate::config_write::Scope;
use crate::event_handler;
use crate::server::Client;
use crate::thinking_picker::{self, ModelPick, THINKING_LEVELS};

/// Alias persisted for the Default (unpinned) row.
pub const UNPINNED_ACTIVE_MODEL: &str = "";

/// A picker row's target, in list order. Staged (custom main + subagent) rows
/// are the Default row plus the models; full rows append the profiles and the
/// Custom entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    Default,
    Model(usize),
    Profile(usize),
    Custom,
}

/// The row `i` stands for, given the picker's mode.
fn row_of(app: &App, i: usize) -> Row {
    if app.model_picker.custom_stage.is_some() {
        return match i {
            0 => Row::Default,
            j => Row::Model(j - 1),
        };
    }
    let models = app.model_picker.models.len();
    if i == 0 {
        Row::Default
    } else if i <= models {
        Row::Model(i - 1)
    } else if i <= models + app.model_picker.profiles.len() {
        Row::Profile(i - 1 - models)
    } else {
        Row::Custom
    }
}

/// The row the highlight is on.
pub fn selected_row(app: &App) -> Row {
    row_of(app, app.model_picker.list.selected())
}

/// A `/config` field write's server answer, applied on the main thread.
pub enum Event {
    /// Runtime returned by `config/write`.
    Written(Value),
    /// The write failed; the message names the field and the server's reason.
    Failed(String),
}

pub(crate) const FAILED_TOAST_SECS: u64 = 8;

/// Apply the server's answer and release the commit that was in flight.
pub fn apply_event(app: &mut App, event: Event) {
    match event {
        Event::Written(runtime) => event_handler::apply_runtime_value(app, &runtime),
        Event::Failed(message) => app.show_toast(message, ToastSeverity::Error, FAILED_TOAST_SECS),
    }
}

/// Act on the highlighted row: a model chains into the thinking picker, a
/// profile writes its main and subagent models together and chains the same
/// way on its main model, and the Custom entry stages the main-then-subagent
/// flow. Nothing is written until that picker commits or is dismissed. A
/// model that cannot be resolved has no levels to offer and is written alone.
pub fn select(app: &mut App, client: &Arc<Client>, scope: Scope) {
    match selected_row(app) {
        Row::Custom => enter_custom(app, CustomStage::PickMain),
        Row::Profile(i) => {
            let Some(profile) = app.model_picker.profiles.get(i) else {
                return;
            };
            let pick = ModelPick {
                alias: profile.main.clone(),
                target: profile.main.clone(),
                subagent_alias: Some(profile.subagent.clone()),
                scope,
                main_level: None,
            };
            app.model_picker.open = false;
            hand_off(app, client, pick);
        }
        _ => select_model_row(app, client, scope),
    }
}

/// The Default and model rows, the picker's whole list in the staged flow.
fn select_model_row(app: &mut App, client: &Arc<Client>, scope: Scope) {
    let alias = selected_alias(app);
    match app.model_picker.custom_stage {
        // First stage done: remember the main model and ask for the subagents'.
        Some(CustomStage::PickMain) => {
            let main = match alias.as_str() {
                UNPINNED_ACTIVE_MODEL => app.model_picker.default_alias.clone(),
                _ => alias,
            };
            enter_custom_with_main(app, main);
        }
        Some(CustomStage::PickSub) => {
            let main = app
                .model_picker
                .custom_main
                .clone()
                .unwrap_or_else(|| app.model_picker.default_alias.clone());
            let pick = ModelPick {
                alias: main.clone(),
                target: main,
                subagent_alias: Some(alias),
                scope,
                main_level: None,
            };
            app.model_picker.open = false;
            app.model_picker.custom_stage = None;
            hand_off(app, client, pick);
        }
        None => {
            app.model_picker.open = false;
            let target = match alias.as_str() {
                UNPINNED_ACTIVE_MODEL => app.model_picker.default_alias.clone(),
                _ => alias.clone(),
            };
            let pick = ModelPick {
                alias,
                target,
                subagent_alias: None,
                scope,
                main_level: None,
            };
            hand_off(app, client, pick);
        }
    }
}

/// Open the staged flow on its main-model step.
fn enter_custom(app: &mut App, stage: CustomStage) {
    app.model_picker.custom_stage = Some(stage);
    app.model_picker.list = crate::option_list::OptionList::new(0);
}

fn enter_custom_with_main(app: &mut App, main: String) {
    app.model_picker.custom_main = Some(main);
    enter_custom(app, CustomStage::PickSub);
}

/// Route a finished pick into the thinking picker: opened on its levels when
/// the model has them, written stand-alone when it cannot be resolved.
fn hand_off(app: &mut App, client: &Arc<Client>, pick: ModelPick) {
    let option = app
        .model_picker
        .models
        .iter()
        .find(|m| m.alias == pick.target);
    let Some((thinking, levels)) = option.map(|m| (m.thinking.clone(), m.thinking_levels.clone()))
    else {
        thinking_picker::commit_model(app, client, &pick);
        return;
    };
    thinking_picker::open_for_model(app, pick, thinking, levels);
}

/// Open the picker highlighted on the current choice.
pub fn open(app: &mut App) {
    app.model_picker.custom_stage = None;
    app.model_picker.custom_main = None;
    app.model_picker.list = crate::option_list::OptionList::new(current_index(app));
    app.model_picker.open = true;
}

/// Pre-selected row: pinned model `+1` (offset by Default), else `0` (Default).
fn current_index(app: &App) -> usize {
    if let Some(CustomStage::PickSub) = app.model_picker.custom_stage {
        if app.model_picker.current_subagent.is_empty() {
            return 0;
        }
    }
    if app.model_picker.is_pinned {
        if let Some(i) = model_index(app, &app.model_picker.current_model) {
            return i + 1;
        }
    }
    0
}

/// Index into `models` of the currently active alias, if any.
fn model_index(app: &App, alias: &str) -> Option<usize> {
    app.model_picker
        .models
        .iter()
        .position(|m| m.alias == alias)
}

/// Row count: the Default row plus one per model; full mode adds one per
/// profile and the Custom entry.
pub fn option_count(app: &App) -> usize {
    let base = app.model_picker.models.len() + 1;
    match app.model_picker.custom_stage {
        Some(_) => base,
        None => base + app.model_picker.profiles.len() + 1,
    }
}

/// Move the highlight up/down by one, wrapping at the ends.
pub fn navigate(app: &mut App, down: bool) {
    let count = option_count(app);
    app.model_picker.list.step(down, count, true, |_| true);
}

/// Whether row `i` is the current choice.
pub fn is_current(app: &App, i: usize) -> bool {
    match row_of(app, i) {
        Row::Default => {
            if app.model_picker.custom_stage == Some(CustomStage::PickSub) {
                return app.model_picker.current_subagent.is_empty();
            }
            !app.model_picker.is_pinned
        }
        Row::Model(j) => {
            let alias = match app.model_picker.custom_stage {
                Some(CustomStage::PickSub) => app.model_picker.current_subagent.as_str(),
                _ => {
                    if app.model_picker.is_pinned {
                        app.model_picker.current_model.as_str()
                    } else {
                        return false;
                    }
                }
            };
            model_index(app, alias) == Some(j)
        }
        Row::Profile(j) => {
            let Some(profile) = app.model_picker.profiles.get(j) else {
                return false;
            };
            app.model_picker.is_pinned
                && app.model_picker.current_model == profile.main
                && app.model_picker.current_subagent == profile.subagent
        }
        Row::Custom => false,
    }
}

/// The alias to persist for the highlighted row (`""` for Default).
pub fn selected_alias(app: &App) -> String {
    let sel = app.model_picker.list.selected();
    if sel == 0 {
        return UNPINNED_ACTIVE_MODEL.to_owned();
    }
    app.model_picker
        .models
        .get(sel - 1)
        .map(|m| m.alias.clone())
        .unwrap_or_default()
}

/// The picker's title: the staged flow names the step it is on.
pub fn title(app: &App) -> &'static str {
    match app.model_picker.custom_stage {
        Some(CustomStage::PickMain) => "Select Main Model",
        Some(CustomStage::PickSub) => "Select Subagent Model",
        None => "Select Model",
    }
}

/// A row's label and, when set, its dim hint beside it. The staged flow hides
/// the profiles and the Custom entry, and its Default row names the step it is
/// on instead.
pub fn option_label(app: &App, i: usize) -> (String, Option<String>) {
    match row_of(app, i) {
        Row::Default => match app.model_picker.custom_stage {
            Some(_) => ("Inherit session model".to_owned(), None),
            None => (
                "Default".to_owned(),
                Some(format!(
                    "(currently {})",
                    app.model_picker.default_display_name
                )),
            ),
        },
        Row::Model(j) => match app.model_picker.models.get(j) {
            Some(model) => (model.display_name.clone(), None),
            None => (String::new(), None),
        },
        Row::Profile(j) => match app.model_picker.profiles.get(j) {
            Some(profile) => (
                profile.display_name.clone(),
                Some(profile_hint(app, profile)),
            ),
            None => (String::new(), None),
        },
        Row::Custom => (
            "Custom main + subagents...".to_owned(),
            Some("(pick both, stage by stage)".to_owned()),
        ),
    }
}

/// The profile's pair in display names, falling back to the alias.
fn profile_hint(app: &App, profile: &crate::app::ModelProfileOption) -> String {
    let display = |alias: &str| {
        app.model_picker
            .models
            .iter()
            .find(|m| m.alias == alias)
            .map_or_else(|| alias.to_owned(), |m| m.display_name.clone())
    };
    format!(
        "main {}, subagents {}",
        display(&profile.main),
        display(&profile.subagent)
    )
}

/// Close the picker without persisting (Esc / cancel).
pub fn cancel(app: &mut App) {
    app.model_picker.open = false;
    app.model_picker.custom_stage = None;
    app.model_picker.custom_main = None;
}

/// Refresh the model snapshot from a `runtime/read`/`runtime/updated` value
/// (shape `{runtime: {config: {...}}}`), so the picker reflects live config.
pub fn apply_runtime(app: &mut App, response: &Value) {
    let Some(config) = crate::server::RuntimeSnapshot::config(response) else {
        return;
    };
    let models: Vec<ModelOption> = config.models.into_iter().filter_map(model_option).collect();
    let alias = config.default_model_alias;
    app.model_picker.default_display_name = models
        .iter()
        .find(|model| model.alias == alias)
        .map_or_else(|| alias.clone(), |model| model.display_name.clone());
    app.model_picker.models = models;
    app.model_picker.profiles = config
        .model_profiles
        .into_iter()
        .map(|profile| crate::app::ModelProfileOption {
            display_name: profile.display_name.unwrap_or_else(|| profile.name.clone()),
            name: profile.name,
            main: profile.main,
            subagent: profile.subagent,
        })
        .collect();
    app.model_picker.current_model = config.active_model.map(|m| m.alias).unwrap_or_default();
    app.model_picker.current_subagent = config.subagent_model;
    app.model_picker.is_pinned = config.active_model_pinned;
    app.model_picker.default_alias = alias;
}

fn model_option(model: crate::server::ModelView) -> Option<ModelOption> {
    if model.alias.is_empty() {
        return None;
    }
    // Older servers omit the per-model set; fall back to the canonical five (ADR 0014).
    let thinking_levels = match model.thinking_levels.is_empty() {
        true => THINKING_LEVELS.iter().map(|it| it.to_string()).collect(),
        false => model.thinking_levels,
    };
    Some(ModelOption {
        display_name: model.display_name.unwrap_or_else(|| model.alias.clone()),
        alias: model.alias,
        thinking: model.thinking.unwrap_or_default(),
        thinking_levels,
    })
}
