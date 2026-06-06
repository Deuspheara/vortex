use agent_protocol::{
    AndroidActionActor, AndroidActionPhase, AndroidActionTrace, AndroidActionVisualization,
    AndroidLanePolicy, AndroidObservation, AndroidPointPx, AndroidUiNode, IconToken, NetworkAccess,
    RiskLevel, ToolAssessment, ToolCapabilities, ToolCategory, ToolContext, ToolModeGate, ToolPack,
    ToolPackPolicy, ToolPolicy, ToolResult, ToolRuntimeFamily,
};
use android_device::{
    AdbSnapshotDriver, AndroidDeviceDriver, AndroidKey, AndroidTarget, JourneyRecorder,
    LogcatFilter, TextMatchMode, resolve_target,
};
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::tool::{AgentTool, default_finish_summary};

pub struct AndroidObserveTool;
pub struct AndroidEnsureEmulatorTool;
pub struct AndroidTapTextTool;
pub struct AndroidTapResourceIdTool;
pub struct AndroidTapPointTool;
pub struct AndroidTypeTextTool;
pub struct AndroidSwipeTool;
pub struct AndroidPressBackTool;
pub struct AndroidPressHomeTool;
pub struct AndroidLaunchAppTool;
pub struct AndroidReadLogcatTool;

#[async_trait]
impl AgentTool for AndroidObserveTool {
    fn name(&self) -> &'static str {
        "android.observe"
    }
    fn description(&self) -> &'static str {
        "Ensure an Android emulator is running, then observe it with an ADB screenshot and filtered UIAutomator tree."
    }
    fn schema(&self) -> Value {
        json!({"type":"object","additionalProperties":false,"properties":{"serial":{"type":"string"},"avd":{"type":"string"}}})
    }
    fn capabilities(&self) -> ToolCapabilities {
        android_capabilities()
    }
    fn policy(&self) -> ToolPolicy {
        android_policy(AndroidLanePolicy::Observe)
    }
    fn icon(&self) -> IconToken {
        IconToken::Terminal
    }
    fn label(&self, running: bool) -> String {
        label("Observe Android", running)
    }
    fn row_label(&self, _command: Option<&str>, running: bool) -> String {
        label("Android · observe", running)
    }
    fn finish_summary(&self, _args: &Value, output: &str, is_error: bool) -> String {
        android_finish_summary("Android · observed", output, is_error)
    }
    async fn assess(&self, args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        android_assessment(args, ctx, false, "observe Android emulator")
    }
    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
        let driver = driver(&args, &ctx);
        let observation = driver.observe().await?;
        let _ = JourneyRecorder::new(&ctx.project_root, &ctx.run_id.0)
            .and_then(|recorder| recorder.write_observation_refs(&observation));
        Ok(tool_json(
            self.name(),
            json!({
                "summary": observe_summary(&observation),
                "observation": observation,
            }),
        ))
    }
}

#[async_trait]
impl AgentTool for AndroidEnsureEmulatorTool {
    fn name(&self) -> &'static str {
        "android.ensure_emulator"
    }
    fn description(&self) -> &'static str {
        "Ensure an Android emulator is running. If none is connected, start the requested AVD or the first available AVD and wait for boot."
    }
    fn schema(&self) -> Value {
        json!({
            "type":"object",
            "additionalProperties":false,
            "properties":{
                "serial":{"type":"string"},
                "avd":{"type":"string"}
            }
        })
    }
    fn capabilities(&self) -> ToolCapabilities {
        android_capabilities()
    }
    fn policy(&self) -> ToolPolicy {
        android_policy(AndroidLanePolicy::Utility)
    }
    fn icon(&self) -> IconToken {
        IconToken::Terminal
    }
    fn label(&self, running: bool) -> String {
        label("Ensure Android emulator", running)
    }
    fn row_label(&self, _command: Option<&str>, running: bool) -> String {
        label("Android · ensure emulator", running)
    }
    fn finish_summary(&self, _args: &Value, output: &str, is_error: bool) -> String {
        android_finish_summary("Android · emulator ready", output, is_error)
    }
    async fn assess(&self, args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        android_assessment(args, ctx, false, "start or reuse Android emulator")
    }
    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
        let driver = driver(&args, &ctx);
        let device = driver.ensure_emulator_ready().await?;
        let observation = driver.observe().await.ok();
        Ok(tool_json(
            self.name(),
            json!({
                "summary": format!("Android · emulator ready · {}", device.serial),
                "device": device,
                "observation": observation,
            }),
        ))
    }
}

