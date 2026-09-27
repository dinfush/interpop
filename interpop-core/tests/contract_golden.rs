//! # PR-01 L0 契约与 Golden 样本锁定集成测试
//!
//! 本测试集严格兑现 SOP-P1 PR-01 卡规定的四类测试先行规格：
//! 1. Golden 逐字节往返与 AtomSpec 18 变体全构造往返 (F1/F2/F4)
//! 2. 前向未知字段容忍性断言 (台账#6)
//! 3. DSL 复合组件封禁反射断言 (白皮书 §9 / §18.1-2)
//! 4. 下行反写 7 事件全量往返矩阵与富化载荷断言 (附录 C.1-R1 / 白皮书 §12.4)

use interpop_core::contract::{
    AtomSpec, DismissPolicy, ErrorReason, Event, FallbackReason, Form, IntentAction,
    PatchPayload, PatchRequest, Rect, Request, RestyleRequest, SpawnRequest, WarningReason,
};
use std::collections::BTreeMap;
use std::fs;

/// 读取测试金样本文件并剔除尾部 '\n'，保证与紧凑单行 NDJSON 序列化产物逐字节对齐。
///
/// 路径由编译期 CARGO_MANIFEST_DIR 静态确定，绝对锚定 workspace 根目录 tests/golden/，
/// 严禁任何运行态猜测与回退分支。
fn read_golden(filename: &str) -> String {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/golden/");
    let path = format!("{}{}", dir, filename);
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("无法读取金样本文件 {}: {}", path, err));
    content.trim_end_matches('\n').to_string()
}

// ============================================================================
// 测试规格 ①: Golden 逐字节往返与 AtomSpec 18 变体全构造往返
// ============================================================================

#[test]
fn test_golden_spawn_popup_roundtrip() {
    let golden = read_golden("spawn_popup.json");
    let req: Request = serde_json::from_str(&golden).expect("反序列化 spawn_popup 失败");

    match &req {
        Request::Spawn(spawn @ SpawnRequest { .. }) => {
            assert_eq!(spawn.form, Form::Popup);
            assert_eq!(spawn.target, "vol-island");
            assert_eq!(spawn.dismiss_policy, DismissPolicy::Manual);
            assert_eq!(spawn.output, None);
            assert!(!spawn.force);
            assert_eq!(spawn.heartbeat_ms, Some(5000));
            assert_eq!(
                spawn.anchor_rect,
                Some(Rect {
                    x: 1720,
                    y: 40,
                    w: 64,
                    h: 32
                })
            );
            assert_eq!(
                spawn.atoms,
                AtomSpec::Box {
                    children: vec![AtomSpec::Fill {
                        color: "#204060".to_string()
                    }]
                }
            );
        }
        _ => panic!("预期为 Request::Spawn 变体"),
    }

    let serialized = serde_json::to_string(&req).expect("序列化 spawn_popup 失败");
    assert_eq!(
        serialized, golden,
        "SpawnRequest 序列化结果与 golden 样本未达成逐字节全等"
    );
}

#[test]
fn test_golden_patch_slider_roundtrip() {
    let golden = read_golden("patch_slider.json");
    let req: Request = serde_json::from_str(&golden).expect("反序列化 patch_slider 失败");

    match &req {
        Request::Patch(patch @ PatchRequest { .. }) => {
            assert_eq!(patch.target, "vol-island");
            assert_eq!(patch.atom_id, "s1");
            assert_eq!(patch.timestamp, 1760000000123456);
            assert_eq!(patch.payload, PatchPayload::Slider { value: 0.5 });
        }
        _ => panic!("预期为 Request::Patch 变体"),
    }

    let serialized = serde_json::to_string(&req).expect("序列化 patch_slider 失败");
    assert_eq!(
        serialized, golden,
        "PatchRequest 序列化结果与 golden 样本未达成逐字节全等"
    );
}

#[test]
fn test_golden_error_target_owned_roundtrip() {
    let golden = read_golden("error_target_owned.json");
    let ev: Event = serde_json::from_str(&golden).expect("反序列化 error_target_owned 失败");

    match &ev {
        Event::Error { reason } => {
            assert_eq!(*reason, ErrorReason::TargetOwned);
        }
        _ => panic!("预期为 Event::Error 变体"),
    }

    let serialized = serde_json::to_string(&ev).expect("序列化 error_target_owned 失败");
    assert_eq!(
        serialized, golden,
        "Event::Error 序列化结果与 golden 样本未达成逐字节全等"
    );
}

