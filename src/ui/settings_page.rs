//! The settings window, laid out like flomo: section navigation on the left, grouped rows on the right.

use std::rc::Rc;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, StyledExt as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::{
    Anchor, AnyElement, App, AppContext as _, ClipboardItem, Context, Entity, InteractiveElement as _,
    IntoElement, ParentElement as _, PathPromptOptions, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Task, Window, div,
    prelude::FluentBuilder as _, px,
};

use crate::core::model::{AiConfig, Appearance, BodyFont, LineSpacing, ReviewRange, ReviewScope, Settings};
use crate::core::query::SortOrder;
use crate::core::store::new_id;
use crate::core::{ai, io, time};

use super::library::{EmbeddingStatus, Library};
use super::tasks::run_blocking;
use super::theme::{self, surfaces};
use super::workspace::hash_passcode;
use super::{Command, Lucide, Route, dispatch};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsSection {
    Account,
    Data,
    General,
    Shortcuts,
    Review,
    Ai,
    Open,
    ImportExport,
    Lab,
}

impl SettingsSection {
    const NAV: [Option<SettingsSection>; 11] = [
        Some(SettingsSection::Account),
        Some(SettingsSection::Data),
        None,
        Some(SettingsSection::General),
        Some(SettingsSection::Shortcuts),
        None,
        Some(SettingsSection::Review),
        Some(SettingsSection::Ai),
        Some(SettingsSection::Open),
        Some(SettingsSection::ImportExport),
        Some(SettingsSection::Lab),
    ];

    fn label(self) -> &'static str {
        match self {
            SettingsSection::Account => "账号",
            SettingsSection::Data => "数据与存储",
            SettingsSection::General => "通用",
            SettingsSection::Shortcuts => "快捷键",
            SettingsSection::Review => "每日回顾",
            SettingsSection::Ai => "AI 服务",
            SettingsSection::Open => "MCP / API",
            SettingsSection::ImportExport => "导入导出",
            SettingsSection::Lab => "实验室",
        }
    }

    fn icon(self) -> Lucide {
        match self {
            SettingsSection::Account => Lucide::CircleUser,
            SettingsSection::Data => Lucide::HardDrive,
            SettingsSection::General => Lucide::Settings,
            SettingsSection::Shortcuts => Lucide::Keyboard,
            SettingsSection::Review => Lucide::Sparkle,
            SettingsSection::Ai => Lucide::BrainCircuit,
            SettingsSection::Open => Lucide::Webhook,
            SettingsSection::ImportExport => Lucide::ArrowDownUp,
            SettingsSection::Lab => Lucide::FlaskConical,
        }
    }
}

struct Preset {
    name: &'static str,
    base_url: &'static str,
    chat_model: &'static str,
    embedding_model: &'static str,
}

const PRESETS: [Preset; 7] = [
    Preset { name: "OpenAI", base_url: "https://api.openai.com/v1", chat_model: "gpt-4o-mini", embedding_model: "text-embedding-3-small" },
    Preset { name: "DeepSeek", base_url: "https://api.deepseek.com/v1", chat_model: "deepseek-chat", embedding_model: "" },
    Preset { name: "通义千问", base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1", chat_model: "qwen-plus", embedding_model: "text-embedding-v3" },
    Preset { name: "Kimi", base_url: "https://api.moonshot.cn/v1", chat_model: "moonshot-v1-8k", embedding_model: "" },
    Preset { name: "智谱 GLM", base_url: "https://open.bigmodel.cn/api/paas/v4", chat_model: "glm-4-flash", embedding_model: "embedding-3" },
    Preset { name: "硅基流动", base_url: "https://api.siliconflow.cn/v1", chat_model: "Qwen/Qwen2.5-7B-Instruct", embedding_model: "BAAI/bge-m3" },
    Preset { name: "Ollama 本地", base_url: "http://localhost:11434/v1", chat_model: "qwen2.5:7b", embedding_model: "nomic-embed-text" },
];

const SHORTCUTS: [(&str, &str); 14] = [
    ("新建笔记", "⌘ N"),
    ("快速输入（聚焦输入框）", "⌘ /"),
    ("发布笔记 / 保存编辑", "⌘ ↩"),
    ("编辑笔记", "双击卡片"),
    ("加粗", "⌘ B"),
    ("高亮", "⌘ ⇧ H"),
    ("下划线", "⌘ U"),
    ("搜索", "⌘ K"),
    ("密码锁", "⌘ ⇧ L"),
    ("侧边栏收缩/展开", "⌘ \\"),
    ("回到首页", "⌘ ⇧ F"),
    ("切换深色模式", "⌘ ⇧ D"),
    ("关闭详情 / 覆盖页", "Esc"),
    ("偏好设置", "⌘ ,"),
];

const FONT_SIZES: [f32; 7] = [12., 13., 14., 15., 16., 18., 20.];
const REVIEW_COUNTS: [usize; 6] = [4, 8, 12, 16, 20, 24];

type Action = Rc<dyn Fn(&mut Window, &mut App)>;
type Choice = (String, bool, Action);

fn update_settings(cx: &mut App, f: impl FnOnce(&mut Settings)) {
    let _ = Library::global(cx).update(cx, |lib, cx| lib.update_settings(cx, f));
}

/// A flomo-style value picker: the current value with a chevron, opening a checked menu.
fn picker(id: impl Into<SharedString>, current: String, choices: Vec<Choice>, cx: &App) -> impl IntoElement {
    let muted = cx.theme().muted_foreground;
    let choices = Rc::new(choices);
    Button::new(gpui_kit::ElementId::Name(id.into()))
        .ghost()
        .small()
        .child(
            h_flex()
                .gap_1()
                .text_color(muted)
                .child(current)
                .child(Icon::new(Lucide::ChevronDown).xsmall()),
        )
        .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, _| {
            choices.iter().fold(menu, |menu, (label, checked, action)| {
                let action = action.clone();
                menu.item(PopupMenuItem::new(label.clone()).checked(*checked).on_click(move |_, window, cx| action(window, cx)))
            })
        })
}