#[async_trait]
impl AgentTool for AndroidTapTextTool {
    fn name(&self) -> &'static str {
        "android.tap_text"
    }
    fn description(&self) -> &'static str {
        "Tap an Android UI element by visible text using the UIAutomator tree, then refresh the observation."
    }
    fn schema(&self) -> Value {
        json!({
            "type":"object","additionalProperties":false,
            "properties":{
                "text":{"type":"string"},
                "match":{"type":"string","enum":["exact","contains"]},
                "serial":{"type":"string"}
            },
            "required":["text"]
        })
    }
    fn capabilities(&self) -> ToolCapabilities {
        android_capabilities()
    }
    fn policy(&self) -> ToolPolicy {
        android_policy(AndroidLanePolicy::Action)
    }
    fn icon(&self) -> IconToken {
        IconToken::Terminal
    }
    fn label(&self, running: bool) -> String {
        label("Tap Android text", running)
    }
    fn args_preview(&self, args: &Value) -> String {
        quote_arg(args, "text")
    }
    fn row_label(&self, command: Option<&str>, running: bool) -> String {
        android_row("tap", command, running)
    }
    fn finish_summary(&self, _args: &Value, output: &str, is_error: bool) -> String {
        android_finish_summary("Android · tapped", output, is_error)
    }
    async fn assess(&self, args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        android_assessment(args, ctx, false, "tap Android emulator")
    }
    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
        let text = required_str(&args, "text")?;
        let requested_match = args
            .get("match")
            .and_then(|v| v.as_str())
            .unwrap_or("exact");
        if strict_android_agent_mode(&ctx) && requested_match == "contains" {
            return Ok(tool_json_error(
                self.name(),
                json!({
                    "summary": "Android · tap rejected · contains match not allowed in agent mode",
                    "requested_selector": {"type": "text", "value": text, "match": requested_match},
                }),
            ));
        }
        let match_mode = match args
            .get("match")
            .and_then(|v| v.as_str())
            .unwrap_or("exact")
        {
            "contains" => TextMatchMode::Contains,
            _ => TextMatchMode::Exact,
        };
        execute_resolved_tap(
            self.name(),
            &args,
            ctx,
            AndroidTarget::Text {
                value: text.to_string(),
                match_mode,
            },
            format!("Tap \"{text}\""),
        )
        .await
    }
}

#[async_trait]
impl AgentTool for AndroidTapResourceIdTool {
    fn name(&self) -> &'static str {
        "android.tap_resource_id"
    }
    fn description(&self) -> &'static str {
        "Tap an Android UI element by exact resource id using the UIAutomator tree."
    }
    fn schema(&self) -> Value {
        json!({"type":"object","additionalProperties":false,"properties":{"resource_id":{"type":"string"},"serial":{"type":"string"}},"required":["resource_id"]})
    }
    fn capabilities(&self) -> ToolCapabilities {
        android_capabilities()
    }
    fn policy(&self) -> ToolPolicy {
        android_policy(AndroidLanePolicy::Action)
    }
    fn icon(&self) -> IconToken {
        IconToken::Terminal
    }
    fn label(&self, running: bool) -> String {
        label("Tap Android resource", running)
    }
    fn args_preview(&self, args: &Value) -> String {
        quote_arg(args, "resource_id")
    }
    fn row_label(&self, command: Option<&str>, running: bool) -> String {
        android_row("tap", command, running)
    }
    async fn assess(&self, args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        android_assessment(args, ctx, false, "tap Android emulator")
    }
    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
        let resource_id = required_str(&args, "resource_id")?;
        execute_resolved_tap(
            self.name(),
            &args,
            ctx,
            AndroidTarget::ResourceId(resource_id.to_string()),
            format!("Tap {resource_id}"),
        )
        .await
    }
}

