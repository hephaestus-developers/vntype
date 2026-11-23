//! Bộ lập kế hoạch thi hành hành động (`planner`) và tính toán thời gian chờ theo hồ sơ ứng dụng.

use vntype_engine::{Action, AppProfile, FieldKind};

use crate::field_policy;
use crate::offset::{
    char_suffix_utf16_len, is_word_codepoint, next_codepoint, utf16_len, utf16_prefix, utf16_suffix,
    utf16_to_byte_offset,
};
use crate::surrounding_guard::{SurroundingSnapshot, check_replace};

/// Thời gian chờ tối thiểu (mili-giây) giữa lệnh xoá surrounding và lệnh chèn văn bản mới.
pub const MIN_REPLACE_WAIT_MS: u32 = 3;

/// Thời gian chờ tối đa (mili-giây) giữa lệnh xoá surrounding và lệnh chèn văn bản mới.
pub const MAX_REPLACE_WAIT_MS: u32 = 60;

/// Khoảng trễ bảo vệ (mili-giây) sau khi hoàn tất commit trước khi xả phím tiếp theo trong hàng đợi.
pub const POST_COMMIT_DELAY_MS: u32 = 10;

/// Tính thời gian chờ (mili-giây) trước khi commit sau khi đã xoá `delete_chars` ký tự.
///
/// # Examples
///
/// ```
/// use vntype_adapter::{replace_wait_ms, MIN_REPLACE_WAIT_MS};
/// use vntype_engine::AppProfile;
///
/// let p = AppProfile { wait_ack: Some(true), backspace_delay_ms: Some(5), delay_per_char_ms: Some(3), ..Default::default() };
/// assert_eq!(replace_wait_ms(&p, 2), 11);
/// assert_eq!(replace_wait_ms(&AppProfile::default(), 2), MIN_REPLACE_WAIT_MS);
/// ```
pub fn replace_wait_ms(profile: &AppProfile, delete_chars: u32) -> u32 {
    if !profile.wait_ack.unwrap_or(false) {
        return MIN_REPLACE_WAIT_MS;
    }
    let base = profile.backspace_delay_ms.unwrap_or(0);
    let per_char = profile.delay_per_char_ms.unwrap_or(0);
    let computed = base.saturating_add(per_char.saturating_mul(delete_chars));
    computed.clamp(MIN_REPLACE_WAIT_MS, MAX_REPLACE_WAIT_MS)
}

/// Lượng văn bản cần xoá lùi trong một yêu cầu thay thế.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteAmount {
    /// Tính theo số lượng ký tự Unicode (`char`).
    Chars(u32),
    /// Tính theo số lượng đơn vị mã UTF-16.
    Utf16(u32),
}

/// Hành động đầu vào của bộ lập kế hoạch (đã quy đổi con trỏ Preedit sang UTF-16).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterAction {
    /// Chuyển phím thẳng cho hệ điều hành.
    Pass,
    /// Hiển thị vùng gạch chân tạm thời.
    Preedit {
        /// Chuỗi văn bản gạch chân.
        text: String,
        /// Vị trí con trỏ theo đơn vị mã UTF-16.
        cursor_utf16: u32,
    },
    /// Chốt chuỗi văn bản chính thức.
    Commit {
        /// Chuỗi văn bản cần chốt.
        text: String,
        /// Cờ chuyển tiếp phím gốc sau khi chốt.
        pass_key: bool,
    },
    /// Xoá lùi và chèn chuỗi văn bản mới.
    Replace {
        /// Chuỗi văn bản cần chèn.
        insert: String,
        /// Số lượng ký tự hoặc đơn vị mã UTF-16 cần xoá lùi.
        delete: DeleteAmount,
        /// Cờ chuyển tiếp phím gốc sau khi thay thế.
        pass_key: bool,
    },
}