#[test]
fn test_request_five_verbs_exhaustive_roundtrip() {
    // 覆盖 Request 5 项收敛顶层动词 (IF-BluePrint L0-1)
    let requests = vec![
        Request::Ping,
        Request::Dismiss {
            target: "vol-island".into(),
        },
        Request::Restyle(RestyleRequest {
            theme: "dark".into(),
            tokens: BTreeMap::from([
                ("bg_primary".to_string(), "#1c1c1c".to_string()),
                ("accent".to_string(), "#24405e".to_string()),
            ]),
        }),
    ];

    for req in requests {
        let json = serde_json::to_string(&req).expect("序列化 Request 失败");
        let decoded: Request = serde_json::from_str(&json).expect("反序列化 Request 失败");
        assert_eq!(req, decoded, "Request 动词往返未达成全等: {}", json);
    }
}

#[test]
fn test_atomspec_18_variants_exhaustive_roundtrip() {
    // 涵盖白皮书 §8.3 17 项标准基元 + §16 探路原子 fill 共 18 变体 (台账#20 / F1)
    let variants: Vec<AtomSpec> = vec![
        // P0
        AtomSpec::Box {
            children: vec![AtomSpec::Fill {
                color: "#ff0000".into(),
            }],
        },
        AtomSpec::Text {
            content: "hello interpop".into(),
        },
        AtomSpec::Button {
            id: Some("btn1".into()),
            child: Box::new(AtomSpec::Text {
                content: "ok".into(),
            }),
        },
        AtomSpec::Slider {
            id: Some("s1".into()),
            value: 0.75,
            min: Some(0.15),
            max: Some(1.0),
            step: Some(0.05),
        },
        AtomSpec::Switch {
            id: Some("sw1".into()),
            checked: true,
        },
        AtomSpec::Progress {
            id: Some("p1".into()),
            value: 0.5,
        },
        AtomSpec::Image {
            src: "icon.svg".into(),
        },
        // P1
        AtomSpec::Grid {
            children: vec![],
        },
        AtomSpec::Stack {
            children: vec![],
        },
        AtomSpec::List {
            id: Some("l1".into()),
        },
        AtomSpec::Input {
            id: Some("i1".into()),
            placeholder: Some("Search...".into()),
        },
        AtomSpec::Scroll {
            child: Box::new(AtomSpec::Text {
                content: "scrollable".into(),
            }),
        },
        // P2
        AtomSpec::Path {
            data: "M 0 0 L 10 10".into(),
        },
        AtomSpec::Surface {
            slot: "mpv-slot".into(),
        },
        AtomSpec::Radio {
            id: Some("r1".into()),
            selected: false,
        },
        AtomSpec::Checkbox {
            id: Some("c1".into()),
            checked: true,
        },
        AtomSpec::Tray {
            service: "org.kde.StatusNotifierItem-1".into(),
        },
        // Phase 1 探路原子
        AtomSpec::Fill {
            color: "#1c1f26".into(),
        },
    ];

    assert_eq!(variants.len(), 18, "AtomSpec 必须且恰好覆盖 18 变体全集");

    for spec in variants {
        let json = serde_json::to_string(&spec).expect("序列化 AtomSpec 变体失败");
        let decoded: AtomSpec = serde_json::from_str(&json).expect("反序列化 AtomSpec 变体失败");
        assert_eq!(spec, decoded, "AtomSpec 变体往返未达成全等: {}", json);
    }
}

#[test]
fn test_patch_payload_exhaustive_roundtrip() {
    let payloads = vec![
        PatchPayload::Slider { value: 0.25 },
        PatchPayload::Switch { checked: false },
        PatchPayload::Progress { value: 0.875 },
        PatchPayload::Text {
            content: "updated text".into(),
        },
        PatchPayload::Fill {
            color: "#abcdef".into(),
        },
    ];

    for p in payloads {
        let json = serde_json::to_string(&p).expect("序列化 PatchPayload 失败");
        let decoded: PatchPayload =
            serde_json::from_str(&json).expect("反序列化 PatchPayload 失败");
        assert_eq!(p, decoded);
    }
}

// ============================================================================
// 测试规格 ②: 容忍未知字段测试 (台账#6)
// ============================================================================