#[async_trait]
impl AgentTool for AndroidTapPointTool {
    fn name(&self) -> &'static str {
        "android.tap_point"
    }
    fn description(&self) -> &'static str {
        "Tap a raw Android device coordinate. Prefer semantic tap tools when possible."
    }
    fn schema(&self) -> Value {
        json!({"type":"object","additionalProperties":false,"properties":{"x":{"type":"number"},"y":{"type":"number"},"serial":{"type":"string"}},"required":["x","y"]})
    }
    fn capabilities(&self) -> ToolCapabilities {
        android_capabilities()
    }
    fn policy(&self) -> ToolPolicy {
        android_policy(AndroidLanePolicy::DenyInAgentMode)
    }
    fn icon(&self) -> IconToken {
        IconToken::Terminal
    }
    fn label(&self, running: bool) -> String {
        label("Tap Android point", running)
    }
    async fn assess(&self, args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        if strict_android_agent_mode(ctx) {
            return Ok(ToolAssessment {
                risk: RiskLevel::Medium,
                requires_approval: false,
                reason: "raw tap not allowed in agent mode".into(),
                affected_paths: vec![ctx.project_root.join(".android-agent")],
                network_access: NetworkAccess::Disabled,
                writes_to_disk: true,
                runs_real_process: true,
                denied: true,
            });
        }
        android_assessment(args, ctx, false, "tap Android emulator")
    }
    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
        let point = point_arg(&args, "x", "y")?;
        let driver = driver(&args, &ctx);
        let before = driver.observe().await.ok();
        let result = driver.tap(point).await?;
        let after = driver.observe().await.ok();
        let action = action_trace(
            "tap_point",
            Some(format!("{:.0},{:.0}", point.x, point.y)),
            result.duration_ms,
            before.as_ref(),
            after.as_ref(),
        );
        record_action(&ctx, &action);
        Ok(tool_json(
            self.name(),
            json!({"summary": result.summary, "observation": after, "action_trace": action}),
        ))
    }
}

#[async_trait]
impl AgentTool for AndroidTypeTextTool {
    fn name(&self) -> &'static str {
        "android.type_text"
    }
    fn description(&self) -> &'static str {
        "Type text into the currently focused Android field."
    }
    fn schema(&self) -> Value {
        json!({"type":"object","additionalProperties":false,"properties":{"text":{"type":"string"},"sensitive":{"type":"boolean"},"serial":{"type":"string"}},"required":["text"]})
    }
    fn capabilities(&self) -> ToolCapabilities {
        android_capabilities()
    }
    fn policy(&self) -> ToolPolicy {
        android_policy(AndroidLanePolicy::Action)
    }
    fn icon(&self) -> IconToken {
        IconToken::Terminal
    }
    fn label(&self, running: bool) -> String {
        label("Type Android text", running)
    }
    fn args_preview(&self, args: &Value) -> String {
        if args
            .get("sensitive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            "sensitive text".into()
        } else {
            args.get("text")
                .and_then(|v| v.as_str())
                .map(|s| format!("{} chars", s.chars().count()))
                .unwrap_or_default()
        }
    }
    async fn assess(&self, args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        android_assessment(args, ctx, false, "type into Android emulator")
    }
    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
        let text = required_str(&args, "text")?;
        let sensitive = args
            .get("sensitive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let driver = driver(&args, &ctx);
        let before = driver.observe().await.ok();
        let result = driver.type_text(text, sensitive).await?;
        let after = driver.observe().await.ok();
        let action = action_trace(
            "type_text",
            Some(format!("{} chars", text.chars().count())),
            result.duration_ms,
            before.as_ref(),
            after.as_ref(),
        );
        record_action(&ctx, &action);
        Ok(tool_json(
            self.name(),
            json!({"summary": result.summary, "observation": after, "action_trace": action}),
        ))
    }
}

