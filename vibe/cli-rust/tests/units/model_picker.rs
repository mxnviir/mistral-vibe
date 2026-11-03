//! A model pick opens the thinking picker at once on the picked model's levels, holding the pick.

use serde_json::{json, Value};
use vibe_rs::app::App;
use vibe_rs::config_write::Scope;
use vibe_rs::model_picker;
use vibe_rs::thinking_picker::{self, ModelPick};

use crate::support::stub;

fn runtime() -> Value {
    json!({"runtime": {"config": {
        "activeModel": {"alias": "medium", "thinking": "off", "thinkingLevels": ["off", "low"]},
        "defaultModelAlias": "medium",
        "models": [
            {"alias": "medium", "thinking": "off", "thinkingLevels": ["off", "low"]},
            {"alias": "large", "thinking": "high", "thinkingLevels": ["off", "high", "max"]},
            {"alias": "legacy", "thinking": "low"}
        ]
    }}})
}

fn select(app: &mut App, scope: Scope) {
    model_picker::select(app, &stub(), scope);
}

fn app_on(row: usize) -> App {
    let mut app = App::default();
    vibe_rs::event_handler::apply_runtime_value(&mut app, &runtime());
    model_picker::open(&mut app);
    app.model_picker.list.highlight(row);
    app
}

#[test]
fn pick_opens_thinking_on_the_picked_model() {
    let mut app = app_on(2);
    select(&mut app, Scope::Session);
    assert!(!app.model_picker.open);
    assert!(app.thinking_picker.open);
    assert_eq!(app.thinking_picker.levels, vec!["off", "high", "max"]);
    assert_eq!(thinking_picker::selected_level(&app), "high");
    assert_eq!(thinking_picker::enter_scope(&app), Scope::Session);
    assert_eq!(
        app.thinking_picker.model_pick,
        Some(ModelPick {
            alias: "large".into(),
            target: "large".into(),
            subagent_alias: None,
            scope: Scope::Session,
        })
    );
}

#[test]
fn default_row_targets_the_default_model() {
    let mut app = app_on(0);
    select(&mut app, Scope::Saved);
    let pick = app.thinking_picker.model_pick.clone().unwrap();
    assert_eq!((pick.alias.as_str(), pick.target.as_str()), ("", "medium"));
    assert_eq!(app.thinking_picker.levels, vec!["off", "low"]);
}

#[test]
fn model_without_levels_falls_back_to_the_canonical_five() {
    let mut app = app_on(3);
    select(&mut app, Scope::Saved);
    assert_eq!(app.thinking_picker.levels, thinking_picker::THINKING_LEVELS);
    assert_eq!(thinking_picker::selected_level(&app), "low");
}

#[test]
fn runtime_push_does_not_override_a_pending_pick() {
    let mut app = app_on(2);
    select(&mut app, Scope::Saved);
    vibe_rs::event_handler::apply_runtime_value(&mut app, &runtime());
    assert_eq!(app.thinking_picker.levels, vec!["off", "high", "max"]);
}

#[test]
fn plain_thinking_open_drops_a_stale_pick() {
    let mut app = app_on(2);
    select(&mut app, Scope::Session);
    thinking_picker::open(&mut app);
    assert_eq!(app.thinking_picker.model_pick, None);
    assert_eq!(thinking_picker::enter_scope(&app), Scope::Saved);
    assert_eq!(app.thinking_picker.levels, vec!["off", "low"]);
    assert_eq!(thinking_picker::selected_level(&app), "off");
}

#[test]
fn esc_drops_the_pick_and_restores_the_active_model() {
    let mut app = app_on(2);
    select(&mut app, Scope::Saved);
    thinking_picker::cancel(&mut app);
    assert!(!app.thinking_picker.open);
    assert_eq!(app.thinking_picker.model_pick, None);
    assert_eq!(app.thinking_picker.levels, vec!["off", "low"]);
    assert_eq!(app.thinking_picker.current_level, "off");
}

#[test]
fn unresolved_default_skips_the_thinking_picker() {
    let mut runtime = runtime();
    runtime["runtime"]["config"]
        .as_object_mut()
        .unwrap()
        .remove("defaultModelAlias");
    let mut app = App::default();
    vibe_rs::event_handler::apply_runtime_value(&mut app, &runtime);
    model_picker::open(&mut app);
    app.model_picker.list.highlight(0);
    select(&mut app, Scope::Saved);
    assert!(!app.model_picker.open);
    assert!(!app.thinking_picker.open);
    assert_eq!(app.thinking_picker.model_pick, None);
}

