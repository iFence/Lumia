//! 标注面板的 UI 模型构建：分区、控件、工具选择器与设置项。

use std::collections::BTreeMap;

use lumia_plugin_api::{
    LocalizedText, PanelControl, PanelModel, PanelOption, PanelSection, PluginIcon,
};

use crate::{tool_name, AnnotationState, Tool};

/// 中英双语标签。
pub fn text(english: &str, chinese: &str) -> LocalizedText {
    LocalizedText {
        fallback: english.to_string(),
        translations: BTreeMap::from([("zh-CN".to_string(), chinese.to_string())]),
    }
}

fn color_control(state: &AnnotationState) -> PanelControl {
    PanelControl::Color {
        id: "color".to_string(),
        label: text("Color", "颜色"),
        value: state.color.clone(),
        enabled: true,
    }
}
fn opacity_control(state: &AnnotationState) -> PanelControl {
    PanelControl::Slider {
        id: "opacity".to_string(),
        label: text("Opacity", "透明度"),
        value: state.opacity,
        min: 0.1,
        max: 1.0,
        step: 0.1,
        enabled: true,
    }
}
fn settings_controls(state: &AnnotationState) -> Vec<PanelControl> {
    match state.tool {
        Tool::Text => vec![
            PanelControl::TextInput {
                id: "text".to_string(),
                label: text("Annotation text", "标注文字"),
                value: String::new(),
                enabled: true,
            },
            PanelControl::Slider {
                id: "font_size".to_string(),
                label: text("Font size", "字号"),
                value: state.font_size,
                min: 8.0,
                max: 256.0,
                step: 2.0,
                enabled: true,
            },
            color_control(state),
            opacity_control(state),
        ],
        Tool::Rectangle => vec![
            PanelControl::Slider {
                id: "stroke_width".to_string(),
                label: text("Stroke width", "线宽"),
                value: state.stroke_width,
                min: 1.0,
                max: 64.0,
                step: 1.0,
                enabled: true,
            },
            color_control(state),
            opacity_control(state),
        ],
        Tool::NumberedStep => vec![
            PanelControl::Slider {
                id: "badge_size".to_string(),
                label: text("Badge size", "徽标大小"),
                value: state.badge_size,
                min: 12.0,
                max: 96.0,
                step: 2.0,
                enabled: true,
            },
            color_control(state),
            opacity_control(state),
        ],
    }
}

pub fn panel_model(state: &AnnotationState) -> PanelModel {
    PanelModel {
        title: text("Annotation", "标注"),
        sections: vec![
            PanelSection {
                id: "tools".to_string(),
                title: None,
                controls: vec![PanelControl::Select {
                    id: "tool".to_string(),
                    label: text("Tool", "工具"),
                    options: vec![
                        PanelOption {
                            value: "text".to_string(),
                            label: text("Text", "文字"),
                            icon: Some(PluginIcon::Text),
                        },
                        PanelOption {
                            value: "rectangle".to_string(),
                            label: text("Rectangle", "矩形框"),
                            icon: Some(PluginIcon::Rectangle),
                        },
                        PanelOption {
                            value: "numbered_step".to_string(),
                            label: text("Numbered step", "数字步骤"),
                            icon: Some(PluginIcon::NumberedStep),
                        },
                    ],
                    selected: tool_name(state.tool).to_string(),
                    enabled: true,
                }],
            },
            PanelSection {
                id: "tool_settings".to_string(),
                title: Some(text("Settings", "设置")),
                controls: settings_controls(state),
            },
            PanelSection {
                id: "history".to_string(),
                title: None,
                controls: vec![
                    PanelControl::Button {
                        id: "undo".to_string(),
                        label: text("Undo", "撤销"),
                        icon: PluginIcon::Undo,
                        enabled: true,
                    },
                    PanelControl::Button {
                        id: "redo".to_string(),
                        label: text("Redo", "重做"),
                        icon: PluginIcon::Redo,
                        enabled: true,
                    },
                    PanelControl::Button {
                        id: "clear".to_string(),
                        label: text("Clear", "清空"),
                        icon: PluginIcon::Annotation,
                        enabled: true,
                    },
                    PanelControl::Button {
                        id: "export".to_string(),
                        label: text("Export copy", "导出副本"),
                        icon: PluginIcon::Export,
                        enabled: true,
                    },
                ],
            },
        ],
    }
}