#[async_trait]
impl AgentTool for AndroidSwipeTool {
    fn name(&self) -> &'static str {
        "android.swipe"
    }
    fn description(&self) -> &'static str {
        "Swipe between two Android device coordinates."
    }
    fn schema(&self) -> Value {
        json!({
            "type":"object","additionalProperties":false,
            "properties":{
                "from":{"type":"object","properties":{"x":{"type":"number"},"y":{"type":"number"}},"required":["x","y"]},
                "to":{"type":"object","properties":{"x":{"type":"number"},"y":{"type":"number"}},"required":["x","y"]},
                "duration_ms":{"type":"integer"},
                "serial":{"type":"string"}
            },
            "required":["from","to"]
        })
    }
    fn capabilities(&self) -> ToolCapabilities {
        android_capabilities()
    }
    fn policy(&self) -> ToolPolicy {
        android_policy(AndroidLanePolicy::Action)
    }
    fn icon(&self) -> IconToken {
        IconToken::Terminal
    }
    fn label(&self, running: bool) -> String {
        label("Swipe Android", running)
    }
    async fn assess(&self, args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        android_assessment(args, ctx, false, "swipe Android emulator")
    }
    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
        let from = nested_point_arg(&args, "from")?;
        let to = nested_point_arg(&args, "to")?;
        let duration_ms = args
            .get("duration_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(300);
        let driver = driver(&args, &ctx);
        let before = driver.observe().await.ok();
        let result = driver.swipe(from, to, duration_ms).await?;
        let after = driver.observe().await.ok();
        let action = action_trace(
            "swipe",
            None,
            result.duration_ms,
            before.as_ref(),
            after.as_ref(),
        );
        record_action(&ctx, &action);
        Ok(tool_json(
            self.name(),
            json!({"summary": result.summary, "observation": after, "action_trace": action}),
        ))
    }
}

macro_rules! key_tool {
    ($ty:ident, $name:literal, $label:literal, $key:expr) => {
        #[async_trait]
        impl AgentTool for $ty {
            fn name(&self) -> &'static str { $name }
            fn description(&self) -> &'static str { concat!("Press ", $label, " on the Android device.") }
            fn schema(&self) -> Value { json!({"type":"object","additionalProperties":false,"properties":{"serial":{"type":"string"}}}) }
            fn capabilities(&self) -> ToolCapabilities { android_capabilities() }
            fn policy(&self) -> ToolPolicy { android_policy(AndroidLanePolicy::Action) }
            fn icon(&self) -> IconToken { IconToken::Terminal }
            fn label(&self, running: bool) -> String { label(concat!("Press Android ", $label), running) }
            async fn assess(&self, args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
                android_assessment(args, ctx, false, concat!("press Android ", $label))
            }
            async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
                let driver = driver(&args, &ctx);
                let before = driver.observe().await.ok();
                let result = driver.press_key($key).await?;
                let after = driver.observe().await.ok();
                let action = action_trace($name, None, result.duration_ms, before.as_ref(), after.as_ref());
                record_action(&ctx, &action);
                Ok(tool_json(self.name(), json!({"summary": result.summary, "observation": after, "action_trace": action})))
            }
        }
    };
}

key_tool!(
    AndroidPressBackTool,
    "android.press_back",
    "back",
    AndroidKey::Back
);
key_tool!(
    AndroidPressHomeTool,
    "android.press_home",
    "home",
    AndroidKey::Home
);