#[test]
fn test_forward_compatibility_unknown_fields_tolerance() {
    // 注入机制注记: Request 内部标记下，verb 所在 Map 的其余键全部平铺流入变体载荷。
    // 在此注入未知键，直接验证未知字段容忍与反序列化安全性 (台账#6)。
    let raw = r##"{
        "verb": "spawn",
        "form": "panel",
        "target": "top-bar",
        "dismiss_policy": "manual",
        "atoms": {"type": "fill", "color": "#000000"},
        "__future_protocol_extension": 42,
        "experimental_flags": ["smooth_vblank", "direct_scanout"]
    }"##;

    let req: Request = serde_json::from_str(raw).expect("容忍未知字段反序列化失败");
    match req {
        Request::Spawn(spawn) => {
            assert_eq!(spawn.form, Form::Panel);
            assert_eq!(spawn.target, "top-bar");
        }
        _ => panic!("解析结果应当为 Request::Spawn"),
    }
}

// ============================================================================
// 测试规格 ③: 封禁反射测试 (白皮书 §9 / §18.1-2)
// ============================================================================

#[test]
fn test_prohibited_dsl_macro_components_rejected() {
    let prohibited_types = ["badge", "chip", "session_tile"];

    for comp in prohibited_types {
        let raw = format!(
            r#"{{"type":"{}","content":"prohibited","label":"fail"}}"#,
            comp
        );
        let result: Result<AtomSpec, _> = serde_json::from_str(&raw);
        assert!(
            result.is_err(),
            "DSL 复合组件 '{}' 严禁作为独立原子直接解析，必须在宏展开层由外部展开",
            comp
        );
    }
}

// ============================================================================
// 测试规格 ④: 事件枚举往返矩阵 (附录 C.1-R1 / 白皮书 §12.4)
// ============================================================================
#[test]
fn test_event_exhaustive_matrix_roundtrip() {
    let events = vec![
        Event::Pong,
        Event::Dismissed {
            target: "vol-island".into(),
        },
        Event::Intent {
            target: "vol-island".into(),
            atom_id: "s1".into(),
            action: IntentAction::Release { value: 0.5 },
        },
        Event::AtomFallback {
            target: "status-bar".into(),
            atom_id: "tray1".into(),
            reason: FallbackReason::FeatureDisabled,
        },
        Event::BackpressureWarning {
            dropped_atom: "s1".into(),
            last_applied: PatchPayload::Slider { value: 0.5 },
            tombstone_us: 1760000000999999,
        },
        Event::Warning {
            reason: WarningReason::GpuDriverFallback,
        },
        Event::Error {
            reason: ErrorReason::TargetOwned,
        },
        Event::Error {
            reason: ErrorReason::UnsupportedLayerInMutter,
        },
        Event::Error {
            reason: ErrorReason::ProtocolRestricted,
        },
        Event::Error {
            reason: ErrorReason::RenderPipelineTimeout,
        },
    ];

    for ev in events {
        let json = serde_json::to_string(&ev).expect("序列化 Event 失败");

        match ev {
            // 重点断言 1: Pong 必须显式大写
            Event::Pong => {
                assert_eq!(
                    json, r#"{"event":"PONG"}"#,
                    "Pong 事件序列化必须保留白皮书 §14.2 大写 PONG 样式"
                );
            }
            // 重点断言 3: backpressure_warning 富化载荷 Schema 锁定
            Event::BackpressureWarning { .. } => {
                assert!(
                    json.starts_with(r#"{"event":""#),
                    "事件必须携带 event 标签: {}",
                    json
                );
                assert!(
                    json.contains(r#""dropped_atom":"s1""#),
                    "backpressure_warning 缺失 dropped_atom: {}",
                    json
                );
                assert!(
                    json.contains(r#""last_applied":{"slider":{"value":0.5}}"#),
                    "backpressure_warning 缺失 last_applied 载荷: {}",
                    json
                );
                assert!(
                    json.contains(r#""tombstone_us":1760000000999999"#),
                    "backpressure_warning 缺失 tombstone_us: {}",
                    json
                );
            }
            // 重点断言 2: 其余事件必须携带 snake_case 的 event 字段
            _ => {
                assert!(
                    json.starts_with(r#"{"event":""#),
                    "事件必须携带 event 标签: {}",
                    json
                );
            }
        }

        let decoded: Event = serde_json::from_str(&json).expect("反序列化 Event 失败");
        assert_eq!(ev, decoded, "Event 往返全等校验失败: {}", json);
    }
}