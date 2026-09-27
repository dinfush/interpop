//! 画布生命周期核心状态机与 P1-9 单漏斗架构 (canvas_transition)
//!
//! 依据白皮书 §6.3 画布生命周期状态机、IF-BluePrint P1-9 与台账 #1、#7、#21、#24。
//!
//! 本模块实现核心状态迁移的纯函数单漏斗 `transition()`。
//! 状态机拓扑由 Rust 强类型系统与显式 match 穷举全网格严格密封，
//! 越权迁移在编译期与运行态均不可表示。

use core::fmt;
use crate::contract::{CanvasId, SpawnRequest, TargetId};

/// 画布生命周期的内部相位枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CanvasPhase {
    /// 未映射地皮（初始态）。
    Unmapped,
    /// 协商与配置阶段（等待合成器初次 configure）。
    Configuring,
    /// 活跃渲染态（正常接收事件与渲染上屏）。
    Active,
    /// 离场动效过渡期（注销输入，播放 150ms~250ms 平滑补间）。
    Dismissing,
    /// 表面已解绑注销（地皮与显存已交还合成器，不可逆）。
    Reclaimed,
    /// 终极销毁态（生命周期终结）。
    Destroyed,
}

/// 空间状态机宿主载体（Newtype 构造器密封）。
///
/// 内部包含画布相位及其绑定的空间身份上下文（`CanvasId` 与 `TargetId`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanvasState {
    pub(crate) phase: CanvasPhase,
    pub(crate) id: CanvasId,
    pub(crate) target: TargetId,
}

impl CanvasState {
    /// 创建未映射的空白初始态。
    ///
    /// # 设计接缝说明
    /// 返回的 `id: 0` 为占位值，`Unmapped` 态在物理与逻辑上均未持有有效的画布地皮身份。
    #[must_use]
    pub const fn unmapped() -> Self {
        Self {
            phase: CanvasPhase::Unmapped,
            id: 0,
            target: String::new(),
        }
    }

    /// 构造处于 Configuring 阶段的测试与运行时实例。
    #[must_use]
    pub fn configuring(id: CanvasId, target: impl Into<TargetId>) -> Self {
        Self {
            phase: CanvasPhase::Configuring,
            id,
            target: target.into(),
        }
    }

    /// 构造处于 Active 阶段的实例。
    #[must_use]
    pub fn active(id: CanvasId, target: impl Into<TargetId>) -> Self {
        Self {
            phase: CanvasPhase::Active,
            id,
            target: target.into(),
        }
    }

    /// 构造处于 Dismissing 阶段的实例。
    #[must_use]
    pub fn dismissing(id: CanvasId, target: impl Into<TargetId>) -> Self {
        Self {
            phase: CanvasPhase::Dismissing,
            id,
            target: target.into(),
        }
    }

    /// 构造处于 Reclaimed 阶段的实例。
    #[must_use]
    pub fn reclaimed(id: CanvasId, target: impl Into<TargetId>) -> Self {
        Self {
            phase: CanvasPhase::Reclaimed,
            id,
            target: target.into(),
        }
    }

    /// 构造处于 Destroyed 阶段的实例。
    #[must_use]
    pub fn destroyed(id: CanvasId, target: impl Into<TargetId>) -> Self {
        Self {
            phase: CanvasPhase::Destroyed,
            id,
            target: target.into(),
        }
    }

    /// 获取当前所处相位。
    #[must_use]
    pub const fn phase(&self) -> CanvasPhase {
        self.phase
    }

    /// 获取当前画布绑定的 TargetId 引用。
    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    /// 获取当前画布唯一的 CanvasId。
    #[must_use]
    pub const fn id(&self) -> CanvasId {
        self.id
    }
}

/// 销毁策略机仲裁触发原因（P2-8 DismissArbiter 产生与 §10 归因）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DismissReason {
    /// 空间指针点击在画布 AABB 包围盒外（§6.4 策略）。
    ClickOutside,
    /// 视口交互超时（§6.4 策略）。
    Timeout,
    /// 失去键盘输入焦点（§6.4 策略）。
    OnBlur,
    /// 模态视口未捕获的全局 Escape 离场（§10 主动清退内部归因，非 §6.4 外部声明策略）。
    Escape,
}

/// 汇入状态机的所有离场触发源分类（白皮书 §6.4、§12.5、IF-BluePrint P1-9）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DismissTrigger {
    /// 策略机综合决策触发。
    Arbiter(DismissReason),
    /// 硬件心跳守护超时（§12.5）。
    HeartbeatTimeout,
    /// 客户端连接断开 EOF 强制清退（§5.2）。
    ConnectionEof,
    /// 外部协议下发显式 dismiss 动词（§14.1）。
    ExternalDismiss,
    /// 同 UID 客户端 force: true 强制抢管接管（§5.2、台账 #1）。
    ForceTakeover,
}