impl From<Action> for AdapterAction {
    fn from(action: Action) -> Self {
        match action {
            Action::PassThrough => AdapterAction::Pass,
            Action::Preedit { text, cursor } => {
                let byte_offset =
                    text.char_indices().nth(cursor).map(|(offset, _)| offset).unwrap_or(text.len());
                let cursor_utf16 = utf16_len(&text[..byte_offset]);
                AdapterAction::Preedit { text, cursor_utf16 }
            }
            Action::Commit { text, pass_key } => AdapterAction::Commit { text, pass_key },
            Action::Replace { delete_before, insert, pass_key } => AdapterAction::Replace {
                insert,
                delete: DeleteAmount::Chars(delete_before as u32),
                pass_key,
            },
        }
    }
}

/// Ngữ cảnh truyền vào bộ lập kế hoạch [`plan_action`].
#[derive(Debug, Clone, Copy)]
pub struct PlanContext<'a> {
    /// Phân loại trường nhập liệu hiện tại.
    pub field_kind: FieldKind,
    /// Chuỗi văn bản mà bộ gõ tin là đang hiển thị trước phím gõ hiện tại.
    pub before_display: &'a str,
    /// Ảnh chụp nhanh văn bản xung quanh của ứng dụng (nếu ứng dụng hỗ trợ).
    pub surrounding: Option<SurroundingSnapshot<'a>>,
    /// Cho phép thay thế trực tiếp không cần xác minh surrounding (chỉ dùng cho Windows Hook).
    pub allow_unverified_replace: bool,
}

/// Chính sách thử lại khi thao tác `Replace` bị rơi vào `FailSafe` do ứng dụng cập nhật surrounding chậm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReplaceFallback {
    /// Không thử lại, giữ kết quả `FailSafe`.
    #[default]
    None,
    /// Thử lại dựa trên `before_display` nếu ứng dụng có cung cấp surrounding snapshot (nhưng bị trễ).
    StaleSurrounding,
    /// Luôn tin cậy `before_display` ngay cả khi ứng dụng không cung cấp surrounding.
    Always,
}

/// Phân loại kế hoạch thi hành sau khi đã kiểm tra an toàn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanKind {
    /// Không can thiệp, chuyển phím gốc cho ứng dụng.
    Pass,
    /// Cập nhật vùng gạch chân tạm thời.
    Preedit,
    /// Chốt văn bản vào ứng dụng.
    Commit,
    /// Xoá lùi văn bản trước con trỏ và chèn văn bản mới.
    Replace,
    /// Phát hiện bất đồng bộ hoặc vùng chọn không an toàn: không xoá mù, kích hoạt quy trình bảo toàn chữ.
    FailSafe,
}

/// Kế hoạch thi hành chi tiết đã tính sẵn toàn bộ hệ đơn vị đo (`UTF-16`, `char`, `byte`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// Loại kế hoạch.
    pub kind: PlanKind,
    /// Chuỗi văn bản cần hiển thị hoặc chèn.
    pub text: String,
    /// Vị trí con trỏ trong Preedit theo đơn vị mã UTF-16.
    pub cursor_utf16: u32,
    /// Vị trí con trỏ trong Preedit theo số byte UTF-8.
    pub cursor_bytes: u32,
    /// Số lượng đơn vị mã UTF-16 cần xoá lùi.
    pub delete_utf16: u32,
    /// Số lượng ký tự Unicode (`char`) cần xoá lùi.
    pub delete_chars: u32,
    /// Số lượng byte UTF-8 cần xoá lùi.
    pub delete_bytes: u32,
    /// Thời gian chờ (mili-giây) giữa bước xoá và bước chèn.
    pub wait_ms: u32,
    /// Cờ chỉ định có chuyển tiếp phím gốc sau khi hoàn tất hay không.
    pub pass_key: bool,
    /// Mã lý do phục vụ gỡ lỗi và kiểm thử.
    pub reason: &'static str,
}

impl Plan {
    fn pass(reason: &'static str) -> Self {
        Plan {
            kind: PlanKind::Pass,
            text: String::new(),
            cursor_utf16: 0,
            cursor_bytes: 0,
            delete_utf16: 0,
            delete_chars: 0,
            delete_bytes: 0,
            wait_ms: 0,
            pass_key: false,
            reason,
        }
    }

