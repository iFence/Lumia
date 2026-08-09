use std::collections::BTreeMap;

use lumia_plugin_api::{
    CanvasToolKind, CanvasToolSettings, CanvasToolState, CommandContribution, LocalizedText,
    MenuContribution, PanelControl, PanelModel, PanelOption, PluginAsset, PluginCapability,
    PluginContributions, PluginIcon, PluginManifest,
};

use crate::{validate_canvas_state, validate_panel_model, validate_ui_manifest};

fn text(value: &str) -> LocalizedText {
    LocalizedText {
        fallback: value.into(),
        translations: BTreeMap::new(),
    }
}

fn manifest() -> PluginManifest {
    PluginManifest {
        id: "lumia.annotation".into(),
        name: "Annotation".into(),
        version: "0.1.0".into(),
        entry: "annotation".into(),
        capabilities: vec![
            PluginCapability::UiContributions,
            PluginCapability::CanvasOverlay,
        ],
        permissions: Vec::new(),
        supported_inputs: Vec::new(),
        supported_extensions: Vec::new(),
        supported_outputs: vec!["image/png".into()],
        contributions: PluginContributions {
            commands: vec![CommandContribution {
                id: "open".into(),
                label: text("Open"),
                icon: PluginIcon::Annotation,
                requires_document: true,
            }],
            viewer_context_menu: vec![MenuContribution {
                id: "menu".into(),
                command_id: "open".into(),
                group: 0,
                order: 0,
            }],
            ..PluginContributions::default()
        },
        assets: vec![PluginAsset {
            id: "pin".into(),
            path: "assets/pin.svg".into(),
            media_type: "image/svg+xml".into(),
            sha256: "0".repeat(64),
        }],
    }
}

#[test]
fn manifest_rejects_unknown_commands_and_unsafe_assets() {
    let mut manifest = manifest();
    manifest.contributions.viewer_context_menu[0].command_id = "missing".into();
    assert!(validate_ui_manifest(&manifest).is_err());

    manifest.contributions.viewer_context_menu[0].command_id = "open".into();
    manifest.assets[0].path = "../bad.svg".into();
    assert!(validate_ui_manifest(&manifest).is_err());
}

#[test]
fn panel_rejects_unknown_assets_and_non_finite_sliders() {
    let manifest = manifest();
    let mut panel = PanelModel {
        title: text("Panel"),
        sections: vec![lumia_plugin_api::PanelSection {
            id: "main".into(),
            title: None,
            controls: vec![PanelControl::Select {
                id: "icon".into(),
                label: text("Icon"),
                options: vec![PanelOption {
                    value: "pin".into(),
                    label: text("Pin"),
                    icon: Some(PluginIcon::Asset("missing".into())),
                }],
                selected: "pin".into(),
                enabled: true,
            }],
        }],
    };
    assert!(validate_panel_model(&manifest, &panel).is_err());

    panel.sections[0].controls = vec![PanelControl::Slider {
        id: "size".into(),
        label: text("Size"),
        value: f32::NAN,
        min: 1.0,
        max: 10.0,
        step: 1.0,
        enabled: true,
    }];
    assert!(validate_panel_model(&manifest, &panel).is_err());
}

#[test]
fn canvas_state_must_reference_a_declared_typed_tool() {
    let mut manifest = manifest();
    manifest.contributions.canvas_tools = vec![
        lumia_plugin_api::CanvasToolContribution {
            id: "annotation.text".into(),
            label: text("Text"),
            icon: PluginIcon::Text,
            kind: CanvasToolKind::Text,
        },
        lumia_plugin_api::CanvasToolContribution {
            id: "annotation.rectangle".into(),
            label: text("Rectangle"),
            icon: PluginIcon::Rectangle,
            kind: CanvasToolKind::Rectangle,
        },
        lumia_plugin_api::CanvasToolContribution {
            id: "annotation.numbered_step".into(),
            label: text("Numbered step"),
            icon: PluginIcon::NumberedStep,
            kind: CanvasToolKind::NumberedStep,
        },
    ];
    let text_tool = CanvasToolState {
        tool_id: "annotation.text".into(),
        settings: CanvasToolSettings::Text {
            font_size: 24.0,
            color: "#ff0000".into(),
            opacity: 1.0,
        },
    };
    let rectangle_tool = CanvasToolState {
        tool_id: "annotation.rectangle".into(),
        settings: CanvasToolSettings::Rectangle {
            stroke_width: 4.0,
            color: "#ff0000".into(),
            opacity: 1.0,
        },
    };
    let step_tool = CanvasToolState {
        tool_id: "annotation.numbered_step".into(),
        settings: CanvasToolSettings::NumberedStep {
            size: 24.0,
            color: "#ff0000".into(),
            opacity: 1.0,
        },
    };
    validate_canvas_state(&manifest, Some(&text_tool)).unwrap();
    validate_canvas_state(&manifest, Some(&rectangle_tool)).unwrap();
    validate_canvas_state(&manifest, Some(&step_tool)).unwrap();

    let mut unknown = text_tool;
    unknown.tool_id = "missing".into();
    assert!(validate_canvas_state(&manifest, Some(&unknown)).is_err());

    let out_of_range = CanvasToolState {
        tool_id: "annotation.text".into(),
        settings: CanvasToolSettings::Text {
            font_size: 9999.0,
            color: "#ff0000".into(),
            opacity: 1.0,
        },
    };
    assert!(validate_canvas_state(&manifest, Some(&out_of_range)).is_err());

    let mismatched = CanvasToolState {
        tool_id: "annotation.text".into(),
        settings: CanvasToolSettings::NumberedStep {
            size: 24.0,
            color: "#ff0000".into(),
            opacity: 1.0,
        },
    };
    assert!(validate_canvas_state(&manifest, Some(&mismatched)).is_err());
}

#[test]
fn text_input_control_rejects_oversized_values() {
    let manifest = manifest();
    let panel = PanelModel {
        title: text("Panel"),
        sections: vec![lumia_plugin_api::PanelSection {
            id: "settings".into(),
            title: None,
            controls: vec![PanelControl::TextInput {
                id: "text".into(),
                label: text("Text"),
                value: "x".repeat(4097),
                enabled: true,
            }],
        }],
    };
    assert!(validate_panel_model(&manifest, &panel).is_err());
}