/// 驱动单漏斗状态迁移的输入事件（台账 #21、#24）。
#[derive(Debug, Clone, PartialEq)]
pub enum CanvasEvent {
    /// 申请创建新视口。
    Spawn(SpawnRequest),
    /// 各类渠道汇入的销毁请求。
    Dismiss(DismissTrigger),
    /// 离场动效正常播放完毕，或初次配置就绪。
    AnimFinished,
    /// 底层 Wayland Surface 异常物理断裂注销。
    SurfaceDestroyed,
    /// 离场动画期内极速重启原位复活（§12.5）。
    RebindSpawn,
    /// 看门狗定时器超时触发守护（陷阱 5.3、台账 #21、#24）。
    GuardTimeout,
}

/// 状态迁移伴随产生的物理副作用序列（台账 #22、#24）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// 注销输入区域 input_region，阻断后续点击（白皮书 §6.3 阶段 A）。
    UnregisterInputRegion(CanvasId),
    /// 强制熔断该 Target 名下的全部活跃交互租约（台账 #1）。
    EvictLeases(TargetId),
    /// 启动离场补间动效计时器（150ms~250ms 动效超时守护）。
    StartDismissTimer(CanvasId),
    /// 原位复活：重置动画时钟并恢复 Active（白皮书 §12.5）。
    ResetAnimClock(CanvasId),
    /// 解绑并释放底层 Wayland Surface 及显存句柄（白皮书 §6.3 阶段 B）。
    ReleaseSurface(CanvasId),
    /// 向客户端广播 dismissed 离场完毕事件（白皮书 §14.2）。
    EmitDismissed(TargetId),
}

/// 状态迁移结果载体。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionResult {
    /// 转移后的新状态。
    pub state: CanvasState,
    /// 伴随产生的副作用序列（按执行先后顺序保序）。
    pub effects: Vec<Effect>,
}

/// 非法状态迁移异常（违反白皮书 §6.3 宪法拓扑）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvariantViolation {
    /// 不合法的状态迁移路径。
    InvalidTransition,
}

impl fmt::Display for InvariantViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("违反状态机宪法拓扑的不合法迁移")
    }
}

impl std::error::Error for InvariantViolation {}