    fn fail(reason: &'static str) -> Self {
        Plan { kind: PlanKind::FailSafe, reason, ..Plan::pass(reason) }
    }
}

/// Lập kế hoạch thi hành cho một [`AdapterAction`] trong ngữ cảnh [`PlanContext`].
pub fn plan_action(action: &AdapterAction, context: &PlanContext<'_>, profile: &AppProfile) -> Plan {
    if field_policy::is_disabled(context.field_kind) {
        return Plan::pass("field-disabled");
    }

    match action {
        AdapterAction::Pass => Plan::pass("engine-pass"),
        AdapterAction::Preedit { text, cursor_utf16 } => {
            let cursor_bytes = utf16_to_byte_offset(text, *cursor_utf16).unwrap_or(text.len()) as u32;
            Plan {
                kind: PlanKind::Preedit,
                text: text.clone(),
                cursor_utf16: *cursor_utf16,
                cursor_bytes,
                delete_utf16: 0,
                delete_chars: 0,
                delete_bytes: 0,
                wait_ms: 0,
                pass_key: false,
                reason: "preedit",
            }
        }
        AdapterAction::Commit { text, pass_key } => Plan {
            kind: PlanKind::Commit,
            text: text.clone(),
            cursor_utf16: 0,
            cursor_bytes: 0,
            delete_utf16: 0,
            delete_chars: 0,
            delete_bytes: 0,
            wait_ms: 0,
            pass_key: *pass_key,
            reason: "commit",
        },
        AdapterAction::Replace { insert, delete, pass_key } => {
            let delete_utf16 = match delete {
                DeleteAmount::Utf16(units) => *units,
                DeleteAmount::Chars(chars) => {
                    let source = if context.before_display.is_empty() {
                        context.surrounding.and_then(|s| utf16_prefix(s.text, s.cursor_utf16)).unwrap_or("")
                    } else {
                        context.before_display
                    };
                    let Some(units) = char_suffix_utf16_len(source, *chars) else {
                        return Plan::fail("replace-suffix-out-of-range");
                    };
                    units
                }
            };

            if delete_utf16 == 0 {
                return Plan {
                    kind: PlanKind::Commit,
                    text: insert.clone(),
                    cursor_utf16: 0,
                    cursor_bytes: 0,
                    delete_utf16: 0,
                    delete_chars: 0,
                    delete_bytes: 0,
                    wait_ms: 0,
                    pass_key: *pass_key,
                    reason: "replace-no-delete",
                };
            }

            if context.allow_unverified_replace {
                let shown = if context.before_display.is_empty() {
                    context.surrounding.and_then(|s| utf16_prefix(s.text, s.cursor_utf16)).unwrap_or("")
                } else {
                    context.before_display
                };
                let Some(deleted) = utf16_suffix(shown, delete_utf16) else {
                    return Plan::fail("replace-unverified-suffix");
                };
                if deleted.is_empty() {
                    return Plan::fail("replace-unverified-empty");
                }
                let delete_chars = deleted.chars().count() as u32;
                let delete_bytes = deleted.len() as u32;
                return Plan {
                    kind: PlanKind::Replace,
                    text: insert.clone(),
                    cursor_utf16: 0,
                    cursor_bytes: 0,
                    delete_utf16,
                    delete_chars,
                    delete_bytes,
                    wait_ms: replace_wait_ms(profile, delete_chars),
                    pass_key: *pass_key,
                    reason: "replace-trusted-direct",
                };
            }

            let Some(surrounding) = context.surrounding else {
                return Plan::fail("replace-no-surrounding");
            };
            let Some(check) = check_replace(context.before_display, delete_utf16, surrounding) else {
                return Plan::fail("replace-surrounding-mismatch");
            };
            Plan {
                kind: PlanKind::Replace,
                text: insert.clone(),
                cursor_utf16: 0,
                cursor_bytes: 0,
                delete_utf16: check.delete_utf16,
                delete_chars: check.delete_chars,
                delete_bytes: check.delete_bytes,
                wait_ms: replace_wait_ms(profile, check.delete_chars),
                pass_key: *pass_key,
                reason: "replace-verified",
            }
        }
    }
}