#[test]
fn a_pinned_model_opens_highlighted_and_marked_current() {
    let mut runtime = runtime();
    runtime["runtime"]["config"]["activeModel"]["alias"] = json!("large");
    runtime["runtime"]["config"]["activeModelPinned"] = json!(true);
    let mut app = App::default();
    vibe_rs::event_handler::apply_runtime_value(&mut app, &runtime);

    model_picker::open(&mut app);

    assert_eq!(app.model_picker.list.selected(), 2);
    assert!(model_picker::is_current(&app, 2));
    assert!(!model_picker::is_current(&app, 0));
    assert_eq!(model_picker::selected_alias(&app), "large");
}

/// A snapshot with one main+subagent mode and a subagent model set.
fn profile_runtime() -> Value {
    let mut value = runtime();
    let config = value["runtime"]["config"].as_object_mut().unwrap();
    config.insert(
        "modelProfiles".to_owned(),
        json!([{"name": "glm-mode", "main": "large", "subagent": "medium"}]),
    );
    config.insert("subagentModel".to_owned(), json!("medium"));
    config.insert(
        "activeModel".to_owned(),
        json!({"alias": "large", "thinking": "high", "thinkingLevels": ["off", "high", "max"]}),
    );
    config.insert("activeModelPinned".to_owned(), json!(true));
    value
}

/// The Custom row: after the Default row, the models and the profiles.
fn custom_row(app: &App) -> usize {
    model_picker::option_count(app) - 1
}

#[test]
fn full_mode_offers_one_row_per_profile_plus_the_custom_entry() {
    let mut app = App::default();
    vibe_rs::event_handler::apply_runtime_value(&mut app, &profile_runtime());
    model_picker::open(&mut app);
    assert_eq!(model_picker::option_count(&app), 6);
    assert_eq!(model_picker::option_label(&app, 4).0, "glm-mode");
    assert!(model_picker::option_label(&app, 5)
        .0
        .starts_with("Custom main"));
}

#[test]
fn a_fully_active_profile_row_is_marked_current() {
    let mut app = App::default();
    vibe_rs::event_handler::apply_runtime_value(&mut app, &profile_runtime());
    model_picker::open(&mut app);
    assert!(model_picker::is_current(&app, 4));
    assert!(!model_picker::is_current(&app, 5));
}

#[test]
fn a_profile_pick_holds_main_and_subagent_for_one_write() {
    let mut app = App::default();
    vibe_rs::event_handler::apply_runtime_value(&mut app, &profile_runtime());
    model_picker::open(&mut app);
    app.model_picker.list.highlight(4);
    select(&mut app, Scope::Saved);
    assert!(!app.model_picker.open);
    assert!(app.thinking_picker.open);
    assert_eq!(
        app.thinking_picker.model_pick,
        Some(ModelPick {
            alias: "large".into(),
            target: "large".into(),
            subagent_alias: Some("medium".into()),
            scope: Scope::Saved,
        })
    );
    assert_eq!(app.thinking_picker.levels, vec!["off", "high", "max"]);
}

#[test]
fn the_custom_flow_stages_main_then_subagent() {
    let mut app = App::default();
    vibe_rs::event_handler::apply_runtime_value(&mut app, &profile_runtime());
    model_picker::open(&mut app);
    app.model_picker.list.highlight(custom_row(&app));
    select(&mut app, Scope::Saved);
    // The first stage stays open, listing only the Default row and the models.
    assert!(app.model_picker.open);
    assert_eq!(
        app.model_picker.custom_stage,
        Some(vibe_rs::app::CustomStage::PickMain)
    );
    assert_eq!(model_picker::option_count(&app), 4);

    // Picking the main model moves to the subagents' stage.
    app.model_picker.list.highlight(2);
    select(&mut app, Scope::Saved);
    assert!(app.model_picker.open);
    assert_eq!(
        app.model_picker.custom_stage,
        Some(vibe_rs::app::CustomStage::PickSub)
    );
    assert_eq!(app.model_picker.custom_main.as_deref(), Some("large"));

    // "Inherit session model" unpins the subagent model; both writes travel together.
    app.model_picker.list.highlight(0);
    select(&mut app, Scope::Saved);
    assert_eq!(
        app.thinking_picker.model_pick,
        Some(ModelPick {
            alias: "large".into(),
            target: "large".into(),
            subagent_alias: Some(String::new()),
            scope: Scope::Saved,
        })
    );
}

#[test]
fn esc_closes_the_staged_flow_without_writing() {
    let mut app = App::default();
    vibe_rs::event_handler::apply_runtime_value(&mut app, &profile_runtime());
    model_picker::open(&mut app);
    app.model_picker.list.highlight(custom_row(&app));
    select(&mut app, Scope::Saved);
    model_picker::cancel(&mut app);
    assert!(!app.model_picker.open);
    assert_eq!(app.model_picker.custom_stage, None);
    assert!(!app.thinking_picker.open);
    assert_eq!(app.thinking_picker.model_pick, None);
}