/// 核心状态迁移单漏斗纯函数（P1-9 transition）。
///
/// 严格显式穷举白皮书 §6.3 状态拓扑；无锁、无全局依赖、纯确定性执行。
///
/// # Errors
///
/// 当输入事件在当前相位下不合法时，返回 [`InvariantViolation::InvalidTransition`]。
pub fn transition(
    cur: CanvasState,
    ev: &CanvasEvent,
) -> Result<TransitionResult, InvariantViolation> {
    match (cur.phase, ev) {
        // ─────────────────────────────────────────────────────────────────
        // 1. Unmapped 态合法路径
        // ─────────────────────────────────────────────────────────────────
        // (a) 初次申请地皮
        // 设计接缝说明：返回态的 id 承袭 cur.id（Unmapped 态本为占位 0），
        // 真实业务 CanvasId 归属 KernelShell 上下文逐画布分发绑定（台账 #21）。
        (CanvasPhase::Unmapped, CanvasEvent::Spawn(req)) => Ok(TransitionResult {
            state: CanvasState {
                phase: CanvasPhase::Configuring,
                id: cur.id,
                target: req.target.clone(),
            },
            effects: Vec::new(),
        }),

        // ─────────────────────────────────────────────────────────────────
        // 2. Configuring 期合法路径
        // ─────────────────────────────────────────────────────────────────
        // (a) 看门狗超时：合成器未下发 configure，直达 Destroyed（陷阱 5.3）
        (CanvasPhase::Configuring, CanvasEvent::GuardTimeout) => Ok(TransitionResult {
            state: CanvasState {
                phase: CanvasPhase::Destroyed,
                id: cur.id,
                target: cur.target,
            },
            effects: vec![Effect::ReleaseSurface(cur.id)],
        }),

        // (b) 配置期提前收到离场：快径清场，防御性熔断租约直达 Destroyed
        // （注释：Configuring 期正常无活跃租约，此处 EvictLeases 属防御性清扫）
        (CanvasPhase::Configuring, CanvasEvent::Dismiss(_)) => Ok(TransitionResult {
            state: CanvasState {
                phase: CanvasPhase::Destroyed,
                id: cur.id,
                target: cur.target.clone(),
            },
            effects: vec![
                Effect::ReleaseSurface(cur.id),
                Effect::EvictLeases(cur.target),
            ],
        }),

        // (c) 物理表面断裂：直达 Destroyed
        (CanvasPhase::Configuring, CanvasEvent::SurfaceDestroyed) => Ok(TransitionResult {
            state: CanvasState {
                phase: CanvasPhase::Destroyed,
                id: cur.id,
                target: cur.target,
            },
            effects: vec![Effect::ReleaseSurface(cur.id)],
        }),

        // (d) 初次首帧就绪，进入 Active
        (CanvasPhase::Configuring, CanvasEvent::AnimFinished) => Ok(TransitionResult {
            state: CanvasState {
                phase: CanvasPhase::Active,
                id: cur.id,
                target: cur.target,
            },
            effects: Vec::new(),
        }),

        // ─────────────────────────────────────────────────────────────────
        // 3. Active 活跃期合法路径
        // ─────────────────────────────────────────────────────────────────
        // (a) 触发离场：进入 Dismissing，内联注销输入 + 熔断租约 + 启动动效计时器 (台账 #1)
        (CanvasPhase::Active, CanvasEvent::Dismiss(_)) => Ok(TransitionResult {
            state: CanvasState {
                phase: CanvasPhase::Dismissing,
                id: cur.id,
                target: cur.target.clone(),
            },
            effects: vec![
                Effect::UnregisterInputRegion(cur.id),
                Effect::EvictLeases(cur.target),
                Effect::StartDismissTimer(cur.id),
            ],
        }),

        // (b) 活跃期物理表面异常断裂：直接进入 Reclaimed
        (CanvasPhase::Active, CanvasEvent::SurfaceDestroyed) => Ok(TransitionResult {
            state: CanvasState {
                phase: CanvasPhase::Reclaimed,
                id: cur.id,
                target: cur.target.clone(),
            },
            effects: vec![
                Effect::UnregisterInputRegion(cur.id),
                Effect::EvictLeases(cur.target),
                Effect::ReleaseSurface(cur.id),
            ],
        }),

        // ─────────────────────────────────────────────────────────────────
        // 4. Dismissing 动效离场期合法路径
        // ─────────────────────────────────────────────────────────────────
        // (a) 动效正常播完：进入 Reclaimed，出环 EmitDismissed (台账 #22)
        (CanvasPhase::Dismissing, CanvasEvent::AnimFinished) => Ok(TransitionResult {
            state: CanvasState {
                phase: CanvasPhase::Reclaimed,
                id: cur.id,
                target: cur.target.clone(),
            },
            effects: vec![
                Effect::ReleaseSurface(cur.id),
                Effect::EmitDismissed(cur.target),
            ],
        }),

        // (b) 动效超时看门狗触发：强制进入 Reclaimed，出环 EmitDismissed (台账 #24)
        (CanvasPhase::Dismissing, CanvasEvent::GuardTimeout) => Ok(TransitionResult {
            state: CanvasState {
                phase: CanvasPhase::Reclaimed,
                id: cur.id,
                target: cur.target.clone(),
            },
            effects: vec![
                Effect::ReleaseSurface(cur.id),
                Effect::EmitDismissed(cur.target),
            ],
        }),

        // (c) 动效期物理表面断裂：进入 Reclaimed，不出环 (台账 #22)
        (CanvasPhase::Dismissing, CanvasEvent::SurfaceDestroyed) => Ok(TransitionResult {
            state: CanvasState {
                phase: CanvasPhase::Reclaimed,
                id: cur.id,
                target: cur.target,
            },
            effects: vec![Effect::ReleaseSurface(cur.id)],
        }),

        // (d) 离场期同名 spawn 极速重启原位复活：重置时钟恢复 Active (白皮书 §12.5)
        (CanvasPhase::Dismissing, CanvasEvent::RebindSpawn) => Ok(TransitionResult {
            state: CanvasState {
                phase: CanvasPhase::Active,
                id: cur.id,
                target: cur.target,
            },
            effects: vec![Effect::ResetAnimClock(cur.id)],
        }),

        // ─────────────────────────────────────────────────────────────────
        // 5. 显式穷举非法转移路径（共 25 格，零兜底通配符，激活编译期穷举哨兵）
        // ─────────────────────────────────────────────────────────────────
        (
            CanvasPhase::Unmapped,
            CanvasEvent::Dismiss(_)
            | CanvasEvent::AnimFinished
            | CanvasEvent::SurfaceDestroyed
            | CanvasEvent::RebindSpawn
            | CanvasEvent::GuardTimeout,
        )
        | (
            CanvasPhase::Configuring,
            CanvasEvent::Spawn(_) | CanvasEvent::RebindSpawn,
        )
        | (
            CanvasPhase::Active,
            CanvasEvent::Spawn(_)
            | CanvasEvent::AnimFinished
            | CanvasEvent::RebindSpawn
            | CanvasEvent::GuardTimeout,
        )
        | (
            CanvasPhase::Dismissing,
            CanvasEvent::Spawn(_) | CanvasEvent::Dismiss(_),
        )
        | (
            CanvasPhase::Reclaimed,
            CanvasEvent::Spawn(_)
            | CanvasEvent::Dismiss(_)
            | CanvasEvent::AnimFinished
            | CanvasEvent::SurfaceDestroyed
            | CanvasEvent::RebindSpawn
            | CanvasEvent::GuardTimeout,
        )
        | (
            CanvasPhase::Destroyed,
            CanvasEvent::Spawn(_)
            | CanvasEvent::Dismiss(_)
            | CanvasEvent::AnimFinished
            | CanvasEvent::SurfaceDestroyed
            | CanvasEvent::RebindSpawn
            | CanvasEvent::GuardTimeout,
        ) => Err(InvariantViolation::InvalidTransition),
    }
}