#[async_trait]
impl AgentTool for AndroidLaunchAppTool {
    fn name(&self) -> &'static str {
        "android.launch_app"
    }
    fn description(&self) -> &'static str {
        "Launch an already-installed Android app by package and optional activity."
    }
    fn schema(&self) -> Value {
        json!({"type":"object","additionalProperties":false,"properties":{"package":{"type":"string"},"activity":{"type":"string"},"serial":{"type":"string"}},"required":["package"]})
    }
    fn capabilities(&self) -> ToolCapabilities {
        android_capabilities()
    }
    fn policy(&self) -> ToolPolicy {
        android_policy(AndroidLanePolicy::Action)
    }
    fn icon(&self) -> IconToken {
        IconToken::Terminal
    }
    fn label(&self, running: bool) -> String {
        label("Launch Android app", running)
    }
    fn args_preview(&self, args: &Value) -> String {
        args.get("package")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    }
    async fn assess(&self, args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        android_assessment(args, ctx, false, "launch already-installed Android app")
    }
    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
        let package = required_str(&args, "package")?;
        let activity = args.get("activity").and_then(|v| v.as_str());
        let driver = driver(&args, &ctx);
        let before = driver.observe().await.ok();
        let result = driver.launch_app(package, activity).await?;
        let observation = driver.observe().await.ok();
        let action = action_trace(
            "launch_app",
            Some(package.to_string()),
            result.duration_ms,
            before.as_ref(),
            observation.as_ref(),
        );
        record_action(&ctx, &action);
        Ok(tool_json(
            self.name(),
            json!({"summary": result.summary, "observation": observation, "action_trace": action}),
        ))
    }
}

#[async_trait]
impl AgentTool for AndroidReadLogcatTool {
    fn name(&self) -> &'static str {
        "android.read_logcat"
    }
    fn description(&self) -> &'static str {
        "Read a compact Android logcat excerpt, optionally filtered by package."
    }
    fn schema(&self) -> Value {
        json!({"type":"object","additionalProperties":false,"properties":{"package":{"type":"string"},"max_lines":{"type":"integer"},"serial":{"type":"string"}}})
    }
    fn capabilities(&self) -> ToolCapabilities {
        android_capabilities()
    }
    fn policy(&self) -> ToolPolicy {
        android_policy(AndroidLanePolicy::Utility)
    }
    fn icon(&self) -> IconToken {
        IconToken::Terminal
    }
    fn label(&self, running: bool) -> String {
        label("Read Android logs", running)
    }
    async fn assess(&self, args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        android_assessment(args, ctx, false, "read Android app-scoped logcat")
    }
    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
        let driver = driver(&args, &ctx);
        let logs = driver
            .read_logcat(LogcatFilter {
                package: args
                    .get("package")
                    .and_then(|v| v.as_str())
                    .map(ToOwned::to_owned),
                max_lines: args
                    .get("max_lines")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(120) as usize,
            })
            .await?;
        let _ = JourneyRecorder::new(&ctx.project_root, &ctx.run_id.0)
            .and_then(|recorder| recorder.write_logcat(&logs.output));
        Ok(tool_json(
            self.name(),
            json!({"summary": format!("Android · logcat {} lines", logs.output.lines().count()), "logcat": logs.output, "truncated": logs.truncated}),
        ))
    }
}

async fn execute_resolved_tap(
    tool_name: &str,
    args: &Value,
    ctx: ToolContext,
    target: AndroidTarget,
    label: String,
) -> Result<ToolResult, String> {
    let driver = driver(args, &ctx);
    let before = driver.observe().await?;
    let resolved = match resolve_target(&before.visible_targets, target.clone()) {
        Some(resolved) => resolved,
        None => {
            return Ok(tap_resolution_error(tool_name, &before, target, label));
        }
    };
    let result = driver.tap(resolved.point).await?;
    let after = driver.observe().await.ok();
    let changed = after
        .as_ref()
        .is_some_and(|after| observation_changed(&before, after));
    let action = AndroidActionTrace {
        action_id: format!("act_{}", now_ms()),
        action: "tap".into(),
        target: Some(label.clone()),
        reason: Some(match resolved.node.as_ref() {
            Some(node) if node.resource_id.is_some() => {
                "resource/text matched UIAutomator node".into()
            }
            Some(_) => "visible UIAutomator node matched".into(),
            None => "raw point fallback".into(),
        }),
        confidence: Some(format!("{:?}", resolved.confidence).to_ascii_lowercase()),
        before_observation: Some(before.observation_id.clone()),
        after_observation: after.as_ref().map(|obs| obs.observation_id.clone()),
        settle: None,
        status: if result.success && changed {
            "completed"
        } else if result.success {
            "no_visible_change"
        } else {
            "failed"
        }
        .into(),
        duration_ms: Some(result.duration_ms),
    };
    record_action(&ctx, &action);
    Ok(tool_json(
        tool_name,
        json!({
            "summary": tap_summary(&label, &before, after.as_ref()),
            "observation": after,
            "action_trace": action,
            "action_visualization": AndroidActionVisualization {
                label,
                reason: action.reason.clone(),
                confidence: action.confidence.clone(),
                target_bounds: resolved.node.as_ref().map(|node| node.bounds),
                from: None,
                to: Some(resolved.point),
                phase: AndroidActionPhase::Completed,
                actor: AndroidActionActor::Agent,
            }
        }),
    ))
}

