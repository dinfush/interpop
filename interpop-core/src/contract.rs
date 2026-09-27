//! # L0 契约层定义
//!
//! 本模块定义了 interpop 微内核图形伺服器的 L0 契约层类型（IF-BluePrint 第 3 章）。
//!
//! ## 契约法典与同构铁律
//! - **类型即协议**：本模块的所有类型通过 `serde` 直接序列化与反序列化为 wire 格式（NDJSON）。
//! - **参数透传同构铁律（白皮书 §2）**：测试驱动（scaffold）与真实网络 IPC 共用同一组强类型契约，
//!   接入真实 Socket 时核心渲染逻辑变更率为 0.00%。
//! - **绝对快照铁律（白皮书 §12.3、台账#20）**：`PatchPayload` 排除一切增量 Delta 与动态宽类型后门，
//!   确保脏表覆盖与丢弃操作的幂等安全性。
//! - **Wire 序列化收敛**：所有 `Option<T>` 字段统一标注 `default` 与 `skip_serializing_if = "Option::is_none"`，
//!   确保 `None` 值不在 wire 输出键名，与 golden 样本保持逐字节完全同构。

use serde::{Deserialize, Serialize};

// ============================================================================
// L0-5 共享词汇类型与别名定义 (IF-BluePrint L0-5)
// ============================================================================

/// 画布唯一物理标识符（内部自增 ID）。
pub type CanvasId = u64;

/// 视口逻辑目标标识（由外部客户端声明的语义标识，如 "vol-island"）。
pub type TargetId = String;

/// 交互租约唯一标识符（由外部客户端或原子拖拽交互申请）。
pub type LeaseId = String;

/// 客户端连接唯一标识符（IPC 连接的文件描述符/通道抽象）。
pub type ConnId = u32;

/// 输出显示器标识（由合成器通告的物理输出名称或 ID）。
pub type OutputId = String;

/// 矩形区域描述（逻辑像素域，白皮书 §10、SOP-P1 PR-05 R3 换算前提）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    /// 矩形左上角 X 轴逻辑坐标。
    pub x: i32,
    /// 矩形左上角 Y 轴逻辑坐标。
    pub y: i32,
    /// 矩形宽度（逻辑像素）。
    pub w: u32,
    /// 矩形高度（逻辑像素）。
    pub h: u32,
}

/// 四大正交销毁策略（白皮书 §6.4、IF-BluePrint L0-5）。
///
/// 外部标记（默认 serde 规则）：无载荷变体序列化为字符串（如 `"manual"`），
/// 带载荷变体序列化为对象（如 `{"timeout":{"ms":5000}}`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DismissPolicy {
    /// 仅显式下发 dismiss 或内部冒泡意图响应时销毁（缺省）。
    #[default]
    Manual,
    /// 空间指针在画布 AABB 包围盒外发生有效点击时销毁。
    ClickOutside,
    /// 视口激活启动硬件定时器，超时启动双阶段离场（具备交互驻留保护）。
    Timeout {
        /// 超时毫秒数。
        ms: u32,
    },
    /// 主视口失去键盘输入焦点时销毁。
    OnBlur,
}


/// 业务交互动作意图（白皮书 §14.1、IF-BluePrint L0-5）。
///
/// 归一化的用户交互动作，反向回写至客户端进行业务裁决。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentAction {
    /// 单击或激活触发。
    Activate,
    /// 持续拖拽（携带当前标准化或绝对数值）。
    Drag {
        /// 拖拽位置或数值。
        value: f64,
    },
    /// 拖拽释放（携带最终确认数值）。
    Release {
        /// 释放时的数值。
        value: f64,
    },
    /// 输入提交（文本框回车或确认）。
    Submit {
        /// 提交的文本内容。
        text: String,
    },
}

// ============================================================================
// L0-1 顶层动词 Request (IF-BluePrint L0-1)
// ============================================================================

/// 顶层上行请求动词契约（白皮书 §14.1、IF-BluePrint L0-1）。
///
/// 封闭收敛为 5 项顶层动词。未知字段容忍（不启用 `deny_unknown_fields`）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "verb", rename_all = "snake_case")]
pub enum Request {
    /// 握手检活：内核立即回发 `PONG` 事件，用于防双开与残留自愈。
    Ping,
    /// 申请物理地皮并初次排版上屏。
    Spawn(SpawnRequest),
    /// 高频局部差分更新（直达单槽脏表）。
    Patch(PatchRequest),
    /// 主动触发对应画布启动双阶段平滑离场。
    Dismiss {
        /// 待离场视口的目标标识。
        target: TargetId,
    },
    /// 整树下发设计令牌，广播热重载换肤。
    Restyle(RestyleRequest),
}

// ============================================================================
// L0-2 SpawnRequest 与形态/原子语法树 (IF-BluePrint L0-2)
// ============================================================================