fn switch(id: &'static str, checked: bool, f: impl Fn(&mut Settings, bool) + 'static) -> impl IntoElement {
    Switch::new(id).checked(checked).on_change(move |value: &bool, _, cx| {
        let value = *value;
        update_settings(cx, |s| f(s, value));
    })
}

fn group(rows: Vec<AnyElement>, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let s = surfaces(cx);
    let count = rows.len();
    v_flex()
        .rounded_lg()
        .bg(s.card)
        .children(rows.into_iter().enumerate().map(move |(i, row)| {
            div()
                .when(i + 1 < count, |d| d.border_b_1().border_color(theme.border.opacity(0.6)))
                .child(row)
        }))
}

fn row(label: &str, control: impl IntoElement) -> AnyElement {
    h_flex()
        .min_h_12()
        .px_4()
        .py_1p5()
        .gap_4()
        .child(div().flex_1().text_sm().child(label.to_string()))
        .child(control)
        .into_any_element()
}

fn block(content: impl IntoElement) -> AnyElement {
    div().px_4().py_3().child(content).into_any_element()
}

fn caption(text: &str, cx: &App) -> impl IntoElement {
    div().px_1().text_xs().text_color(cx.theme().muted_foreground).child(text.to_string())
}

fn heading(text: &str, cx: &App) -> impl IntoElement {
    div().px_1().pt_2().text_sm().text_color(cx.theme().muted_foreground).child(text.to_string())
}