fn android_capabilities() -> ToolCapabilities {
    ToolCapabilities {
        category: ToolCategory::Other,
        parallel_safe: false,
        cache_output: false,
        persist_result_body: false,
        suppress_live_output: true,
    }
}

fn android_policy(android_lane: AndroidLanePolicy) -> ToolPolicy {
    ToolPolicy {
        mode_gate: ToolModeGate::RunRealCommands,
        pack_policy: ToolPackPolicy::Only(vec![ToolPack::UiBrowser, ToolPack::General]),
        runtime_family: ToolRuntimeFamily::AndroidDevice,
        android_lane,
        ..ToolPolicy::default()
    }
}

fn android_assessment(
    args: &Value,
    ctx: &ToolContext,
    requires_approval: bool,
    reason: &str,
) -> Result<ToolAssessment, String> {
    let physical = args
        .get("serial")
        .and_then(|v| v.as_str())
        .is_some_and(|serial| !serial.starts_with("emulator-"));
    let denied = !ctx.mode.can_run_real_commands();
    Ok(ToolAssessment {
        risk: if physical {
            RiskLevel::High
        } else if requires_approval {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        },
        requires_approval: requires_approval || physical,
        reason: if denied {
            "Android tools require real command capability".into()
        } else if physical {
            "physical Android device interaction requires approval".into()
        } else {
            reason.into()
        },
        affected_paths: vec![ctx.project_root.join(".android-agent")],
        network_access: NetworkAccess::Disabled,
        writes_to_disk: true,
        runs_real_process: true,
        denied,
    })
}

fn driver(args: &Value, ctx: &ToolContext) -> AdbSnapshotDriver {
    AdbSnapshotDriver::new(
        &ctx.project_root,
        args.get("serial")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned),
    )
    .with_avd(
        args.get("avd")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned),
    )
}

fn tool_json(name: &str, payload: Value) -> ToolResult {
    ToolResult {
        call_id: agent_protocol::ToolCallId::new(""),
        name: name.into(),
        output: serde_json::to_string_pretty(&payload).unwrap_or_else(|_| payload.to_string()),
        is_error: false,
    }
}

fn observe_summary(observation: &agent_protocol::AndroidObservation) -> String {
    format!(
        "Android · observed {} · {} targets",
        observation
            .activity
            .as_deref()
            .or(observation.package.as_deref())
            .unwrap_or("screen"),
        observation.visible_targets.len()
    )
}

fn tap_summary(
    label: &str,
    before: &agent_protocol::AndroidObservation,
    after: Option<&agent_protocol::AndroidObservation>,
) -> String {
    let Some(after) = after else {
        return format!("Android · tapped {label} · no follow-up observation");
    };
    if observation_changed(before, after) {
        format!(
            "Android · tapped {label} · now {}",
            observation_label(after)
        )
    } else {
        format!(
            "Android · tapped {label} · no visible change ({})",
            observation_label(after)
        )
    }
}

fn observation_changed(
    before: &agent_protocol::AndroidObservation,
    after: &agent_protocol::AndroidObservation,
) -> bool {
    before.package != after.package
        || before.activity != after.activity
        || before.visible_targets != after.visible_targets
}