/// 地皮申请与视口创建载荷（白皮书 §14.1、IF-BluePrint L0-2）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpawnRequest {
    /// 8 大固化形态之一。
    pub form: Form,
    /// 视口全局唯一目标标识。
    pub target: TargetId,
    /// 销毁策略（缺省为 `Manual`）。
    #[serde(default)]
    pub dismiss_policy: DismissPolicy,
    /// DSL 声明式原子树根（18 变体全集）。
    pub atoms: AtomSpec,
    /// 目标物理输出屏（可选，缺省由多屏仲裁函数决议）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<OutputId>,
    /// 同 UID 抢管逃生门（专治调试期 SIGSTOP 挂死）。
    #[serde(default)]
    pub force: bool,
    /// 心跳清退超时毫秒数（可选）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heartbeat_ms: Option<u32>,
    /// 宿主控件吸附全局矩形（仅 `form: popup` 有效，触发 Clamp & Flip）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_rect: Option<Rect>,
}

/// 8 大固化物理形态全景（白皮书 §6.1、IF-BluePrint L0-2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Form {
    /// 贴附屏幕四边单侧，工作区挤占或浮动（状态栏、Dock 坞）。
    Panel,
    /// 全高贴边，零挤占覆盖（控制中心、侧滑通知）。
    Drawer,
    /// 弹出视口（吸附或自由形态，音量岛、上下文菜单）。
    Popup,
    /// 全局置顶且鼠标绝对穿透（亮度 OSD、系统 Toast）。
    Overlay,
    /// 屏幕居中且独占键盘焦点（全局搜索框）。
    Spotlight,
    /// 桌面底层仪表部件（沉于常规窗口下方）。
    Widget,
    /// 全分辨率置底动态桌面。
    Wallpaper,
    /// 标准 xdg_toplevel 窗口，由宿主窗口管理器调度。
    Window,
}

/// 声明式原子规格语法树（白皮书 §8.3、§16、台账#20/F1）。
///
/// 收敛为 18 变体：17 项标准业务基元 + 1 项 Phase 1 纯色探路原子 `fill`。
/// ⛔ 严禁在此出现 `badge`/`chip`/`session_tile`（它们仅存在于 DSL 宏展开层）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AtomSpec {
    // ---------------- P0 核心基石 ----------------
    /// Flexbox 容器解算原子。
    Box {
        /// 子原子列表。
        #[serde(default)]
        children: Vec<AtomSpec>,
    },
    /// 多语言文本排版原子。
    Text {
        /// 文本内容。
        content: String,
    },
    /// 点击意图与波纹交互原子。
    Button {
        /// 标识符。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// 按钮内嵌原子。
        child: Box<AtomSpec>,
    },
    /// 受控滑块原子（支持外部声明 min/max/step）。
    Slider {
        /// 标识符。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// 当前标称值。
        value: f64,
        /// 最小值（外部声明）。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<f64>,
        /// 最大值（外部声明）。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<f64>,
        /// 步进（外部声明）。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        step: Option<f64>,
    },
    /// 布尔弹簧开关原子。
    Switch {
        /// 标识符。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// 开关当前布尔状态。
        checked: bool,
    },
    /// 平滑进度条原子。
    Progress {
        /// 标识符。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// 进度比率（0.0 ~ 1.0）。
        value: f64,
    },
    /// 内存位图或矢量渲染原子。
    Image {
        /// 资源路径或标识。
        src: String,
    },

    // ---------------- P1 进阶体系 ----------------
    /// 二维网格排布原子。
    Grid {
        /// 子原子列表。
        #[serde(default)]
        children: Vec<AtomSpec>,
    },
    /// Z 轴层叠绝对覆盖原子。
    Stack {
        /// 子原子列表。
        #[serde(default)]
        children: Vec<AtomSpec>,
    },
    /// 虚拟化可见列表原子。
    List {
        /// 标识符。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },
    /// 文本输入框原子（广播物理光标矩形对齐系统 IME）。
    Input {
        /// 标识符。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// 当前占位提示文本。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        placeholder: Option<String>,
    },
    /// 阻尼滚动视口原子。
    Scroll {
        /// 内部子原子。
        child: Box<AtomSpec>,
    },

    // ---------------- P2 低频可选 ----------------
    /// 2D 矢量原语路径流原子（声浪、波形）。
    Path {
        /// 矢量路径数据。
        data: String,
    },
    /// wl_subsurface 外嵌容器原子。
    Surface {
        /// 宿主槽位标识。
        slot: String,
    },
    /// 互斥单选组原子。
    Radio {
        /// 标识符。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// 当前选中状态。
        selected: bool,
    },
    /// 多选布尔复选框原子。
    Checkbox {
        /// 标识符。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// 当前选中状态。
        checked: bool,
    },
    /// SNI 系统托盘挂载原子（Feature 门控）。
    Tray {
        /// 托盘服务总线名称或标识。
        service: String,
    },

    // ---------------- Phase 1 探路原子 ----------------
    /// 纯色背景探路原子（Phase 1 专属渲染基准）。
    Fill {
        /// 纯色十六进制或 CSS 颜色定义（如 "#204060"）。
        color: String,
    },
}

// ============================================================================
// L0-3 PatchRequest 与绝对快照载荷 (IF-BluePrint L0-3)
// ============================================================================

