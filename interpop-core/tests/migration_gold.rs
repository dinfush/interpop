//! 核心状态机 P1-9 单漏斗 36 格迁移金表测试 (G9 门禁)
//!
//! 依据白皮书 §6.3 拓扑、IF-BluePrint P1-9 与台账 #1、#7、#21、#24。

use interpop_core::canvas_transition::{
    transition, CanvasEvent, CanvasPhase, CanvasState, DismissReason, DismissTrigger, Effect,
    InvariantViolation,
};
use interpop_core::contract::{DismissPolicy, Form, SpawnRequest};

fn dummy_spawn_req(target: &str) -> SpawnRequest {
    SpawnRequest {
        form: Form::Panel,
        target: target.into(),
        dismiss_policy: DismissPolicy::Manual,
        atoms: serde_json::from_str(r##"{"type":"fill","color":"#000000"}"##).unwrap(),
        output: None,
        force: false,
        heartbeat_ms: None,
        anchor_rect: None,
    }
}

#[test]
fn test_exhaustive_migration_gold_matrix_36_cells() {
    let test_id = 42;
    let test_target = "test-target";

    let states = [
        CanvasState::unmapped(),
        CanvasState::configuring(test_id, test_target),
        CanvasState::active(test_id, test_target),
        CanvasState::dismissing(test_id, test_target),
        CanvasState::reclaimed(test_id, test_target),
        CanvasState::destroyed(test_id, test_target),
    ];

    let events = [
        CanvasEvent::Spawn(dummy_spawn_req(test_target)),
        CanvasEvent::Dismiss(DismissTrigger::ExternalDismiss),
        CanvasEvent::AnimFinished,
        CanvasEvent::SurfaceDestroyed,
        CanvasEvent::RebindSpawn,
        CanvasEvent::GuardTimeout,
    ];

    let mut legal_transitions = 0;
    let mut illegal_transitions = 0;

    for cur in states {
        for ev in &events {
            let res = transition(cur.clone(), ev);
            match (cur.phase(), ev) {
                // 1. Unmapped 态
                (CanvasPhase::Unmapped, CanvasEvent::Spawn(req)) => {
                    let r = res.expect("Unmapped + Spawn 必须转移至 Configuring");
                    assert_eq!(r.state.phase(), CanvasPhase::Configuring);
                    assert_eq!(r.state.target(), req.target);
                    assert!(r.effects.is_empty(), "Spawn 入场不产生离场副作用");
                    legal_transitions += 1;
                }
                (CanvasPhase::Unmapped, _) => {
                    assert_eq!(res.unwrap_err(), InvariantViolation::InvalidTransition);
                    illegal_transitions += 1;
                }

                // 2. Configuring 态
                (CanvasPhase::Configuring, CanvasEvent::GuardTimeout) => {
                    let r = res.expect("Configuring + GuardTimeout 必须直达 Destroyed");
                    assert_eq!(r.state.phase(), CanvasPhase::Destroyed);
                    assert_eq!(r.effects, vec![Effect::ReleaseSurface(test_id)]);
                    legal_transitions += 1;
                }
                (CanvasPhase::Configuring, CanvasEvent::Dismiss(_)) => {
                    let r = res.expect("Configuring + Dismiss 快径清场至 Destroyed");
                    assert_eq!(r.state.phase(), CanvasPhase::Destroyed);
                    assert_eq!(
                        r.effects,
                        vec![
                            Effect::ReleaseSurface(test_id),
                            Effect::EvictLeases(test_target.into()),
                        ]
                    );
                    legal_transitions += 1;
                }
                (CanvasPhase::Configuring, CanvasEvent::SurfaceDestroyed) => {
                    let r = res.expect("Configuring + SurfaceDestroyed 直达 Destroyed");
                    assert_eq!(r.state.phase(), CanvasPhase::Destroyed);
                    assert_eq!(r.effects, vec![Effect::ReleaseSurface(test_id)]);
                    legal_transitions += 1;
                }
                (CanvasPhase::Configuring, CanvasEvent::AnimFinished) => {
                    let r = res.expect("Configuring + AnimFinished 配置就绪进入 Active");
                    assert_eq!(r.state.phase(), CanvasPhase::Active);
                    assert_eq!(r.state.target(), test_target);
                    assert!(r.effects.is_empty());
                    legal_transitions += 1;
                }
                (CanvasPhase::Configuring, _) => {
                    assert_eq!(res.unwrap_err(), InvariantViolation::InvalidTransition);
                    illegal_transitions += 1;
                }

                // 3. Active 态
                (CanvasPhase::Active, CanvasEvent::Dismiss(_)) => {
                    let r = res.expect("Active + Dismiss 启动双阶段离场进入 Dismissing");
                    assert_eq!(r.state.phase(), CanvasPhase::Dismissing);
                    assert_eq!(
                        r.effects,
                        vec![
                            Effect::UnregisterInputRegion(test_id),
                            Effect::EvictLeases(test_target.into()),
                            Effect::StartDismissTimer(test_id),
                        ]
                    );
                    legal_transitions += 1;
                }
                (CanvasPhase::Active, CanvasEvent::SurfaceDestroyed) => {
                    let r = res.expect("Active + SurfaceDestroyed 异常断裂进入 Reclaimed");
                    assert_eq!(r.state.phase(), CanvasPhase::Reclaimed);
                    assert_eq!(
                        r.effects,
                        vec![
                            Effect::UnregisterInputRegion(test_id),
                            Effect::EvictLeases(test_target.into()),
                            Effect::ReleaseSurface(test_id),
                        ]
                    );
                    legal_transitions += 1;
                }
                (CanvasPhase::Active, _) => {
                    assert_eq!(res.unwrap_err(), InvariantViolation::InvalidTransition);
                    illegal_transitions += 1;
                }

                // 4. Dismissing 态
                (CanvasPhase::Dismissing, CanvasEvent::AnimFinished) => {
                    let r = res.expect("Dismissing + AnimFinished 正常离场进入 Reclaimed");
                    assert_eq!(r.state.phase(), CanvasPhase::Reclaimed);
                    assert_eq!(
                        r.effects,
                        vec![
                            Effect::ReleaseSurface(test_id),
                            Effect::EmitDismissed(test_target.into()),
                        ]
                    );
                    legal_transitions += 1;
                }
                (CanvasPhase::Dismissing, CanvasEvent::GuardTimeout) => {
                    let r = res.expect("Dismissing + GuardTimeout 动效超时强制进入 Reclaimed");
                    assert_eq!(r.state.phase(), CanvasPhase::Reclaimed);
                    assert_eq!(
                        r.effects,
                        vec![
                            Effect::ReleaseSurface(test_id),
                            Effect::EmitDismissed(test_target.into()),
                        ]
                    );
                    legal_transitions += 1;
                }
                (CanvasPhase::Dismissing, CanvasEvent::SurfaceDestroyed) => {
                    let r = res.expect("Dismissing + SurfaceDestroyed 物理断裂进入 Reclaimed");
                    assert_eq!(r.state.phase(), CanvasPhase::Reclaimed);
                    assert_eq!(r.effects, vec![Effect::ReleaseSurface(test_id)]);
                    legal_transitions += 1;
                }
                (CanvasPhase::Dismissing, CanvasEvent::RebindSpawn) => {
                    let r = res.expect("Dismissing + RebindSpawn 原位复活回 Active");
                    assert_eq!(r.state.phase(), CanvasPhase::Active);
                    assert_eq!(r.effects, vec![Effect::ResetAnimClock(test_id)]);
                    legal_transitions += 1;
                }
                (CanvasPhase::Dismissing, _) => {
                    assert_eq!(res.unwrap_err(), InvariantViolation::InvalidTransition);
                    illegal_transitions += 1;
                }

                // 5. Reclaimed 态 & 6. Destroyed 态 (终态)
                (CanvasPhase::Reclaimed, _) | (CanvasPhase::Destroyed, _) => {
                    assert_eq!(
                        res.unwrap_err(),
                        InvariantViolation::InvalidTransition,
                        "已解绑或已销毁终态禁止接收任何事件"
                    );
                    illegal_transitions += 1;
                }
            }
        }
    }

    assert_eq!(legal_transitions, 11, "合法转移必须严格为 11 格");
    assert_eq!(illegal_transitions, 25, "非法转移必须严格为 25 格");
    assert_eq!(legal_transitions + illegal_transitions, 36, "总网格必须严格为 36 格");
}

#[test]
fn test_dismiss_trigger_funnel_convergence() {
    let test_id = 99;
    let test_target = "funnel-target";

    let all_triggers = [
        DismissTrigger::ExternalDismiss,
        DismissTrigger::HeartbeatTimeout,
        DismissTrigger::ConnectionEof,
        DismissTrigger::ForceTakeover,
        DismissTrigger::Arbiter(DismissReason::ClickOutside),
    ];

    for trigger in all_triggers {
        // 1. Configuring 态下 5 种触发源行为全同
        let cur_cfg = CanvasState::configuring(test_id, test_target);
        let res_cfg = transition(cur_cfg, &CanvasEvent::Dismiss(trigger)).expect("Configuring 下 Dismiss 必须合法");
        assert_eq!(res_cfg.state.phase(), CanvasPhase::Destroyed);
        assert_eq!(
            res_cfg.effects,
            vec![
                Effect::ReleaseSurface(test_id),
                Effect::EvictLeases(test_target.into()),
            ]
        );

        // 2. Active 态下 5 种触发源行为全同 (单漏斗核心特征)
        let cur_act = CanvasState::active(test_id, test_target);
        let res_act = transition(cur_act, &CanvasEvent::Dismiss(trigger)).expect("Active 下 Dismiss 必须合法");
        assert_eq!(res_act.state.phase(), CanvasPhase::Dismissing);
        assert_eq!(
            res_act.effects,
            vec![
                Effect::UnregisterInputRegion(test_id),
                Effect::EvictLeases(test_target.into()),
                Effect::StartDismissTimer(test_id),
            ]
        );
    }
}