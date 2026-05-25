use serde_json::Value;

/// Deep-merge JSON objects (nested keys) — object behaviour aligned with `object-assign-deep`.
pub fn merge_json (a: &Value, b: &Value) -> Value {
  match (a, b) {
    (Value::Object(oa), Value::Object(ob)) => {
      let mut m = oa.clone();
      for (k, v) in ob {
        let next = m.get(k).map(|e| merge_json(e, v)).unwrap_or_else(|| v.clone());
        m.insert(k.clone(), next);
      }
      Value::Object(m)
    }
    (_, b) if !b.is_null() => b.clone(),
    _ => a.clone(),
  }
}