/// 标识目标原子内部组件的标识符。
pub type AtomId = String;

/// 高频局部差分更新请求（白皮书 §12.3、IF-BluePrint L0-3）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatchRequest {
    /// 目标视口标识。
    pub target: TargetId,
    /// 待修补的目标原子标识。
    pub atom_id: AtomId,
    /// 微秒单调时间戳（必须为 CLOCK_MONOTONIC 整数）。
    pub timestamp: u64,
    /// 幂等绝对值快照载荷。
    pub payload: PatchPayload,
}

/// 幂等绝对值快照强类型载荷全集（白皮书 §12.3 绑定契约、台账#20/F2）。
///
/// 彻底封死 `Raw(Value)` 任意类型后门，确保脏表原位覆盖与最新值胜出（Latest-Wins）的数学安全性。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchPayload {
    /// 滑块绝对值快照。
    Slider {
        /// 滑块当前绝对数值。
        value: f64,
    },
    /// 开关布尔绝对快照。
    Switch {
        /// 开关当前绝对状态。
        checked: bool,
    },
    /// 进度条绝对进度快照。
    Progress {
        /// 进度条绝对数值。
        value: f64,
    },
    /// 文本内容全量替换快照。
    Text {
        /// 替换后的新文本内容。
        content: String,
    },
    /// 纯色探路原子绝对色彩快照。
    Fill {
        /// 绝对颜色字符串。
        color: String,
    },
}

/// 设计令牌字典广播请求（白皮书 §14.1、IF-BluePrint L0-1）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RestyleRequest {
    /// 主题标识（如 "dark", "light"）。
    pub theme: String,
    /// 设计令牌键值映射（如 "bg_primary" -> "#1c1c1c"）。
    pub tokens: std::collections::BTreeMap<String, String>,
}

// ============================================================================
// L0-4 下行事件 Event (IF-BluePrint L0-4)
// ============================================================================

/// 下行反写事件分类学（白皮书 §14.2、IF-BluePrint L0-4）。
///
/// 严格收敛为 7 项封闭枚举，禁止未登记事件出环。
/// 依附录 C.1-F4 裁决：宏派生统一收缩（rename_all = "snake_case"），
/// 仅 Pong 变体特化 rename = "PONG" 以忠实保留白皮书 §14.2 原始字面样式。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// PING 握手回执。
    #[serde(rename = "PONG")]
    Pong,
    /// 视口双阶段离场彻底销毁完成通告。
    Dismissed {
        /// 销毁完成的目标视口标识。
        target: TargetId,
    },
    /// 交互业务意图冒泡大动脉。
    Intent {
        /// 触发意图的目标视口标识。
        target: TargetId,
        /// 触发意图的原子标识。
        atom_id: AtomId,
        /// 归一化的用户交互动作。
        action: IntentAction,
    },
    /// 可选原子降级通知（特性裁剪或未启用）。
    AtomFallback {
        /// 所在目标视口标识。
        target: TargetId,
        /// 降级原子标识。
        atom_id: AtomId,
        /// 降级归因。
        reason: FallbackReason,
    },
    /// 单槽脏表背压溢出告警（Schema 冻结富化载荷，白皮书 §12.4）。
    BackpressureWarning {
        /// 被丢弃的差分原子标识。
        dropped_atom: AtomId,
        /// 内核当前实际保留应用的上一有效快照。
        last_applied: PatchPayload,
        /// 该原子的墓碑时间戳（微秒）。
        tombstone_us: u64,
    },
    /// 驱动后备降级等非阻断通报。
    Warning {
        /// 告警归因。
        reason: WarningReason,
    },
    /// 策略阻断或运行态熔断通报。
    Error {
        /// 阻断与熔断归因（封闭 4 项）。
        reason: ErrorReason,
    },
}

/// 可选原子降级归因（白皮书 §8.3、IF-BluePrint L0-4）。
///
/// 依据白皮书 §8.3 降级契约文本，仅收敛定义法定存在的 reason。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FallbackReason {
    /// 对应 Cargo Feature 门控位未启用（交付 0 尺寸幽灵图元）。
    FeatureDisabled,
}

/// 非阻断告警归因（白皮书 §11.2、§14.2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WarningReason {
    /// 硬件 Vulkan 驱动缺失，已平滑回落至 tiny-skia CPU 软渲染管线。
    GpuDriverFallback,
}

/// 策略阻断与硬熔断归因（白皮书 §14.2、IF-BluePrint L0-4）。
///
/// 严格封闭为 4 项契约级枚举，禁止开放性错误码。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorReason {
    /// 跨连接越权劫持目标视口被拒绝（已由其他连接独占绑定）。
    TargetOwned,
    /// GNOME/Mutter 环境下请求了不支持的 Layer-Shell 协议形态（绝对拒绝）。
    UnsupportedLayerInMutter,
    /// 宿主合成器未通告所请求的 Wayland 协议扩展。
    ProtocolRestricted,
    /// 渲染管线死线熔断（防止主循环死锁或显存挂死）。
    RenderPipelineTimeout,
}