/// Lập kế hoạch thi hành kèm chính sách dự phòng [`ReplaceFallback`] khi ứng dụng cập nhật surrounding chậm.
///
/// Hàm này không bao giờ bỏ qua lỗi vùng bôi đen (`cursor != anchor`) hoặc lỗi con trỏ đứng giữa từ.
pub fn plan_action_with_fallback(
    action: &AdapterAction,
    context: &PlanContext<'_>,
    profile: &AppProfile,
    fallback: ReplaceFallback,
) -> Plan {
    let plan = plan_action(action, context, profile);
    if plan.kind != PlanKind::FailSafe || !matches!(action, AdapterAction::Replace { .. }) {
        return plan;
    }

    if let Some(surrounding) = context.surrounding {
        if surrounding.cursor_utf16 != surrounding.anchor_utf16 {
            return plan;
        }
        if next_codepoint(surrounding.text, surrounding.cursor_utf16).is_some_and(is_word_codepoint) {
            return plan;
        }
    }

    let eligible = match fallback {
        ReplaceFallback::None => false,
        ReplaceFallback::StaleSurrounding => context.surrounding.is_some(),
        ReplaceFallback::Always => true,
    };
    if !eligible {
        return plan;
    }

    let trusted = PlanContext { allow_unverified_replace: true, ..*context };
    plan_action(action, &trusted, profile)
}

/// Chuyển vị trí con trỏ UTF-16 sang chỉ số byte UTF-8.
pub fn utf16_cursor_to_byte(text: &str, cursor_utf16: u32) -> Option<usize> {
    utf16_to_byte_offset(text, cursor_utf16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_wait_ms_respects_bounds_and_wait_ack() {
        assert_eq!(replace_wait_ms(&AppProfile::default(), 5), MIN_REPLACE_WAIT_MS);
        let p = AppProfile {
            wait_ack: Some(true),
            backspace_delay_ms: Some(10),
            delay_per_char_ms: Some(20),
            ..Default::default()
        };
        assert_eq!(replace_wait_ms(&p, 0), 10);
        assert_eq!(replace_wait_ms(&p, 5), MAX_REPLACE_WAIT_MS);
    }

    #[test]
    fn plan_action_verified_and_failsafe() {
        let profile = AppProfile::default();
        let snap = SurroundingSnapshot { text: "tiêng", cursor_utf16: 5, anchor_utf16: 5 };
        let ctx = PlanContext {
            field_kind: FieldKind::Text,
            before_display: "tiêng",
            surrounding: Some(snap),
            allow_unverified_replace: false,
        };
        let act = AdapterAction::Replace {
            insert: "ếng".into(),
            delete: DeleteAmount::Chars(3),
            pass_key: false,
        };
        let plan = plan_action(&act, &ctx, &profile);
        assert_eq!(plan.kind, PlanKind::Replace);
        assert_eq!(plan.delete_chars, 3);
        assert_eq!(plan.delete_utf16, 3);

        // Sai lệch surrounding -> FailSafe, với StaleSurrounding fallback sẽ cho Replace
        let stale_snap = SurroundingSnapshot { text: "tiê", cursor_utf16: 3, anchor_utf16: 3 };
        let stale_ctx = PlanContext { surrounding: Some(stale_snap), ..ctx };
        assert_eq!(plan_action(&act, &stale_ctx, &profile).kind, PlanKind::FailSafe);
        assert_eq!(
            plan_action_with_fallback(&act, &stale_ctx, &profile, ReplaceFallback::StaleSurrounding).kind,
            PlanKind::Replace
        );

        // Không bao giờ bypass khi đang bôi đen
        let sel_snap = SurroundingSnapshot { text: "tiêng", cursor_utf16: 5, anchor_utf16: 2 };
        let sel_ctx = PlanContext { surrounding: Some(sel_snap), ..ctx };
        assert_eq!(
            plan_action_with_fallback(&act, &sel_ctx, &profile, ReplaceFallback::Always).kind,
            PlanKind::FailSafe
        );
    }
}