fn observation_label(observation: &agent_protocol::AndroidObservation) -> String {
    observation
        .activity
        .as_deref()
        .or(observation.package.as_deref())
        .unwrap_or("screen")
        .to_string()
}

fn android_finish_summary(default: &str, output: &str, is_error: bool) -> String {
    if let Some(summary) = serde_json::from_str::<Value>(output)
        .ok()
        .and_then(|value| {
            value
                .get("summary")
                .and_then(|v| v.as_str())
                .map(ToOwned::to_owned)
        })
    {
        return summary;
    }
    if is_error {
        return default_finish_summary(default, output, true);
    }
    default.to_string()
}

fn action_trace(
    action: &str,
    target: Option<String>,
    duration_ms: u64,
    before: Option<&agent_protocol::AndroidObservation>,
    after: Option<&agent_protocol::AndroidObservation>,
) -> AndroidActionTrace {
    AndroidActionTrace {
        action_id: format!("act_{}", now_ms()),
        action: action.into(),
        target,
        reason: None,
        confidence: None,
        before_observation: before.map(|obs| obs.observation_id.clone()),
        after_observation: after.map(|obs| obs.observation_id.clone()),
        settle: None,
        status: "completed".into(),
        duration_ms: Some(duration_ms),
    }
}

fn record_action(ctx: &ToolContext, action: &AndroidActionTrace) {
    let _ = JourneyRecorder::new(&ctx.project_root, &ctx.run_id.0)
        .and_then(|recorder| recorder.append_action(action));
}

fn label(label: &str, running: bool) -> String {
    if running {
        format!("{label}…")
    } else {
        label.into()
    }
}

fn android_row(kind: &str, command: Option<&str>, running: bool) -> String {
    match (command, running) {
        (Some(command), true) => format!("Android · {kind} {command}…"),
        (Some(command), false) => format!("Android · {kind} {command}"),
        (None, true) => format!("Android · {kind}…"),
        (None, false) => format!("Android · {kind}"),
    }
}

fn quote_arg(args: &Value, key: &str) -> String {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|value| format!("\"{value}\""))
        .unwrap_or_default()
}

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("missing {key}"))
}

fn point_arg(args: &Value, x_key: &str, y_key: &str) -> Result<AndroidPointPx, String> {
    Ok(AndroidPointPx {
        x: args
            .get(x_key)
            .and_then(|v| v.as_f64())
            .ok_or_else(|| format!("missing {x_key}"))? as f32,
        y: args
            .get(y_key)
            .and_then(|v| v.as_f64())
            .ok_or_else(|| format!("missing {y_key}"))? as f32,
    })
}

