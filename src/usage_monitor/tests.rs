use super::*;
use serde_json::json;
#[test]
fn native_warnings_follow_session_weekly_and_reset_thresholds() {
    let reset = crate::session_store::now() + 3600;
    let mut v = json!({"five_hour":{"used":17,"limit":20,"resets_at":reset},"weekly":{"used":0,"limit":100,"resets_at":reset+86400}});
    assert!(label(&v).is_none());
    v["five_hour"]["used"] = json!(18);
    assert!(label(&v).unwrap().contains("90%"));
    v["five_hour"]["used"] = json!(20);
    assert!(label(&v).unwrap().contains("Usage limit reached"));
    v["five_hour"]["resets_at"] = json!(reset - 7200);
    assert!(label(&v).is_none());
    v["weekly"]["used"] = json!(90);
    assert!(label(&v).unwrap().contains("Weekly 90%"));
}