pub struct SettingsPage {
    section: SettingsSection,
    nickname: Entity<InputState>,
    base_url: Entity<InputState>,
    api_key: Entity<InputState>,
    chat_model: Entity<InputState>,
    embedding_model: Entity<InputState>,
    temperature: Entity<InputState>,
    review_tags: Entity<InputState>,
    test_result: Option<Result<String, String>>,
    testing: bool,
    passcode: Entity<InputState>,
    passcode_confirm: Entity<InputState>,
    api_port: Entity<InputState>,
    wechat_text: Entity<TextareaState>,
    skip_duplicates: bool,
    synced_ai: AiConfig,
    _test_task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl SettingsPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let library = Library::global(cx);
        let settings = library.read(cx).settings().clone();
        let input = |value: String, placeholder: &str, window: &mut Window, cx: &mut Context<Self>| {
            let placeholder = placeholder.to_string();
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder).default_value(value))
        };
        let nickname = input(settings.nickname.clone(), "昵称", window, cx);
        let base_url = input(settings.ai.base_url.clone(), "https://api.openai.com/v1", window, cx);
        let api_key = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder("sk-…（本地模型可留空）")
                .default_value(settings.ai.api_key.clone())
        });
        let chat_model = input(settings.ai.chat_model.clone(), "gpt-4o-mini", window, cx);
        let embedding_model = input(settings.ai.embedding_model.clone(), "可选，留空则使用本地语义引擎", window, cx);
        let temperature = input(format!("{}", settings.ai.temperature), "0.7", window, cx);
        let review_tags = input(settings.review.tags.join(" "), "标签，空格分隔", window, cx);
        let passcode = cx.new(|cx| InputState::new(window, cx).masked(true).placeholder("新密码（至少 4 位）"));
        let passcode_confirm = cx.new(|cx| InputState::new(window, cx).masked(true).placeholder("再次输入"));
        let api_port = input(settings.api_port.to_string(), "17321", window, cx);
        let wechat_text = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(4)
                .placeholder("在微信读书网页版打开书籍 → 笔记 → 复制全部笔记，然后粘贴到这里")
        });
        let save_on_blur = |state: &Entity<InputState>, event: &InputEvent| {
            matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }).then(|| state.clone())
        };
        let subscriptions = vec![
            cx.observe_in(&library, window, |this, library, window, cx| {
                let ai = library.read(cx).settings().ai.clone();
                if ai != this.synced_ai {
                    this.load_ai_inputs(&ai, window, cx);
                }
                cx.notify();
            }),
            cx.subscribe_in(&nickname, window, move |_, state, event, _, cx| {
                if let Some(state) = save_on_blur(state, event) {
                    let name = state.read(cx).value().trim().to_string();
                    if !name.is_empty() {
                        update_settings(cx, |s| s.nickname = name);
                    }
                }
            }),
            cx.subscribe_in(&review_tags, window, move |_, state, event, _, cx| {
                if let Some(state) = save_on_blur(state, event) {
                    let tags: Vec<String> = state
                        .read(cx)
                        .value()
                        .split_whitespace()
                        .map(|t| t.trim_start_matches('#').to_string())
                        .filter(|t| !t.is_empty())
                        .collect();
                    update_settings(cx, |s| s.review.tags = tags);
                }
            }),
        ];
        Self {
            section: SettingsSection::General,
            nickname,
            base_url,
            api_key,
            chat_model,
            embedding_model,
            temperature,
            review_tags,
            test_result: None,
            testing: false,
            passcode,
            passcode_confirm,
            api_port,
            wechat_text,
            skip_duplicates: true,
            synced_ai: settings.ai.clone(),
            _test_task: None,
            _subscriptions: subscriptions,
        }
    }

    pub fn set_section(&mut self, section: SettingsSection, cx: &mut Context<Self>) {
        self.section = section;
        cx.notify();
    }

    fn load_ai_inputs(&mut self, ai: &AiConfig, window: &mut Window, cx: &mut Context<Self>) {
        self.base_url.update(cx, |s, cx| s.set_value(ai.base_url.clone(), window, cx));
        self.api_key.update(cx, |s, cx| s.set_value(ai.api_key.clone(), window, cx));
        self.chat_model.update(cx, |s, cx| s.set_value(ai.chat_model.clone(), window, cx));
        self.embedding_model.update(cx, |s, cx| s.set_value(ai.embedding_model.clone(), window, cx));
        self.temperature.update(cx, |s, cx| s.set_value(format!("{}", ai.temperature), window, cx));
        self.synced_ai = ai.clone();
    }

    fn ai_config(&self, cx: &App) -> AiConfig {
        AiConfig {
            base_url: self.base_url.read(cx).value().trim().to_string(),
            api_key: self.api_key.read(cx).value().trim().to_string(),
            chat_model: self.chat_model.read(cx).value().trim().to_string(),
            embedding_model: self.embedding_model.read(cx).value().trim().to_string(),
            temperature: self.temperature.read(cx).value().trim().parse::<f32>().unwrap_or(0.7).clamp(0.0, 2.0),
        }
    }

    fn save_ai(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let cfg = self.ai_config(cx);
        self.synced_ai = cfg.clone();
        update_settings(cx, |s| s.ai = cfg);
        window.push_notification(Notification::success("AI 服务设置已保存"), cx);
    }

    fn apply_preset(&mut self, preset: &Preset, window: &mut Window, cx: &mut Context<Self>) {
        self.base_url.update(cx, |s, cx| s.set_value(preset.base_url, window, cx));
        self.chat_model.update(cx, |s, cx| s.set_value(preset.chat_model, window, cx));
        self.embedding_model.update(cx, |s, cx| s.set_value(preset.embedding_model, window, cx));
        self.test_result = None;
        cx.notify();
    }

    fn test_ai(&mut self, cx: &mut Context<Self>) {
        let cfg = self.ai_config(cx);
        self.testing = true;
        self.test_result = None;
        cx.notify();
        self._test_task = Some(cx.spawn(async move |this, cx| {
            let result = run_blocking(move || ai::test_connection(&cfg)).await;
            let _ = this.update(cx, |this, cx| {
                this.testing = false;
                this.test_result = Some(match result {
                    Some(Ok(s)) => Ok(s),
                    Some(Err(e)) => Err(e.to_string()),
                    None => Err("测试失败".into()),
                });
                cx.notify();
            });
        }));
    }

    fn set_passcode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let a = self.passcode.read(cx).value().to_string();
        let b = self.passcode_confirm.read(cx).value().to_string();
        if a.chars().count() < 4 {
            window.push_notification(Notification::warning("密码至少需要 4 位"), cx);
            return;
        }
        if a != b {
            window.push_notification(Notification::warning("两次输入的密码不一致"), cx);
            return;
        }
        let salt = new_id();
        let hash = hash_passcode(&a, &salt);
        update_settings(cx, |s| {
            s.lock_enabled = true;
            s.lock_salt = salt;
            s.lock_hash = hash;
        });
        for input in [&self.passcode, &self.passcode_confirm] {
            input.update(cx, |s, cx| s.set_value("", window, cx));
        }
        window.push_notification(Notification::success("密码锁已开启，按 ⌘⇧L 可随时锁定"), cx);
    }

    fn import_files(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let skip = self.skip_duplicates;
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("导入".into()),
        });
        cx.spawn(async move |_, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            cx.update(|cx| {
                let library = Library::global(cx);
                let result = library.update(cx, |lib, cx| lib.mutate(cx, |s| io::import_files(s, &paths, skip)));
                let message = match result {
                    Ok(report) => report.summary(),
                    Err(e) => format!("导入失败：{e}"),
                };
                notify_active(cx, message);
            });
        })
        .detach();
    }

    fn import_wechat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.wechat_text.read(cx).value().to_string();
        let skip = self.skip_duplicates;
        let result = Library::global(cx).update(cx, |lib, cx| lib.mutate(cx, |s| io::import_wechat_read(s, &text, skip)));
        match result {
            Ok(report) => {
                self.wechat_text.update(cx, |s, cx| s.set_value("", window, cx));
                window.push_notification(Notification::success(report.summary()), cx);
            }
            Err(e) => window.push_notification(Notification::warning(e.to_string()), cx),
        }
    }

    fn export(&mut self, kind: &'static str, cx: &mut Context<Self>) {
        let dir = dirs::document_dir().or_else(dirs::home_dir).unwrap_or_default();
        let name = format!("mindflow-{}.{kind}", time::fmt_day(time::today()));
        let path = cx.prompt_for_new_path(&dir, Some(&name));
        cx.spawn(async move |_, cx| {
            let Ok(Ok(Some(path))) = path.await else {
                return;
            };
            cx.update(|cx| {
                let library = Library::global(cx);
                let store = library.read(cx).store();
                let result = match kind {
                    "json" => io::export_json(store, &path),
                    "md" => io::export_markdown(store, &path),
                    _ => io::export_html(store, &path),
                };
                notify_active(
                    cx,
                    match result {
                        Ok(n) => format!("已导出 {n} 条笔记到 {}", path.display()),
                        Err(e) => format!("导出失败：{e}"),
                    },
                );
            });
        })
        .detach();
    }

    fn backup_database(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let source = Library::global(cx).read(cx).store().data_dir().join("mindflow.db");
        let dir = dirs::download_dir().or_else(dirs::home_dir).unwrap_or_default().join("MindFlow");
        let target = dir.join(format!("mindflow-backup-{}.db", chrono::Local::now().format("%Y%m%d-%H%M%S")));
        let result = std::fs::create_dir_all(&dir).and_then(|_| std::fs::copy(&source, &target));
        match result {
            Ok(_) => {
                window.push_notification(Notification::success(format!("已备份到 {}", target.display())), cx);
                cx.reveal_path(&target);
            }
            Err(e) => window.push_notification(Notification::warning(format!("备份失败：{e}")), cx),
        }
    }

    fn mcp_config(&self, cx: &App) -> String {
        let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_else(|_| "mindflow".into());
        let data_dir = Library::global(cx).read(cx).store().data_dir().display().to_string();
        serde_json::to_string_pretty(&serde_json::json!({
            "mcpServers": {
                "mindflow": {
                    "command": exe,
                    "args": ["--mcp"],
                    "env": {"MINDFLOW_DATA_DIR": data_dir}
                }
            }
        }))
        .unwrap_or_default()
    }

    fn render_account(&self, cx: &mut Context<Self>) -> AnyElement {
        let settings = Library::global(cx).read(cx).settings().clone();
        let s = surfaces(cx);
        let theme = cx.theme().clone();
        let initial: String = settings.nickname.chars().next().map(|c| c.to_string()).unwrap_or("我".into());
        let enabled = settings.lock_enabled;
        v_flex()
            .gap_3()
            .child(group(
                vec![
                    block(
                        h_flex()
                            .gap_3()
                            .child(
                                div()
                                    .size_12()
                                    .rounded_full()
                                    .bg(s.accent)
                                    .text_color(theme.primary_foreground)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_lg()
                                    .font_semibold()
                                    .child(initial),
                            )
                            .child(
                                v_flex()
                                    .child(h_flex().gap_1p5().child(div().font_semibold().child(settings.nickname.clone())).child(
                                        div().px_1().rounded(px(3.)).bg(s.accent_soft).text_color(s.accent).text_size(px(10.)).font_bold().child("PRO"),
                                    ))
                                    .child(div().text_xs().text_color(theme.muted_foreground).child("本地版 · 全部功能已解锁 · 数据只保存在这台电脑上")),
                            ),
                    ),
                    row("昵称", div().w(px(220.)).child(Input::new(&self.nickname).small())),
                ],
                cx,
            ))
            .child(heading("密码锁", cx))
            .child(group(
                vec![
                    row(
                        if enabled { "密码锁已开启" } else { "密码锁未开启" },
                        h_flex().gap_2().when(enabled, |this| {
                            this.child(Button::new("lock-disable").outline().small().label("关闭密码锁").on_click(|_, window, cx| {
                                update_settings(cx, |s| {
                                    s.lock_enabled = false;
                                    s.lock_hash.clear();
                                });
                                window.push_notification("已关闭密码锁", cx);
                            }))
                        }),
                    ),
                    row(
                        if enabled { "修改密码" } else { "设置密码" },
                        h_flex()
                            .gap_2()
                            .child(div().w(px(150.)).child(Input::new(&self.passcode).small()))
                            .child(div().w(px(150.)).child(Input::new(&self.passcode_confirm).small()))
                            .child(
                                Button::new("lock-enable")
                                    .primary()
                                    .small()
                                    .label(if enabled { "更新" } else { "开启" })
                                    .on_click(cx.listener(|this, _, window, cx| this.set_passcode(window, cx))),
                            ),
                    ),
                ],
                cx,
            ))
            .child(caption("开启后，每次启动 MindFlow 或按 ⌘⇧L 都需要输入密码。", cx))
            .into_any_element()
    }

    fn render_data(&self, cx: &mut Context<Self>) -> AnyElement {
        let library = Library::global(cx);
        let lib = library.read(cx);
        let data_dir = lib.store().data_dir().to_path_buf();
        let db_size = std::fs::metadata(data_dir.join("mindflow.db")).map(|m| m.len()).unwrap_or(0);
        let (image_count, image_size) = std::fs::read_dir(data_dir.join("images"))
            .map(|entries| {
                entries.flatten().fold((0usize, 0u64), |(n, size), e| {
                    (n + 1, size + e.metadata().map(|m| m.len()).unwrap_or(0))
                })
            })
            .unwrap_or_default();
        let mb = |bytes: u64| format!("{:.1} MB", bytes as f64 / 1_048_576.0);
        let overview = lib.overview().clone();
        let trashed = lib.store().trashed_memos().len();
        let reveal = data_dir.join("mindflow.db");
        v_flex()
            .gap_3()
            .child(group(
                vec![
                    row("笔记", div().text_sm().child(format!("{} 条（回收站 {trashed} 条）", overview.memos))),
                    row("数据库", div().text_sm().child(mb(db_size))),
                    row("图片", div().text_sm().child(format!("{image_count} 张 · {}", mb(image_size)))),
                    row(
                        "存储位置",
                        h_flex()
                            .gap_2()
                            .child(div().max_w(px(260.)).truncate().text_xs().text_color(cx.theme().muted_foreground).child(data_dir.display().to_string()))
                            .child(Button::new("reveal-data").ghost().small().icon(Icon::new(Lucide::FolderOpen)).on_click(move |_, _, cx| cx.reveal_path(&reveal))),
                    ),
                    row(
                        "备份数据库",
                        Button::new("backup-db").outline().small().label("立即备份").on_click(cx.listener(|this, _, window, cx| this.backup_database(window, cx))),
                    ),
                ],
                cx,
            ))
            .child(caption("MindFlow 是本地优先的：笔记、图片、AI 记录都保存在本机，不会上传到任何服务器。调用 AI 服务时只会发送当次需要的笔记内容。", cx))
            .into_any_element()
    }

    fn render_general(&self, cx: &mut Context<Self>) -> AnyElement {
        let settings = Library::global(cx).read(cx).settings().clone();
        let appearance_choices: Vec<Choice> = Appearance::ALL
            .iter()
            .map(|&a| {
                let action: Action = Rc::new(move |window, cx| {
                    update_settings(cx, |s| s.appearance = a);
                    let settings = Library::global(cx).read(cx).settings().clone();
                    theme::apply(&settings, window, cx);
                });
                (a.label().to_string(), a == settings.appearance, action)
            })
            .collect();
        let font_choices: Vec<Choice> = BodyFont::ALL
            .iter()
            .map(|&f| {
                let action: Action = Rc::new(move |_, cx| update_settings(cx, |s| s.body_font = f));
                (f.label().to_string(), f == settings.body_font, action)
            })
            .collect();
        let size_choices: Vec<Choice> = FONT_SIZES
            .iter()
            .map(|&size| {
                let action: Action = Rc::new(move |_, cx| update_settings(cx, |s| s.body_font_size = size));
                (format!("{size}"), size == settings.body_font_size, action)
            })
            .collect();
        let spacing_choices: Vec<Choice> = LineSpacing::ALL
            .iter()
            .map(|&l| {
                let action: Action = Rc::new(move |_, cx| update_settings(cx, |s| s.line_spacing = l));
                (l.label().to_string(), l == settings.line_spacing, action)
            })
            .collect();
        let current_sort = match (settings.sort_by_updated, settings.sort_ascending) {
            (true, true) => SortOrder::UpdatedAsc,
            (true, false) => SortOrder::UpdatedDesc,
            (false, true) => SortOrder::CreatedAsc,
            (false, false) => SortOrder::CreatedDesc,
        };
        let sort_choices: Vec<Choice> = SortOrder::ALL
            .iter()
            .map(|&o| {
                let action: Action = Rc::new(move |_, cx| {
                    update_settings(cx, |s| {
                        s.sort_by_updated = o.by_updated();
                        s.sort_ascending = matches!(o, SortOrder::CreatedAsc | SortOrder::UpdatedAsc);
                    })
                });
                (o.label().to_string(), o == current_sort, action)
            })
            .collect();
        v_flex()
            .gap_3()
            .child(heading("显示偏好", cx))
            .child(group(
                vec![
                    row("外观", picker("pick-appearance", settings.appearance.label().into(), appearance_choices, cx)),
                    row("字体", picker("pick-font", settings.body_font.label().into(), font_choices, cx)),
                    row("正文字体大小", picker("pick-size", format!("{}", settings.body_font_size), size_choices, cx)),
                    row("正文字体行高", picker("pick-spacing", settings.line_spacing.label().into(), spacing_choices, cx)),
                    row("显示热力图", switch("sw-heatmap", settings.show_heatmap, |s, v| s.show_heatmap = v)),
                    row("显示笔记来源", switch("sw-source", settings.show_source, |s, v| s.show_source = v)),
                    row("首页显示 AI Agent 入口", switch("sw-agent", settings.show_agent_button, |s, v| s.show_agent_button = v)),
                ],
                cx,
            ))
            .child(
                v_flex()
                    .gap_0p5()
                    .child(caption("API：通过本地 API 接口或快捷指令自动写入的内容", cx))
                    .child(caption("MCP / AI：通过 MCP 服务或 AI 对话写入的内容", cx)),
            )
            .child(heading("笔记", cx))
            .child(group(
                vec![
                    row("笔记折叠", switch("sw-fold", settings.fold_long_memos, |s, v| s.fold_long_memos = v)),
                    row("显示批注关系", switch("sw-backlinks", settings.show_backlinks, |s, v| s.show_backlinks = v)),
                    row("默认排序", picker("pick-sort", current_sort.label().into(), sort_choices, cx)),
                ],
                cx,
            ))
            .child(caption("笔记折叠：超过 10 行的笔记默认收起，点击「展开」查看全文。", cx))
            .into_any_element()
    }

    fn render_shortcuts(&self, cx: &mut Context<Self>) -> AnyElement {
        let mono = cx.theme().mono_font_family.clone();
        let muted = cx.theme().muted_foreground;
        group(
            SHORTCUTS
                .iter()
                .map(|(desc, key)| row(desc, div().text_sm().font_family(mono.clone()).text_color(muted).child(key.to_string())))
                .collect(),
            cx,
        )
        .into_any_element()
    }

    fn render_review(&self, cx: &mut Context<Self>) -> AnyElement {
        let settings = Library::global(cx).read(cx).settings().clone();
        let cfg = settings.review.clone();
        let scopes = [
            (ReviewScope::All, "全部内容"),
            (ReviewScope::IncludeTags, "包含指定标签"),
            (ReviewScope::ExcludeTags, "排除指定标签"),
            (ReviewScope::Untagged, "无标签"),
        ];
        let scope_label = scopes.iter().find(|(s, _)| *s == cfg.scope).map(|(_, l)| *l).unwrap_or("全部内容");
        let scope_choices: Vec<Choice> = scopes
            .iter()
            .map(|&(scope, label)| {
                let action: Action = Rc::new(move |_, cx| update_settings(cx, |s| s.review.scope = scope));
                (label.to_string(), scope == cfg.scope, action)
            })
            .collect();
        let range_choices: Vec<Choice> = ReviewRange::ALL
            .iter()
            .map(|&r| {
                let action: Action = Rc::new(move |_, cx| update_settings(cx, |s| s.review.range = r));
                (r.label().to_string(), r == cfg.range, action)
            })
            .collect();
        let count_choices: Vec<Choice> = REVIEW_COUNTS
            .iter()
            .map(|&n| {
                let action: Action = Rc::new(move |_, cx| update_settings(cx, |s| s.review.count = n));
                (format!("{n} 条/天"), n == cfg.count, action)
            })
            .collect();
        let needs_tags = matches!(cfg.scope, ReviewScope::IncludeTags | ReviewScope::ExcludeTags);
        let mut rows = vec![row("内容范围", picker("pick-review-scope", scope_label.into(), scope_choices, cx))];
        if needs_tags {
            rows.push(row("标签", div().w(px(240.)).child(Input::new(&self.review_tags).small())));
        }
        rows.push(row("时间范围", picker("pick-review-range", cfg.range.label().into(), range_choices, cx)));
        rows.push(row("回顾数量", picker("pick-review-count", format!("{} 条/天", cfg.count), count_choices, cx)));
        v_flex()
            .gap_3()
            .child(group(rows, cx))
            .child(caption("默认「全部内容 + 全部时间 + 8 条/天」。每天的回顾内容固定，在回顾页点「换一批」可重新抽取。", cx))
            .child(
                h_flex().child(Button::new("goto-review").outline().small().label("打开每日回顾").on_click(|_, window, cx| {
                    window.close_dialog(cx);
                    dispatch(Command::Navigate(Route::Review), window, cx);
                })),
            )
            .into_any_element()
    }

    fn render_ai(&self, cx: &mut Context<Self>) -> AnyElement {
        let library = Library::global(cx);
        let lib = library.read(cx);
        let status = lib.embedding_status().clone();
        let index = lib.index();
        let theme = cx.theme().clone();
        let embedding_text = match &status {
            EmbeddingStatus::Disabled => format!("使用本地语义引擎（离线、免费），已索引 {} 条笔记", index.len()),
            EmbeddingStatus::Idle => format!("远程 Embedding 已同步 {} / {} 条笔记", index.dense_count(), index.len()),
            EmbeddingStatus::Running { done, total } => format!("正在同步 Embedding：{done} / {total}"),
            EmbeddingStatus::Failed(e) => format!("Embedding 同步失败：{e}"),
        };
        let can_resync = !matches!(status, EmbeddingStatus::Disabled | EmbeddingStatus::Running { .. });
        v_flex()
            .gap_3()
            .child(caption("兼容 OpenAI 接口的任意服务。API Key 只保存在本机。", cx))
            .child(h_flex().flex_wrap().gap_1p5().children(PRESETS.iter().enumerate().map(|(i, p)| {
                Button::new(("preset", i))
                    .outline()
                    .xsmall()
                    .label(p.name)
                    .on_click(cx.listener(move |this, _, window, cx| this.apply_preset(&PRESETS[i], window, cx)))
            })))
            .child(group(
                vec![
                    row("接口地址", div().w(px(320.)).child(Input::new(&self.base_url).small())),
                    row("API Key", div().w(px(320.)).child(Input::new(&self.api_key).small().mask_toggle())),
                    row("对话模型", div().w(px(320.)).child(Input::new(&self.chat_model).small())),
                    row("Embedding 模型", div().w(px(320.)).child(Input::new(&self.embedding_model).small())),
                    row("温度", div().w(px(100.)).child(Input::new(&self.temperature).small())),
                ],
                cx,
            ))
            .child(
                h_flex()
                    .gap_2()
                    .child(Button::new("ai-save").primary().small().label("保存").on_click(cx.listener(|this, _, window, cx| this.save_ai(window, cx))))
                    .child(Button::new("ai-test").outline().small().label("测试连接").loading(self.testing).on_click(cx.listener(|this, _, _, cx| this.test_ai(cx))))
                    .when_some(self.test_result.clone(), |this, result| {
                        this.child(
                            div()
                                .flex_1()
                                .text_xs()
                                .text_color(if result.is_ok() { theme.success } else { theme.danger })
                                .child(match result {
                                    Ok(s) => s,
                                    Err(e) => e,
                                }),
                        )
                    }),
            )
            .child(group(
                vec![
                    row(
                        &embedding_text,
                        h_flex().when(can_resync, |this| {
                            this.child(Button::new("embedding-resync").ghost().small().label("重新同步").on_click(|_, _, cx| {
                                Library::global(cx).update(cx, |lib, cx| lib.resync_embeddings(cx));
                            }))
                        }),
                    ),
                    row(
                        "AI 记忆档案",
                        Button::new("open-memory").ghost().small().label("打开").on_click(|_, window, cx| {
                            window.close_dialog(cx);
                            dispatch(Command::Navigate(Route::Memory), window, cx);
                        }),
                    ),
                ],
                cx,
            ))
            .into_any_element()
    }

    fn render_open(&self, cx: &mut Context<Self>) -> AnyElement {
        let library = Library::global(cx);
        let lib = library.read(cx);
        let settings = lib.settings().clone();
        let running = lib.api_running();
        let api_error = lib.api_error().map(|s| s.to_string());
        let mcp = self.mcp_config(cx);
        let mcp_copy = mcp.clone();
        let theme = cx.theme().clone();
        let curl = format!(
            "curl -X POST http://127.0.0.1:{}/api/v1/memos \\\n  -H 'Authorization: Bearer {}' \\\n  -H 'Content-Type: application/json' \\\n  -d '{{\"content\": \"来自 API 的笔记 #收集箱\"}}'",
            settings.api_port, settings.api_token
        );
        let curl_copy = curl.clone();
        let code = |text: String| {
            div()
                .p_3()
                .rounded_md()
                .bg(theme.muted)
                .text_xs()
                .font_family(theme.mono_font_family.clone())
                .child(text)
        };
        v_flex()
            .gap_3()
            .child(heading("MCP 服务", cx))
            .child(caption("让 Claude、Cursor、Cherry Studio 等 AI 工具搜索、读取、创建笔记并读取记忆档案（共 10 个工具）。", cx))
            .child(code(mcp))
            .child(h_flex().child(Button::new("copy-mcp").outline().small().icon(Icon::new(Lucide::Copy)).label("复制 MCP 配置").on_click(move |_, window, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(mcp_copy.clone()));
                window.push_notification("已复制，粘贴到 AI 工具的 MCP 配置中即可", cx);
            })))
            .child(heading("本地 API", cx))
            .child(group(
                vec![
                    row(
                        "启用记录 API",
                        h_flex()
                            .gap_2()
                            .child(div().text_xs().text_color(if running { theme.success } else { theme.muted_foreground }).child(if running { format!("运行中 · 127.0.0.1:{}", settings.api_port) } else { "未启用".into() }))
                            .child(Switch::new("api-enabled").checked(settings.api_enabled).on_change(cx.listener(|this, checked: &bool, _, cx| {
                                let enabled = *checked;
                                let port = this.api_port.read(cx).value().trim().parse::<u16>().unwrap_or(17321);
                                update_settings(cx, |s| {
                                    s.api_enabled = enabled;
                                    s.api_port = port;
                                    if s.api_token.is_empty() {
                                        s.api_token = new_id();
                                    }
                                });
                            }))),
                    ),
                    row(
                        "端口",
                        h_flex()
                            .gap_2()
                            .child(div().w(px(100.)).child(Input::new(&self.api_port).small()))
                            .child(Button::new("api-apply-port").ghost().small().label("应用").on_click(cx.listener(|this, _, _, cx| {
                                let port = this.api_port.read(cx).value().trim().parse::<u16>().unwrap_or(17321);
                                update_settings(cx, |s| s.api_port = port);
                            }))),
                    ),
                    row(
                        "令牌",
                        h_flex()
                            .gap_2()
                            .child(div().text_xs().font_family(theme.mono_font_family.clone()).text_color(theme.muted_foreground).child(if settings.api_token.is_empty() { "（启用时自动生成）".into() } else { settings.api_token.clone() }))
                            .child(Button::new("api-regen-token").ghost().small().icon(Icon::new(Lucide::KeyRound)).tooltip("重新生成").on_click(|_, _, cx| update_settings(cx, |s| s.api_token = new_id()))),
                    ),
                ],
                cx,
            ))
            .when_some(api_error, |this, e| this.child(div().text_xs().text_color(theme.danger).child(e)))
            .when(settings.api_enabled, |this| {
                this.child(code(curl)).child(h_flex().child(Button::new("copy-curl").outline().small().icon(Icon::new(Lucide::Copy)).label("复制示例").on_click(move |_, window, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(curl_copy.clone()));
                    window.push_notification("已复制", cx);
                })))
            })
            .into_any_element()
    }

    fn render_import_export(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .gap_3()
            .child(heading("导入", cx))
            .child(group(
                vec![
                    row(
                        "从文件导入",
                        h_flex()
                            .gap_3()
                            .child(Checkbox::new("skip-duplicates").label("跳过重复").checked(self.skip_duplicates).on_change(cx.listener(|this, checked: &bool, _, cx| {
                                this.skip_duplicates = *checked;
                                cx.notify();
                            })))
                            .child(Button::new("import-files").outline().small().icon(Icon::new(Lucide::FileUp)).label("选择文件").on_click(cx.listener(|this, _, window, cx| this.import_files(window, cx)))),
                    ),
                    block(
                        v_flex()
                            .gap_2()
                            .child(div().text_sm().child("微信读书"))
                            .child(Textarea::new(&self.wechat_text).small())
                            .child(h_flex().justify_end().child(Button::new("import-wechat").outline().small().icon(Icon::new(Lucide::BookOpen)).label("导入微信读书笔记").on_click(cx.listener(|this, _, window, cx| this.import_wechat(window, cx))))),
                    ),
                ],
                cx,
            ))
            .child(caption("支持 flomo 导出的 HTML（含图片，在 flomo 网页版「设置 → 导出全部数据」，解压后选择 index.html）、MindFlow JSON、Markdown / TXT、CSV（Day One 等）。", cx))
            .child(heading("导出", cx))
            .child(group(
                vec![
                    row("JSON（完整备份，可再次导入）", Button::new("export-json").outline().small().label("导出").on_click(cx.listener(|this, _, _, cx| this.export("json", cx)))),
                    row("Markdown", Button::new("export-md").outline().small().label("导出").on_click(cx.listener(|this, _, _, cx| this.export("md", cx)))),
                    row("HTML（与 flomo 导出格式相近）", Button::new("export-html").outline().small().label("导出").on_click(cx.listener(|this, _, _, cx| this.export("html", cx)))),
                ],
                cx,
            ))
            .into_any_element()
    }

    fn render_lab(&self, cx: &mut Context<Self>) -> AnyElement {
        let entry = |id: &'static str, label: &str, desc: &str, route: Route| {
            row(
                &format!("{label} · {desc}"),
                Button::new(id).ghost().small().label("打开").on_click(move |_, window, cx| {
                    window.close_dialog(cx);
                    dispatch(Command::Navigate(route), window, cx);
                }),
            )
        };
        v_flex()
            .gap_3()
            .child(group(
                vec![
                    entry("lab-map", "认知地图", "笔记涌现出的思考山峰", Route::Map),
                    entry("lab-walk", "随机漫步", "沿着语义小径游走", Route::Walk),
                    entry("lab-graph", "笔记关系图", "批注与语义连接", Route::Graph),
                    entry("lab-stats", "记录统计", "热力图、分布与排行", Route::Stats),
                    entry("lab-memory", "AI 记忆档案", "让 AI 真正了解你", Route::Memory),
                ],
                cx,
            ))
            .child(caption("实验室里的功能仍在打磨中，欢迎多用多反馈。", cx))
            .into_any_element()
    }
}

