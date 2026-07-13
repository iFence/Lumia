use std::{mem, ptr};

use windows_sys::Win32::{
    Globalization::GetUserDefaultUILanguage,
    UI::Controls::{
        TaskDialogIndirect, TASKDIALOGCONFIG, TASKDIALOGCONFIG_0, TASKDIALOGCONFIG_1,
        TASKDIALOG_BUTTON, TDCBF_CANCEL_BUTTON, TDCBF_OK_BUTTON, TDF_ALLOW_DIALOG_CANCELLATION,
        TDF_SIZE_TO_CONTENT, TDF_USE_COMMAND_LINKS_NO_ICON, TD_ERROR_ICON, TD_INFORMATION_ICON,
        TD_WARNING_ICON,
    },
    UI::WindowsAndMessaging::{IDCANCEL, IDOK},
};

const CHINESE_BUTTON: i32 = 1001;
const ENGLISH_BUTTON: i32 = 1002;
const CONTINUE_BUTTON: i32 = 1003;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Language {
    English,
    Chinese,
}

impl Language {
    pub(crate) fn msi_resource(self) -> u16 {
        match self {
            Self::English => 101,
            Self::Chinese => 102,
        }
    }

    pub(crate) fn msi_file_name(self) -> &'static str {
        match self {
            Self::English => "Lumia-en-US.msi",
            Self::Chinese => "Lumia-zh-CN.msi",
        }
    }
}

pub(crate) fn choose_language() -> Option<Language> {
    let default = system_language();
    let title = wide("Lumia Setup / Lumia 安装程序");
    let instruction = wide("Choose a language / 选择语言");
    let content = wide("Select the language used by Setup.\n请选择安装程序使用的语言。");
    let chinese = wide("简体中文");
    let english = wide("English");
    let buttons = [
        TASKDIALOG_BUTTON {
            nButtonID: CHINESE_BUTTON,
            pszButtonText: chinese.as_ptr(),
        },
        TASKDIALOG_BUTTON {
            nButtonID: ENGLISH_BUTTON,
            pszButtonText: english.as_ptr(),
        },
    ];
    let default_button = match default {
        Language::Chinese => CHINESE_BUTTON,
        Language::English => ENGLISH_BUTTON,
    };
    match show_dialog(
        &title,
        &instruction,
        &content,
        &buttons,
        default_button,
        TDCBF_CANCEL_BUTTON,
        TDF_USE_COMMAND_LINKS_NO_ICON,
        TD_INFORMATION_ICON,
    ) {
        CHINESE_BUTTON => Some(Language::Chinese),
        ENGLISH_BUTTON => Some(Language::English),
        _ => None,
    }
}

pub(crate) fn confirm_legacy_migration(language: Language) -> bool {
    let (instruction, content, continue_text) = match language {
        Language::English => (
            "An older system-wide Lumia installation was found",
            "Setup must remove the copy under Program Files before installing Lumia for your account. Your settings and file-association choices will be kept.",
            "Remove the old version and continue",
        ),
        Language::Chinese => (
            "发现旧的全局 Lumia 安装",
            "安装程序需要先移除 Program Files 下的旧版本，再为当前账户安装 Lumia。您的设置和文件关联选择会保留。",
            "移除旧版本并继续",
        ),
    };
    let title = wide("Lumia Setup");
    let instruction = wide(instruction);
    let content = wide(content);
    let continue_text = wide(continue_text);
    let buttons = [TASKDIALOG_BUTTON {
        nButtonID: CONTINUE_BUTTON,
        pszButtonText: continue_text.as_ptr(),
    }];
    show_dialog(
        &title,
        &instruction,
        &content,
        &buttons,
        CONTINUE_BUTTON,
        TDCBF_CANCEL_BUTTON,
        TDF_USE_COMMAND_LINKS_NO_ICON,
        TD_WARNING_ICON,
    ) == CONTINUE_BUTTON
}

pub(crate) fn show_error(language: Language, error: &anyhow::Error) {
    let (instruction, prefix) = match language {
        Language::English => ("Lumia could not be installed", "Setup stopped because:"),
        Language::Chinese => ("无法安装 Lumia", "安装程序已停止，原因："),
    };
    let title = wide("Lumia Setup");
    let instruction = wide(instruction);
    let content = wide(&format!("{prefix}\n\n{error:#}"));
    show_dialog(
        &title,
        &instruction,
        &content,
        &[],
        IDOK,
        TDCBF_OK_BUTTON,
        0,
        TD_ERROR_ICON,
    );
}

pub(crate) fn show_repair_warning(language: Language, error: &anyhow::Error) {
    let (instruction, prefix) = match language {
        Language::English => (
            "Lumia was installed, but file associations could not be refreshed",
            "You can repair them later from Lumia Settings.",
        ),
        Language::Chinese => (
            "Lumia 已安装，但无法刷新文件关联",
            "您可以稍后在 Lumia 设置中修复文件关联。",
        ),
    };
    let title = wide("Lumia Setup");
    let instruction = wide(instruction);
    let content = wide(&format!("{prefix}\n\n{error:#}"));
    show_dialog(
        &title,
        &instruction,
        &content,
        &[],
        IDOK,
        TDCBF_OK_BUTTON,
        0,
        TD_WARNING_ICON,
    );
}

fn system_language() -> Language {
    let language_id = unsafe { GetUserDefaultUILanguage() };
    if language_id & 0x03ff == 0x0004 {
        Language::Chinese
    } else {
        Language::English
    }
}

#[allow(clippy::too_many_arguments)]
fn show_dialog(
    title: &[u16],
    instruction: &[u16],
    content: &[u16],
    buttons: &[TASKDIALOG_BUTTON],
    default_button: i32,
    common_buttons: i32,
    flags: i32,
    icon: *const u16,
) -> i32 {
    let config = TASKDIALOGCONFIG {
        cbSize: mem::size_of::<TASKDIALOGCONFIG>() as u32,
        hwndParent: ptr::null_mut(),
        hInstance: ptr::null_mut(),
        dwFlags: TDF_ALLOW_DIALOG_CANCELLATION | TDF_SIZE_TO_CONTENT | flags,
        dwCommonButtons: common_buttons,
        pszWindowTitle: title.as_ptr(),
        Anonymous1: TASKDIALOGCONFIG_0 { pszMainIcon: icon },
        pszMainInstruction: instruction.as_ptr(),
        pszContent: content.as_ptr(),
        cButtons: buttons.len() as u32,
        pButtons: buttons.as_ptr(),
        nDefaultButton: default_button,
        cRadioButtons: 0,
        pRadioButtons: ptr::null(),
        nDefaultRadioButton: 0,
        pszVerificationText: ptr::null(),
        pszExpandedInformation: ptr::null(),
        pszExpandedControlText: ptr::null(),
        pszCollapsedControlText: ptr::null(),
        Anonymous2: TASKDIALOGCONFIG_1 {
            pszFooterIcon: ptr::null(),
        },
        pszFooter: ptr::null(),
        pfCallback: None,
        lpCallbackData: 0,
        cxWidth: 0,
    };
    let mut selected = IDCANCEL;
    let result =
        unsafe { TaskDialogIndirect(&config, &mut selected, ptr::null_mut(), ptr::null_mut()) };
    if result == 0 {
        selected
    } else {
        IDCANCEL
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