fn nested_point_arg(args: &Value, key: &str) -> Result<AndroidPointPx, String> {
    let nested = args.get(key).ok_or_else(|| format!("missing {key}"))?;
    point_arg(nested, "x", "y")
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

fn tool_json_error(name: &str, payload: Value) -> ToolResult {
    ToolResult {
        call_id: agent_protocol::ToolCallId::new(""),
        name: name.into(),
        output: serde_json::to_string_pretty(&payload).unwrap_or_else(|_| payload.to_string()),
        is_error: true,
    }
}

fn strict_android_agent_mode(ctx: &ToolContext) -> bool {
    ctx.mode.can_run_real_commands()
}

fn tap_resolution_error(
    tool_name: &str,
    observation: &AndroidObservation,
    target: AndroidTarget,
    label: String,
) -> ToolResult {
    let nearby_targets = observation
        .visible_targets
        .iter()
        .filter(|node| node.visible && node.enabled)
        .take(5)
        .map(nearby_target_candidate)
        .collect::<Vec<_>>();
    tool_json_error(
        tool_name,
        json!({
            "summary": format!("Android · tap rejected · target absent in observation {}", observation.observation_id),
            "requested_selector": requested_selector_json(&target),
            "requested_label": label,
            "observation_id": observation.observation_id,
            "nearby_valid_targets": nearby_targets,
            "observation": observation,
        }),
    )
}

fn requested_selector_json(target: &AndroidTarget) -> Value {
    match target {
        AndroidTarget::Text { value, match_mode } => json!({
            "type": "text",
            "value": value,
            "match": match match_mode {
                TextMatchMode::Exact => "exact",
                TextMatchMode::Contains => "contains",
            }
        }),
        AndroidTarget::ResourceId(value) => json!({"type": "resource_id", "value": value}),
        AndroidTarget::ContentDescription(value) => {
            json!({"type": "content_desc", "value": value})
        }
        AndroidTarget::Bounds(bounds) => json!({
            "type": "bounds",
            "left": bounds.left,
            "top": bounds.top,
            "right": bounds.right,
            "bottom": bounds.bottom,
        }),
        AndroidTarget::Point(point) => json!({"type": "point", "x": point.x, "y": point.y}),
    }
}

fn nearby_target_candidate(node: &AndroidUiNode) -> Value {
    json!({
        "label": node_label(node),
        "text": node.text,
        "resource_id": node.resource_id,
        "content_desc": node.content_desc,
        "clickable": node.clickable,
        "enabled": node.enabled,
        "visible": node.visible,
    })
}

fn node_label(node: &AndroidUiNode) -> String {
    node.text
        .as_deref()
        .filter(|value| !value.is_empty())
        .or(node
            .content_desc
            .as_deref()
            .filter(|value| !value.is_empty()))
        .or(node
            .resource_id
            .as_deref()
            .filter(|value| !value.is_empty()))
        .unwrap_or(node.class_name.as_str())
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_context(mode: agent_protocol::AgentMode) -> ToolContext {
        ToolContext {
            project_root: std::path::PathBuf::from("/tmp/project"),
            project_id: agent_protocol::ProjectId::new("project"),
            session_id: agent_protocol::SessionId::new("session"),
            run_id: agent_protocol::RunId::new("run"),
            mode,
            output_sink: None,
        }
    }

    fn test_observation() -> AndroidObservation {
        AndroidObservation {
            observation_id: "obs-1".into(),
            device: None,
            package: Some("com.example".into()),
            activity: Some("MainActivity".into()),
            screen: agent_protocol::AndroidSizePx {
                width: 1080.0,
                height: 1920.0,
            },
            visible_targets: vec![AndroidUiNode {
                text: Some("Settings".into()),
                resource_id: Some("com.example:id/settings".into()),
                content_desc: None,
                class_name: "android.widget.TextView".into(),
                package: Some("com.example".into()),
                clickable: true,
                enabled: true,
                visible: true,
                bounds: agent_protocol::AndroidRectPx {
                    left: 0.0,
                    top: 0.0,
                    right: 100.0,
                    bottom: 40.0,
                },
            }],
            screenshot_ref: None,
            ui_tree_ref: None,
            timestamp_ms: 1,
        }
    }

    #[tokio::test]
    async fn tap_point_is_denied_in_agent_mode() {
        let assessment = AndroidTapPointTool
            .assess(
                &json!({"x": 1.0, "y": 2.0}),
                &test_context(agent_protocol::AgentMode::ApplyWithApproval),
            )
            .await
            .expect("assessment");
        assert!(assessment.denied);
        assert_eq!(assessment.reason, "raw tap not allowed in agent mode");
    }

    #[test]
    fn tap_resolution_error_reports_observation_and_candidates() {
        let result = tap_resolution_error(
            "android.tap_text",
            &test_observation(),
            AndroidTarget::Text {
                value: "Menu".into(),
                match_mode: TextMatchMode::Exact,
            },
            "Tap \"Menu\"".into(),
        );
        assert!(result.is_error);
        let payload: Value = serde_json::from_str(&result.output).expect("json payload");
        assert_eq!(payload["observation_id"], "obs-1");
        assert_eq!(payload["requested_selector"]["value"], "Menu");
        assert_eq!(payload["nearby_valid_targets"][0]["label"], "Settings");
    }
}