fn notify_active(cx: &mut App, message: String) {
    if let Some(window) = cx.active_window() {
        let _ = window.update(cx, |_, window, cx| window.push_notification(message, cx));
    }
}

impl Render for SettingsPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let s = surfaces(cx);
        let section = self.section;
        let body = match section {
            SettingsSection::Account => self.render_account(cx),
            SettingsSection::Data => self.render_data(cx),
            SettingsSection::General => self.render_general(cx),
            SettingsSection::Shortcuts => self.render_shortcuts(cx),
            SettingsSection::Review => self.render_review(cx),
            SettingsSection::Ai => self.render_ai(cx),
            SettingsSection::Open => self.render_open(cx),
            SettingsSection::ImportExport => self.render_import_export(cx),
            SettingsSection::Lab => self.render_lab(cx),
        };
        h_flex()
            .size_full()
            .items_stretch()
            .child(
                v_flex()
                    .w(px(196.))
                    .flex_shrink_0()
                    .p_3()
                    .gap_0p5()
                    .bg(s.sidebar)
                    .rounded_l(theme.radius_lg)
                    .child(div().px_3().pt_2().pb_3().text_sm().text_color(theme.muted_foreground).child("设置"))
                    .children(SettingsSection::NAV.iter().enumerate().map(|(i, item)| match item {
                        None => div().h_3().into_any_element(),
                        Some(target) => {
                            let target = *target;
                            let active = target == section;
                            h_flex()
                                .id(("settings-nav", i))
                                .gap_2p5()
                                .px_3()
                                .h_9()
                                .rounded_md()
                                .cursor_pointer()
                                .text_sm()
                                .when(active, |d| d.bg(s.accent).text_color(theme.primary_foreground))
                                .when(!active, |d| d.hover(|d| d.bg(theme.sidebar_accent)))
                                .child(Icon::new(target.icon()).small())
                                .child(target.label())
                                .on_click(cx.listener(move |this, _, _, cx| this.set_section(target, cx)))
                                .into_any_element()
                        }
                    }))
                    .child(div().flex_1())
                    .child(
                        h_flex()
                            .gap_2()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .border_1()
                            .border_color(theme.border)
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(Icon::new(Lucide::Crown).xsmall().text_color(theme.yellow))
                            .child(format!("MindFlow {} · 本地版", env!("CARGO_PKG_VERSION"))),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().px_6().pt_5().pb_3().text_lg().font_semibold().child(section.label()))
                    .child(
                        v_flex()
                            .id("settings-body")
                            .flex_1()
                            .min_h_0()
                            .px_6()
                            .pb_6()
                            .overflow_y_scrollbar()
                            .child(body),
                    ),
            )
    }
}
